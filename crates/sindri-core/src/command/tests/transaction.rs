//! A transaction is all or nothing, and undoes as one step.

use crate::{CommandError, CommandHistory, EntityData, Transform3D, WorldCommand, WorldError};

use super::support::{edit, layer_bits, world_with_two_entities};

/// A transaction is all or nothing, and a refusal is what tests that: the
/// rename beside the rejected move must not survive it.
#[test]
fn a_refused_move_rolls_back_the_rest_of_its_transaction() {
    let (mut world, entity, other) = world_with_two_entities();
    world.get_mut(entity).unwrap().transform_3d = Some(Transform3D {
        position: [0.0, 0.0, -50.0],
        z_locked: true,
        ..Transform3D::default()
    });
    let mut history = CommandHistory::default();

    let error = history
        .apply(
            edit(
                "Move selection",
                vec![
                    WorldCommand::SetName {
                        entity: other,
                        name: Some("Renamed".into()),
                    },
                    WorldCommand::SetTransform3D {
                        entity,
                        transform: None,
                    },
                ],
            ),
            &mut world,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CommandError::Rejected {
            source: WorldError::TransformZLocked(_),
            index: 1,
            ..
        }
    ));
    assert_eq!(world.get(other).unwrap().name, None);
    assert_eq!(layer_bits(&world, entity), (-50.0_f32).to_bits());
}

#[test]
fn a_transaction_undoes_as_one_step() {
    let (mut world, parent, child) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let moved = Transform3D {
        position: [1.0, 2.0, 3.0],
        ..Transform3D::default()
    };

    history
        .apply(
            edit(
                "Move selection",
                vec![
                    WorldCommand::SetTransform3D {
                        entity: parent,
                        transform: Some(moved),
                    },
                    WorldCommand::SetTransform3D {
                        entity: child,
                        transform: Some(moved),
                    },
                ],
            ),
            &mut world,
        )
        .unwrap();

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(parent).unwrap().transform_3d, None);
    assert_eq!(world.get(child).unwrap().transform_3d, None);
    assert!(!history.can_undo());
}

#[test]
fn a_rejected_command_rolls_the_whole_transaction_back() {
    let (mut world, parent, child) = world_with_two_entities();
    let stale = world.spawn(EntityData::default());
    world.despawn_recursive(stale).unwrap();
    let mut history = CommandHistory::default();

    let error = history
        .apply(
            edit(
                "Partly invalid",
                vec![
                    WorldCommand::SetName {
                        entity: parent,
                        name: Some("Applied first".into()),
                    },
                    WorldCommand::SetName {
                        entity: stale,
                        name: Some("Never applied".into()),
                    },
                    WorldCommand::SetName {
                        entity: child,
                        name: Some("Never reached".into()),
                    },
                ],
            ),
            &mut world,
        )
        .unwrap_err();

    assert_eq!(
        error,
        CommandError::Rejected {
            label: "Partly invalid".to_owned(),
            index: 1,
            source: WorldError::InvalidEntity(stale),
        }
    );
    // The first command was applied, then reversed by the rollback.
    assert_eq!(world.get(parent).unwrap().name.as_deref(), Some("Parent"));
    assert_eq!(world.get(child).unwrap().name, None);
    assert!(!history.can_undo());
}

#[test]
fn a_rejected_reparent_leaves_the_hierarchy_untouched() {
    let (mut world, parent, child) = world_with_two_entities();
    let mut history = CommandHistory::default();
    history
        .apply(
            edit(
                "Reparent",
                vec![WorldCommand::SetParent {
                    entity: child,
                    parent: Some(parent),
                }],
            ),
            &mut world,
        )
        .unwrap();

    // Parenting the parent under its own child would close a cycle.
    let error = history
        .apply(
            edit(
                "Close a cycle",
                vec![
                    WorldCommand::SetName {
                        entity: child,
                        name: Some("Doomed".into()),
                    },
                    WorldCommand::SetParent {
                        entity: parent,
                        parent: Some(child),
                    },
                ],
            ),
            &mut world,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CommandError::Rejected {
            index: 1,
            source: WorldError::HierarchyCycle,
            ..
        }
    ));
    assert_eq!(world.get(child).unwrap().name, None);
    assert_eq!(world.get(parent).unwrap().parent, None);
    assert_eq!(world.get(child).unwrap().parent, Some(parent));
}

#[test]
fn empty_transactions_are_not_recorded() {
    let (mut world, _, _) = world_with_two_entities();
    let mut history = CommandHistory::default();
    history
        .apply(edit("Nothing", Vec::new()), &mut world)
        .unwrap();
    assert!(!history.can_undo());
    assert_eq!(history.undo(&mut world).unwrap(), None);
    assert_eq!(history.redo(&mut world).unwrap(), None);
}
