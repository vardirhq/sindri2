//! Where an entity sits among its siblings, and what moving it records.

use sindri_core::{CommandHistory, EntityData, EntityId, SceneEntityId, World};

use super::{ORDER_KEY, can_move, move_by, rank, siblings};

/// Four children of one parent, named in the order their IDs sort in.
fn family() -> (World, EntityId, Vec<EntityId>) {
    let mut world = World::default();
    let parent = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("root").unwrap()),
        ..EntityData::default()
    });
    let children: Vec<EntityId> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|id| {
            let child = world.spawn(EntityData {
                source_id: Some(SceneEntityId::new(id).unwrap()),
                ..EntityData::default()
            });
            world.set_parent(child, Some(parent)).unwrap();
            child
        })
        .collect();
    (world, parent, children)
}

fn named(world: &World, entities: &[EntityId]) -> Vec<String> {
    entities
        .iter()
        .map(|entity| {
            world
                .get(*entity)
                .unwrap()
                .source_id
                .as_ref()
                .unwrap()
                .as_str()
                .to_owned()
        })
        .collect()
}

/// A scene nobody has reordered looks exactly as it did: siblings in stable-ID
/// order, which is what the hierarchy sorted by before there was an order at
/// all.
#[test]
fn an_unordered_family_is_still_alphabetical() {
    let (world, parent, _) = family();
    assert_eq!(
        named(&world, &siblings(&world, Some(parent))),
        ["a", "b", "c", "d"]
    );
}

/// Moving one entity stamps every sibling, so the recorded order and the drawn
/// order are the same list rather than half a list and an alphabet.
#[test]
fn a_move_records_the_whole_list() {
    let (mut world, parent, children) = family();
    let mut history = CommandHistory::default();

    let buffer = move_by(&world, children[3], -2);
    assert_eq!(
        buffer.len(),
        4,
        "every sibling is placed, not only the two that swapped"
    );
    history
        .apply(buffer.into_transaction("Move up"), &mut world)
        .unwrap();

    assert_eq!(
        named(&world, &siblings(&world, Some(parent))),
        ["a", "d", "b", "c"]
    );
    assert_eq!(rank(world.get(children[3]).unwrap()), Some(1));
}

/// And it undoes, because the order is saved with the document.
#[test]
fn a_move_undoes() {
    let (mut world, parent, children) = family();
    let mut history = CommandHistory::default();
    history
        .apply(
            move_by(&world, children[0], 1).into_transaction("Move down"),
            &mut world,
        )
        .unwrap();
    assert_eq!(
        named(&world, &siblings(&world, Some(parent))),
        ["b", "a", "c", "d"]
    );

    history.undo(&mut world).unwrap();
    assert_eq!(
        named(&world, &siblings(&world, Some(parent))),
        ["a", "b", "c", "d"]
    );
    assert!(
        !world
            .get(children[0])
            .unwrap()
            .editor
            .contains_key(ORDER_KEY),
        "and leaves no order behind on a list that never had one"
    );
}

/// The ends of a list are the ends: a move off either one is refused rather
/// than wrapping around, and the menu can grey the entry out beforehand.
#[test]
fn the_ends_of_a_list_refuse_rather_than_wrap() {
    let (world, _, children) = family();
    assert!(!can_move(&world, children[0], -1));
    assert!(move_by(&world, children[0], -1).is_empty());
    assert!(!can_move(&world, children[3], 1));
    assert!(move_by(&world, children[3], 1).is_empty());
    assert!(can_move(&world, children[1], -1) && can_move(&world, children[1], 1));
}

/// Top-level entities are siblings too, and there is no parent to read their
/// list off.
#[test]
fn the_top_level_reorders_like_any_other_parent() {
    let mut world = World::default();
    let roots: Vec<EntityId> = ["x", "y", "z"]
        .into_iter()
        .map(|id| {
            world.spawn(EntityData {
                source_id: Some(SceneEntityId::new(id).unwrap()),
                ..EntityData::default()
            })
        })
        .collect();

    let mut history = CommandHistory::default();
    history
        .apply(
            move_by(&world, roots[2], -1).into_transaction("Move up"),
            &mut world,
        )
        .unwrap();
    assert_eq!(named(&world, &siblings(&world, None)), ["x", "z", "y"]);
}

/// An entity created after a reorder has no recorded place, and arrives at the
/// bottom of its parent's list — where a new thing belongs and where the eye
/// looks for it.
#[test]
fn a_newcomer_arrives_at_the_bottom() {
    let (mut world, parent, children) = family();
    let mut history = CommandHistory::default();
    history
        .apply(
            move_by(&world, children[3], -3).into_transaction("Move up"),
            &mut world,
        )
        .unwrap();

    let newcomer = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("aardvark").unwrap()),
        ..EntityData::default()
    });
    world.set_parent(newcomer, Some(parent)).unwrap();

    assert_eq!(
        named(&world, &siblings(&world, Some(parent))),
        ["d", "a", "b", "c", "aardvark"],
        "even one whose ID sorts first"
    );
}
