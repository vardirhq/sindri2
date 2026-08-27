//! The rows the hierarchy shows, and what reparenting does to them.

use std::collections::BTreeSet;

use sindri_core::{CommandBuffer, CommandHistory, EntityData, SceneEntityId, World, WorldCommand};

use super::super::editing::duplicate::duplicate_commands;
use super::super::editing::{find_by_source_id, next_game_object_id, reparent_choices};
use super::super::hierarchy::row::{
    component_label, entity_name, hierarchy_drop_allowed, humanize,
};
use crate::space::EntitySpace;

use super::super::hierarchy::rows::{hierarchy_rows, visible_hierarchy_rows};
use super::support::*;

#[test]
fn hierarchy_rows_nest_children_under_their_parents() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let rows: Vec<(String, usize)> = hierarchy_rows(&world)
        .into_iter()
        .map(|(entity, depth)| (entity_name(world.get(entity).unwrap()), depth))
        .collect();

    assert_eq!(
        rows,
        vec![
            ("Root".to_owned(), 0),
            // Siblings follow stable-ID order, matching the saved file.
            ("Leg".to_owned(), 1),
            ("Torso".to_owned(), 1),
            ("Arm".to_owned(), 2),
        ]
    );
}

#[test]
fn every_entity_appears_exactly_once_in_the_hierarchy() {
    let world = demo_world();
    let rows = hierarchy_rows(&world);
    assert_eq!(rows.len(), world.len());
    let mut entities: Vec<_> = rows.iter().map(|(entity, _)| *entity).collect();
    entities.sort_by_key(|entity| entity.index());
    entities.dedup();
    assert_eq!(entities.len(), world.len());
}

#[test]
fn collapsing_a_game_object_hides_its_whole_subtree() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let root = find_by_source_id(&world, "root").unwrap();
    let rows = visible_hierarchy_rows(&world, &BTreeSet::from([root]), "", EntitySpace::World);
    assert_eq!(rows, vec![(root, 0)]);
}

#[test]
fn hierarchy_search_keeps_the_ancestor_path_visible() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let named = |id: &str| find_by_source_id(&world, id).unwrap();
    let collapsed = BTreeSet::from([named("root"), named("torso")]);
    let rows = visible_hierarchy_rows(&world, &collapsed, "arm", EntitySpace::World);

    assert_eq!(
        rows,
        vec![(named("root"), 0), (named("torso"), 1), (named("arm"), 2)],
        "search opens only the path to the match without changing stored folds"
    );
}

#[test]
fn new_game_object_ids_are_stable_and_skip_existing_ids() {
    let mut world = World::default();
    world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("game-object-1").unwrap()),
        ..EntityData::default()
    });
    assert_eq!(next_game_object_id(&world).as_str(), "game-object-2");
}

/// The parent menu must not offer a move the command layer would refuse,
/// which for an ancestor means none of its own descendants.
#[test]
fn the_parent_menu_never_offers_a_move_that_would_make_a_cycle() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let named = |id: &str| find_by_source_id(&world, id).unwrap();
    let offered: Vec<String> = reparent_choices(&world, named("torso"))
        .into_iter()
        .map(|(_, name)| name)
        .collect();

    assert_eq!(
        offered,
        vec!["Root".to_owned(), "Leg".to_owned()],
        "torso may move to the root or under its sibling, but not under \
         itself or its own child"
    );
}

/// The selected-parent label is looked up in this list, so a parent missing
/// from it would be drawn as though the entity sat at the root.
#[test]
fn the_parent_menu_always_contains_the_parent_an_entity_already_has() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    for (entity, data) in world.entities() {
        let Some(parent) = data.parent else {
            continue;
        };
        assert!(
            reparent_choices(&world, entity)
                .iter()
                .any(|(candidate, _)| *candidate == parent),
            "{} sits under a parent its own menu does not list",
            entity_name(data)
        );
    }
}

/// A leaf can go anywhere, so this is the case that would hide a filter
/// that was accidentally excluding legal parents.
#[test]
fn a_leaf_may_move_under_anything_but_itself() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let arm = find_by_source_id(&world, "arm").unwrap();
    let offered: Vec<String> = reparent_choices(&world, arm)
        .into_iter()
        .map(|(_, name)| name)
        .collect();

    assert_eq!(
        offered,
        vec!["Root".to_owned(), "Leg".to_owned(), "Torso".to_owned()]
    );
}

#[test]
fn reparenting_moves_the_entity_and_undoes_in_one_step() {
    let mut world = World::from_scene(&nested_scene()).unwrap().world;
    let arm = find_by_source_id(&world, "arm").unwrap();
    let leg = find_by_source_id(&world, "leg").unwrap();
    let torso = world.get(arm).unwrap().parent.unwrap();

    let mut history = CommandHistory::default();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::SetParent {
        entity: arm,
        parent: Some(leg),
    });
    history
        .apply(buffer.into_transaction("Reparent entity"), &mut world)
        .unwrap();

    assert_eq!(world.get(arm).unwrap().parent, Some(leg));
    assert!(
        !world.get(torso).unwrap().children.contains(&arm),
        "the old parent should no longer claim the child"
    );

    history.undo(&mut world).unwrap();
    assert_eq!(world.get(arm).unwrap().parent, Some(torso));
    assert!(world.get(torso).unwrap().children.contains(&arm));
}

#[test]
fn hierarchy_drop_rules_allow_moves_but_reject_noops_and_cycles() {
    let world = World::from_scene(&nested_scene()).unwrap().world;
    let named = |id: &str| find_by_source_id(&world, id).unwrap();
    let root = named("root");
    let torso = named("torso");
    let arm = named("arm");
    let leg = named("leg");

    assert!(hierarchy_drop_allowed(&world, arm, Some(leg)));
    assert!(hierarchy_drop_allowed(&world, arm, None));
    assert!(!hierarchy_drop_allowed(&world, arm, Some(torso)));
    assert!(!hierarchy_drop_allowed(&world, arm, Some(arm)));
    assert!(!hierarchy_drop_allowed(&world, root, Some(arm)));
    assert!(!hierarchy_drop_allowed(&world, root, None));
}

#[test]
fn labels_are_human_readable() {
    assert_eq!(humanize("checker-cube"), "Checker Cube");
    assert_eq!(component_label("sindri.sprite"), "Sprite");
    assert_eq!(
        component_label("sindri.ui.image"),
        "UI Image",
        "a word people write in capitals is not a typo in the panel"
    );
}

/// The hierarchy is two lists, and which list an entity is in comes from what
/// it carries rather than from anything an author has to maintain.
#[test]
fn the_hierarchy_lists_world_entities_and_ui_entities_apart() {
    let mut world = World::default();
    let prop = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("prop").unwrap()),
        components: [("sindri.sprite".to_owned(), serde_json::json!({}))]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let pip = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("pip").unwrap()),
        components: [("sindri.ui.image".to_owned(), serde_json::json!({}))]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let empty = world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("empty").unwrap()),
        ..EntityData::default()
    });

    let listed = |space| -> Vec<sindri_core::EntityId> {
        visible_hierarchy_rows(&world, &BTreeSet::new(), "", space)
            .into_iter()
            .map(|(entity, _)| entity)
            .collect()
    };
    assert_eq!(listed(EntitySpace::World), vec![empty, prop]);
    assert_eq!(listed(EntitySpace::Ui), vec![pip]);
}

/// Duplicating copies the whole subtree, keeps the original's parent, and
/// undoes in one step.
///
/// The handles are the interesting part: `WorldCommand::Spawn` names the
/// handle it spawns at, and `World::next_handle` is a peek rather than an
/// allocation, so a transaction that spawns four entities cannot ask for four
/// handles by asking four times. The copy is rehearsed on a clone for exactly
/// that reason, and this is what says the rehearsal matched.
#[test]
fn duplicating_an_entity_copies_everything_under_it() {
    let mut world = World::from_scene(&nested_scene()).unwrap().world;
    let torso = find_by_source_id(&world, "torso").unwrap();
    let root = find_by_source_id(&world, "root").unwrap();
    let before = world.len();

    let (buffer, copy) = duplicate_commands(&world, torso);
    let copy = copy.unwrap();
    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Duplicate entity"), &mut world)
        .unwrap();

    assert_eq!(world.len(), before + 2, "the torso and its arm");
    let copied = world.get(copy).unwrap();
    assert_eq!(copied.parent, Some(root), "a duplicate is a sibling");
    assert_eq!(copied.children.len(), 1, "and brought its arm with it");
    let child = world.get(copied.children[0]).unwrap();
    assert_eq!(
        child.parent,
        Some(copy),
        "which hangs off the copy, not the original"
    );
    assert!(
        world.get(torso).unwrap().children.len() == 1,
        "and the original kept exactly its own"
    );

    history.undo(&mut world).unwrap();
    assert_eq!(world.len(), before, "one undo takes the whole copy back");
}

/// Every copy earns a stable ID nothing else is using.
///
/// A stable ID is what `sindri.grid.occupant` names and what sibling order is
/// sorted by, so two entities sharing one is not a cosmetic collision.
#[test]
fn a_duplicate_gets_an_unused_stable_id() {
    let mut world = World::from_scene(&nested_scene()).unwrap().world;
    let leg = find_by_source_id(&world, "leg").unwrap();
    let mut history = CommandHistory::default();

    let mut ids = Vec::new();
    for _ in 0..3 {
        let (buffer, copy) = duplicate_commands(&world, leg);
        history
            .apply(buffer.into_transaction("Duplicate entity"), &mut world)
            .unwrap();
        ids.push(
            world
                .get(copy.unwrap())
                .unwrap()
                .source_id
                .as_ref()
                .unwrap()
                .as_str()
                .to_owned(),
        );
    }

    assert_eq!(ids, vec!["leg-copy", "leg-copy-2", "leg-copy-3"]);
}

/// A handle that names nothing produces no commands, rather than a transaction
/// that spawns an empty entity.
#[test]
fn duplicating_nothing_asks_for_nothing() {
    let mut world = World::from_scene(&nested_scene()).unwrap().world;
    let leg = find_by_source_id(&world, "leg").unwrap();
    let mut buffer = CommandBuffer::new();
    buffer.push(WorldCommand::Despawn { entity: leg });
    CommandHistory::default()
        .apply(buffer.into_transaction("Delete entity"), &mut world)
        .unwrap();

    let (buffer, copy) = duplicate_commands(&world, leg);
    assert!(buffer.is_empty() && copy.is_none());
}
