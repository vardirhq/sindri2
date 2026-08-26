//! What a locked transform refuses, and what it still allows.

use crate::{CommandError, CommandHistory, Transform3D, WorldCommand, WorldError};

use super::support::{edit, layer_bits, world_with_two_entities};

/// A declared Z lock is a check the command layer makes, which is what
/// makes declaring it worth anything: every tool writes through here.
#[test]
fn a_locked_transform_refuses_to_be_moved_off_its_layer() {
    let (mut world, entity, _) = world_with_two_entities();
    let background = Transform3D {
        position: [0.0, 0.0, -50.0],
        z_locked: true,
        ..Transform3D::default()
    };
    world.get_mut(entity).unwrap().transform_3d = Some(background);

    let mut flattened = background;
    flattened.position[2] = 0.0;
    let mut history = CommandHistory::default();
    let error = history
        .apply(
            edit(
                "Flatten",
                vec![WorldCommand::SetTransform3D {
                    entity,
                    transform: Some(flattened),
                }],
            ),
            &mut world,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CommandError::Rejected {
            source: WorldError::TransformZLocked(named),
            ..
        } if named == entity
    ));
    assert_eq!(
        layer_bits(&world, entity),
        (-50.0_f32).to_bits(),
        "the refused command must not have moved anything"
    );
    assert!(
        history.undo(&mut world).unwrap().is_none(),
        "a refused command must not enter the history"
    );
}

/// Locked is about the layer alone: the same transform still moves around
/// its plane, and unlocking is what grants permission to leave it.
#[test]
fn a_locked_transform_still_moves_within_its_layer_and_unlocks() {
    let (mut world, entity, _) = world_with_two_entities();
    let background = Transform3D {
        position: [0.0, 0.0, -50.0],
        z_locked: true,
        ..Transform3D::default()
    };
    world.get_mut(entity).unwrap().transform_3d = Some(background);
    let mut history = CommandHistory::default();

    let mut slid = background;
    slid.translate_2d([3.0, 1.0]);
    history
        .apply(
            edit(
                "Slide",
                vec![WorldCommand::SetTransform3D {
                    entity,
                    transform: Some(slid),
                }],
            ),
            &mut world,
        )
        .expect("moving in the plane is what the lock leaves alone");

    let unlocked = Transform3D {
        z_locked: false,
        ..slid
    };
    let mut moved = unlocked;
    moved.position[2] = -10.0;
    history
        .apply(
            edit(
                "Unlock",
                vec![WorldCommand::SetTransform3D {
                    entity,
                    transform: Some(unlocked),
                }],
            ),
            &mut world,
        )
        .expect("unlocking changes no layer");
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
        .expect("an unlocked transform moves");
    assert_eq!(layer_bits(&world, entity), (-10.0_f32).to_bits());
}
