//! Undo, redo, the revision they move, and the limit they run into.

use crate::{CommandHistory, Transform3D, WorldCommand};

use super::support::{edit, world_with_two_entities};

#[test]
fn undo_and_redo_restore_each_recorded_value() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "Rename",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some("Renamed".into()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Renamed"));

    assert_eq!(history.undo(&mut world).unwrap().as_deref(), Some("Rename"));
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Parent"));

    assert_eq!(history.redo(&mut world).unwrap().as_deref(), Some("Rename"));
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Renamed"));
}

/// What the revision is for: a caller can ask whether the world has come
/// back to a state it remembers, without comparing whole documents.
#[test]
fn undoing_back_to_a_state_returns_its_revision() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let saved = history.revision();

    let moved = Transform3D {
        position: [1.0, 2.0, 3.0],
        ..Transform3D::default()
    };
    history
        .apply(
            edit(
                "Move",
                vec![WorldCommand::SetTransform3D {
                    entity,
                    transform: Some(moved),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_ne!(history.revision(), saved, "an edit is a different state");

    history.undo(&mut world).unwrap();
    assert_eq!(
        history.revision(),
        saved,
        "undoing back to where it was saved must read as saved"
    );

    history.redo(&mut world).unwrap();
    assert_ne!(history.revision(), saved);
    history.undo(&mut world).unwrap();
    assert_eq!(history.revision(), saved, "and again, however many times");
}

/// A different edit made after undoing cannot land on the abandoned state,
/// however much the stacks happen to line up.
#[test]
fn an_edit_after_undoing_never_reuses_the_state_it_replaced() {
    let (mut world, entity, other) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let rename = |name: &str| {
        edit(
            "Rename",
            vec![WorldCommand::SetName {
                entity,
                name: Some(name.to_owned()),
            }],
        )
    };

    history.apply(rename("First"), &mut world).unwrap();
    let abandoned = history.revision();
    history.undo(&mut world).unwrap();
    history
        .apply(
            edit(
                "Rename other",
                vec![WorldCommand::SetName {
                    entity: other,
                    name: Some("Other".to_owned()),
                }],
            ),
            &mut world,
        )
        .unwrap();

    assert_ne!(history.revision(), abandoned);
    assert!(history.undo(&mut world).unwrap().is_some());
    assert_ne!(
        history.revision(),
        abandoned,
        "the stacks match the abandoned state's shape, and the world does not"
    );
}

/// Rebuilding a world is not a return to anything.
#[test]
fn clearing_the_history_moves_to_a_state_of_its_own() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let empty = history.revision();
    history
        .apply(
            edit(
                "Rename",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some("Renamed".to_owned()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    history.clear();
    assert_ne!(history.revision(), empty);
}

#[test]
fn applying_a_new_edit_discards_the_redo_stack() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();

    history
        .apply(
            edit(
                "First",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some("First".into()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    history.undo(&mut world).unwrap();
    assert!(history.can_redo());

    history
        .apply(
            edit(
                "Second",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some("Second".into()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert!(!history.can_redo());
    assert_eq!(history.undo_label(), Some("Second"));
}

#[test]
fn history_is_bounded_by_its_limit() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::with_limit(2);
    for index in 0..4 {
        history
            .apply(
                edit(
                    format!("Edit {index}"),
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some(format!("Name {index}")),
                    }],
                ),
                &mut world,
            )
            .unwrap();
    }
    assert_eq!(history.undo_label(), Some("Edit 3"));
    history.undo(&mut world).unwrap();
    history.undo(&mut world).unwrap();
    assert!(!history.can_undo());
}

#[test]
fn a_zero_limit_applies_without_recording() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::with_limit(0);
    history
        .apply(
            edit(
                "Unrecorded",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some("Applied".into()),
                }],
            ),
            &mut world,
        )
        .unwrap();
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Applied"));
    assert!(!history.can_undo());
}
