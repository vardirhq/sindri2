//! A run of edits that collapses into one undo step, and where it breaks.

use crate::{CommandBuffer, CommandHistory, Transform3D, World, WorldCommand};

use super::support::{edit, position, world_with_two_entities};

/// A drag merges into one undo step, so the stack does not grow while the
/// world keeps changing. The revision has to move anyway, or a drag that
/// began at the saved state would read as though nothing had happened.
#[test]
fn a_merged_run_moves_the_revision_every_time_it_changes_the_world() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let saved = history.revision();

    let mut seen = vec![saved];
    for step in [1.0_f32, 2.0, 3.0, 4.0] {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetTransform3D {
            entity,
            transform: Some(Transform3D {
                position: [step, 0.0, 0.0],
                ..Transform3D::default()
            }),
        });
        history
            .apply(buffer.into_transaction("Drag").merging("drag"), &mut world)
            .unwrap();
        assert!(
            !seen.contains(&history.revision()),
            "every step of a drag is a state of its own"
        );
        seen.push(history.revision());
    }

    history.undo(&mut world).unwrap();
    assert_eq!(
        history.revision(),
        saved,
        "and undoing the run returns to before it started"
    );
}

#[test]
fn a_merge_run_collapses_into_one_undo_step() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();
    let start = Transform3D::default();
    world.get_mut(entity).unwrap().transform_3d = Some(start);

    // Stand in for a drag: one transaction per frame, all merging.
    for step in [1.0, 2.0, 3.0, 4.0, 5.0] {
        let moved = Transform3D {
            position: [step, 0.0, 0.0],
            ..start
        };
        history
            .apply(
                edit(
                    "Move",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(moved),
                    }],
                )
                .merging("drag:torso"),
                &mut world,
            )
            .unwrap();
    }
    assert!(position(&world, entity)[0] > 4.9);

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(entity).unwrap().transform_3d, Some(start));
    assert!(!history.can_undo(), "the run should be a single step");

    history.redo(&mut world).unwrap();
    assert!(position(&world, entity)[0] > 4.9);
}

#[test]
fn breaking_a_run_starts_a_new_undo_step() {
    let (mut world, entity, _) = world_with_two_entities();
    let mut history = CommandHistory::default();

    let drag = |history: &mut CommandHistory, world: &mut World, name: &str| {
        history
            .apply(
                edit(
                    "Rename",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some(name.to_owned()),
                    }],
                )
                .merging("rename"),
                world,
            )
            .unwrap();
    };

    drag(&mut history, &mut world, "First");
    drag(&mut history, &mut world, "Second");
    history.break_merge_run();
    drag(&mut history, &mut world, "Third");

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Second"));
    history.undo(&mut world).unwrap();
    assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Parent"));
    assert!(!history.can_undo());
}

#[test]
fn different_merge_keys_do_not_collapse_together() {
    let (mut world, parent, child) = world_with_two_entities();
    let mut history = CommandHistory::default();

    for (entity, key) in [(parent, "drag:parent"), (child, "drag:child")] {
        history
            .apply(
                edit(
                    "Move",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(Transform3D::default()),
                    }],
                )
                .merging(key),
                &mut world,
            )
            .unwrap();
    }

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(child).unwrap().transform_3d, None);
    assert!(history.can_undo());
    history.undo(&mut world).unwrap();
    assert_eq!(world.get(parent).unwrap().transform_3d, None);
}
