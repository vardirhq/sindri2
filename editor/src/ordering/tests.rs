//! Where an entity sits among its siblings, and what moving it records.

use sindri_core::{CommandHistory, EntityData, EntityId, SceneEntityId, World};

use super::{ORDER_KEY, can_move, move_by, rank, siblings};

/// Every sibling counts, which is the case everywhere but the hierarchy's top
/// level — the one place the panel draws two groups.
fn all(_: EntityId) -> bool {
    true
}

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

    let buffer = move_by(&world, children[3], -2, &all);
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
            move_by(&world, children[0], 1, &all).into_transaction("Move down"),
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
    assert!(!can_move(&world, children[0], -1, &all));
    assert!(move_by(&world, children[0], -1, &all).is_empty());
    assert!(!can_move(&world, children[3], 1, &all));
    assert!(move_by(&world, children[3], 1, &all).is_empty());
    assert!(can_move(&world, children[1], -1, &all) && can_move(&world, children[1], 1, &all));
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
            move_by(&world, roots[2], -1, &all).into_transaction("Move up"),
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
            move_by(&world, children[3], -3, &all).into_transaction("Move up"),
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

/// The hierarchy draws the top level as two groups, so "the row above this
/// one" there means the row above it in its own group. Moving past a row from
/// the other group would change the recorded order and move nothing on screen.
#[test]
fn a_move_at_the_top_level_stays_inside_its_group() {
    let mut world = World::default();
    // Interleaved by ID, which is how they sort before anyone reorders: a, b,
    // c, d — and the groups below split them into (a, c) and (b, d).
    let roots: Vec<EntityId> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|id| {
            world.spawn(EntityData {
                source_id: Some(SceneEntityId::new(id).unwrap()),
                ..EntityData::default()
            })
        })
        .collect();
    let first_group = [roots[0], roots[2]];
    let grouped = |entity: EntityId| first_group.contains(&entity);
    let mine = |entity: EntityId| grouped(entity) == grouped(roots[2]);

    // `c` is the second of its group, so it has exactly one place to go up and
    // none to go down — even though the full list has a row on either side.
    assert!(can_move(&world, roots[2], -1, &mine));
    assert!(!can_move(&world, roots[2], 1, &mine));

    let mut history = CommandHistory::default();
    history
        .apply(
            move_by(&world, roots[2], -1, &mine).into_transaction("Move up"),
            &mut world,
        )
        .unwrap();

    let order = named(&world, &siblings(&world, None));
    assert_eq!(
        order.iter().position(|id| id == "c"),
        Some(0),
        "c moved above a, which is the row above it in its group: {order:?}"
    );
    assert!(
        order.iter().position(|id| id == "c") < order.iter().position(|id| id == "a"),
        "{order:?}"
    );
}
