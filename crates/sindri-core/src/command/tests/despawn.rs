//! Despawning, and the claim that undoing one hands back the same handle.

use crate::{CommandBuffer, CommandHistory, EntityData, World, WorldCommand, WorldError};

/// The claim the whole design rests on: undoing a despawn hands back the
/// *same* handle, so the selection and every earlier command naming it are
/// still pointing at the entity they named.
///
/// A generation-checked handle normally changes when a slot is reused, and
/// a respawn that handed back a new one would quietly invalidate the rest
/// of the history.
#[test]
fn undoing_a_despawn_gives_back_the_same_handle() {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        name: Some("Doomed".to_owned()),
        ..EntityData::default()
    });

    let mut history = CommandHistory::default();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::Despawn { entity });
    history
        .apply(buffer.into_transaction("Delete"), &mut world)
        .unwrap();
    assert!(world.get(entity).is_none());

    history.undo(&mut world).unwrap();
    assert_eq!(
        world.get(entity).and_then(|data| data.name.as_deref()),
        Some("Doomed"),
        "the same handle finds the same entity"
    );

    // And redo takes it away again, so the pair is total rather than
    // one-way.
    history.redo(&mut world).unwrap();
    assert!(world.get(entity).is_none());
}

/// The slot being free when undo reaches a despawn is not luck: the
/// history undoes in order, so everything that could have taken the slot
/// has already been undone. This is that argument, executed.
#[test]
fn a_slot_reused_after_a_despawn_is_free_again_by_the_time_undo_needs_it() {
    let mut world = World::default();
    let first = world.spawn(EntityData {
        name: Some("First".to_owned()),
        ..EntityData::default()
    });

    let mut history = CommandHistory::default();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::Despawn { entity: first });
    history
        .apply(buffer.into_transaction("Delete"), &mut world)
        .unwrap();

    // A second entity takes the freed slot, with a new generation.
    let second = world.next_handle();
    assert_eq!(second.index(), first.index(), "the same slot");
    assert_ne!(second, first, "but not the same handle");
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::Spawn {
        entity: second,
        data: Box::new(EntityData {
            name: Some("Second".to_owned()),
            ..EntityData::default()
        }),
    });
    history
        .apply(buffer.into_transaction("Create"), &mut world)
        .unwrap();

    history.undo(&mut world).unwrap();
    history.undo(&mut world).unwrap();

    assert_eq!(
        world.get(first).and_then(|data| data.name.as_deref()),
        Some("First"),
        "the first entity is back at its own handle"
    );
    assert!(
        world.get(second).is_none(),
        "and the one that had borrowed its slot is gone"
    );
}

/// Deleting a parent takes its children, and undoing brings the whole
/// subtree back — with its links and its place among its siblings.
#[test]
fn undoing_a_despawn_restores_the_whole_subtree_in_place() {
    let mut world = World::default();
    let root = world.spawn(EntityData::default());
    let first = world.spawn(EntityData::default());
    let doomed = world.spawn(EntityData {
        name: Some("Doomed".to_owned()),
        ..EntityData::default()
    });
    let last = world.spawn(EntityData::default());
    let child = world.spawn(EntityData {
        name: Some("Child".to_owned()),
        ..EntityData::default()
    });
    for entity in [first, doomed, last] {
        world.set_parent(entity, Some(root)).unwrap();
    }
    world.set_parent(child, Some(doomed)).unwrap();
    let before = world.get(root).unwrap().children.clone();

    let mut history = CommandHistory::default();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::Despawn { entity: doomed });
    history
        .apply(buffer.into_transaction("Delete"), &mut world)
        .unwrap();
    assert!(world.get(child).is_none(), "a child goes with its parent");
    assert_eq!(world.get(root).unwrap().children, vec![first, last]);

    history.undo(&mut world).unwrap();
    assert_eq!(
        world.get(root).unwrap().children,
        before,
        "and comes back between the siblings it was between, not at the end"
    );
    assert_eq!(
        world.get(child).and_then(|data| data.name.as_deref()),
        Some("Child")
    );
    assert_eq!(world.get(child).unwrap().parent, Some(doomed));
}

/// Spawning at an occupied handle is refused rather than overwriting what
/// is there. Nothing should reach it, and that is exactly why it is checked.
#[test]
fn spawning_onto_a_live_entity_is_refused() {
    let mut world = World::default();
    let entity = world.spawn(EntityData::default());
    assert!(matches!(
        world.spawn_at(entity, EntityData::default()),
        Err(WorldError::SlotOccupied(_))
    ));
}
