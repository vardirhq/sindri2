//! What a click does to a selection, which is the whole of the type.

use sindri_core::{EntityData, EntityId, World};

use super::{Selection, topmost};

/// Four entities in a world, and their handles in spawn order.
fn four() -> (World, Vec<EntityId>) {
    let mut world = World::default();
    let entities = (0..4).map(|_| world.spawn(EntityData::default())).collect();
    (world, entities)
}

/// An ordinary click replaces, which is what it did when a selection was one
/// entity, and what it has to keep doing.
#[test]
fn a_plain_click_replaces_whatever_was_selected() {
    let (_, ids) = four();
    let mut selection = Selection::default();
    selection.replace(Some(ids[0]));
    selection.replace(Some(ids[2]));
    assert_eq!(selection.all(), &[ids[2]]);
    assert_eq!(selection.primary(), Some(ids[2]));
    selection.replace(None);
    assert!(selection.is_empty() && selection.primary().is_none());
}

/// Ctrl-click adds, and the thing just pointed at is what the inspector shows.
#[test]
fn adding_makes_the_new_one_the_primary() {
    let (_, ids) = four();
    let mut selection = Selection::default();
    selection.replace(Some(ids[0]));
    assert!(selection.toggle(ids[3]));
    assert_eq!(selection.all(), &[ids[0], ids[3]]);
    assert_eq!(selection.primary(), Some(ids[3]));
}

/// Ctrl-clicking a selected row removes it, and the inspector falls back to
/// what was pointed at before rather than going blank because one row was
/// deselected.
#[test]
fn removing_falls_back_to_the_one_before() {
    let (_, ids) = four();
    let mut selection = Selection::default();
    selection.replace(Some(ids[0]));
    selection.toggle(ids[1]);
    selection.toggle(ids[1]);
    assert_eq!(selection.all(), &[ids[0]]);
    assert_eq!(selection.primary(), Some(ids[0]));
}

/// A range runs between two rows whichever order they are in, and the clicked
/// end is the primary either way.
#[test]
fn a_range_runs_both_ways_and_ends_where_it_was_clicked() {
    let (_, ids) = four();
    let order = ids.clone();
    let mut down = Selection::default();
    down.replace(Some(ids[0]));
    down.extend_through(ids[2], &order);
    assert_eq!(down.all(), &[ids[0], ids[1], ids[2]]);
    assert_eq!(down.primary(), Some(ids[2]));

    let mut up = Selection::default();
    up.replace(Some(ids[3]));
    up.extend_through(ids[1], &order);
    assert_eq!(up.len(), 3);
    assert!(up.contains(ids[1]) && up.contains(ids[2]) && up.contains(ids[3]));
    assert_eq!(up.primary(), Some(ids[1]));
}

/// There is no range without two ends: shift-clicking with nothing selected,
/// or from a primary the listing does not hold, is an ordinary click rather
/// than nothing at all.
#[test]
fn a_range_with_one_end_is_an_ordinary_click() {
    let (_, ids) = four();
    let mut selection = Selection::default();
    selection.extend_through(ids[2], &ids);
    assert_eq!(selection.all(), &[ids[2]]);

    // The primary is selected but filtered out of the listing, so there is no
    // row to run a range from.
    let mut filtered = Selection::default();
    filtered.replace(Some(ids[0]));
    filtered.extend_through(ids[3], &ids[2..]);
    assert_eq!(filtered.all(), &[ids[3]]);
}

/// A despawn must not leave a handle behind for a command to be aimed at.
#[test]
fn a_deleted_entity_leaves_the_selection() {
    let (mut world, ids) = four();
    let mut selection: Selection = ids.iter().copied().collect();
    world
        .despawn_recursive(ids[1])
        .expect("a spawned entity despawns");
    selection.retain_live(&world);
    assert_eq!(selection.len(), 3);
    assert!(!selection.contains(ids[1]));
    assert_eq!(selection.primary(), Some(ids[3]));
}

/// Every bulk verb takes the subtree, so a set holding a parent and its child
/// has to be folded to the parent or the child is acted on twice.
#[test]
fn a_child_of_something_selected_is_not_its_own_subject() {
    let mut world = World::default();
    let root = world.spawn(EntityData::default());
    let torso = world.spawn(EntityData::default());
    let arm = world.spawn(EntityData::default());
    let loose = world.spawn(EntityData::default());
    world.set_parent(torso, Some(root)).unwrap();
    world.set_parent(arm, Some(torso)).unwrap();

    // A grandchild folds away too, not only a direct child.
    assert_eq!(
        topmost(&world, &[root, torso, arm, loose]),
        vec![root, loose]
    );
    // A child whose parent is not in the set stays.
    assert_eq!(topmost(&world, &[arm, loose]), vec![arm, loose]);
    assert!(topmost(&world, &[]).is_empty());
}
