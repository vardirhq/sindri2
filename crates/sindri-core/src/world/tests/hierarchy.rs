//! Parentage, despawning, and the handles that survive both.

use crate::{EntityData, World, WorldError};

#[test]
fn stale_handles_do_not_access_reused_slots() {
    let mut world = World::default();
    let first = world.spawn(EntityData::default());
    world.despawn_recursive(first).unwrap();
    let second = world.spawn(EntityData::default());
    assert_eq!(first.index(), second.index());
    assert_ne!(first.generation(), second.generation());
    assert!(world.get(first).is_none());
    assert!(world.get(second).is_some());
}

#[test]
fn recursive_despawn_removes_hierarchy() {
    let mut world = World::default();
    let parent = world.spawn(EntityData::default());
    let child = world.spawn(EntityData::default());
    world.set_parent(child, Some(parent)).unwrap();
    let removed = world.despawn_recursive(parent).unwrap();
    assert_eq!(removed.len(), 2);
    assert!(world.is_empty());
}

#[test]
fn hierarchy_rejects_cycles() {
    let mut world = World::default();
    let parent = world.spawn(EntityData::default());
    let child = world.spawn(EntityData::default());
    world.set_parent(child, Some(parent)).unwrap();
    assert_eq!(
        world.set_parent(parent, Some(child)),
        Err(WorldError::HierarchyCycle)
    );
}

/// The check exists so an interface can refuse a drop before making it. It
/// is only worth having if it answers exactly what the move would, so this
/// asks both about every pair in a three-deep chain.
#[test]
fn checking_a_reparent_agrees_with_making_one() {
    let mut world = World::default();
    let root = world.spawn(EntityData::default());
    let middle = world.spawn(EntityData::default());
    let leaf = world.spawn(EntityData::default());
    world.set_parent(middle, Some(root)).unwrap();
    world.set_parent(leaf, Some(middle)).unwrap();
    let stale = world.spawn(EntityData::default());
    world.despawn_recursive(stale).unwrap();

    let entities = [root, middle, leaf, stale];
    for child in entities {
        for parent in entities.map(Some).into_iter().chain([None]) {
            let checked = world.check_set_parent(child, parent);
            let mut copy = world.clone();
            let made = copy.set_parent(child, parent);
            assert_eq!(
                checked, made,
                "check and move disagreed about {child:?} under {parent:?}"
            );
        }
    }
}
