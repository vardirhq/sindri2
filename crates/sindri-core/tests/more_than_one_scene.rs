//! Several scenes can be live in one world at once.
//!
//! A game with a farm and a farmhouse wants both loaded: walking indoors has to
//! leave the crops growing, and reloading the farm from its file on the way back
//! out would reset them, because a scene file holds the *authored* state and not
//! the *played* one. So a scene is added beside the others and switched off,
//! rather than loaded and dropped.
//!
//! What has to hold for that to work is mostly about identity. Scene IDs are
//! unique inside one file and nowhere else, so two interiors may each author a
//! `door`, and the world answers for all of them at once.

use serde_json::json;
use sindri_core::{SceneDocument, SceneEntityId, World};

/// A scene with a root, a child under it, and a second root beside them.
fn cottage(door: &str) -> SceneDocument {
    SceneDocument::from_json(
        &json!({
            "format_version": 9,
            "entities": [
                { "id": "room", "name": "Room" },
                { "id": door, "name": "Door", "parent": "room" },
                { "id": "lamp", "name": "Lamp" },
            ],
        })
        .to_string(),
    )
    .unwrap_or_else(|error| panic!("the scene parses: {error}"))
}

fn id(text: &str) -> SceneEntityId {
    SceneEntityId::new(text.to_owned()).expect("a usable id")
}

#[test]
fn a_scene_can_be_added_to_a_world_that_already_holds_one() {
    let mut world = World::from_scene(&cottage("door"))
        .expect("the first scene loads")
        .world;
    let before = world.len();

    let added = world
        .add_scene(&cottage("door"), "barn", None)
        .expect("a second scene loads beside the first");

    assert_eq!(world.len(), before * 2, "both scenes are in the world");
    assert_eq!(added.entity_map.len(), 3);
    assert_eq!(
        added.roots.len(),
        2,
        "`room` and `lamp` were authored at the root"
    );
}

/// The point of the parent: one switch takes a whole scene out of play, because
/// `is_active` walks ancestors.
#[test]
fn switching_off_the_root_switches_off_the_whole_scene() {
    let mut world = World::default();
    let holder = world.spawn(sindri_core::EntityData {
        name: Some("Barn".to_owned()),
        ..sindri_core::EntityData::default()
    });
    let added = world
        .add_scene(&cottage("door"), "barn", Some(holder))
        .expect("the scene loads under the holder");

    for entity in added.entity_map.values() {
        assert!(world.is_active(*entity), "everything starts in play");
    }

    world.get_mut(holder).expect("the holder").disabled = true;

    for (scene_id, entity) in &added.entity_map {
        assert!(
            !world.is_active(*entity),
            "{} should be out of play with its scene",
            scene_id.as_str()
        );
    }
}

/// Two scenes may each author the same ID. The world answers for all of them at
/// once, so the IDs it holds are namespaced on the way in.
#[test]
fn two_scenes_may_each_author_the_same_id() {
    let mut world = World::default();
    let house = world
        .add_scene(&cottage("door"), "house", None)
        .expect("the house loads");
    let barn = world
        .add_scene(&cottage("door"), "barn", None)
        .expect("the barn loads beside it");

    assert_ne!(
        house.entity_map[&id("door")],
        barn.entity_map[&id("door")],
        "each scene's door is its own entity"
    );
    assert_eq!(house.source_ids[&id("door")], id("house/door"));
    assert_eq!(barn.source_ids[&id("door")], id("barn/door"));
    assert_eq!(
        world.entity_for_source_id(&id("house/door")),
        Some(house.entity_map[&id("door")]),
        "and is reachable by the identity the world now holds"
    );
}

/// The map is keyed by the ID the *file* spells, because that is the one a
/// caller reading its own scene knows.
#[test]
fn the_map_is_keyed_by_the_scene_s_own_ids() {
    let mut world = World::default();
    let added = world
        .add_scene(&cottage("door"), "house", None)
        .expect("the scene loads");
    assert!(added.entity_map.contains_key(&id("door")));
    assert!(!added.entity_map.contains_key(&id("house/door")));
}

/// Giving two scenes one namespace is the caller's mistake, and it is refused
/// rather than resolved into an ambiguity nobody can see.
#[test]
fn the_same_namespace_twice_is_refused() {
    let mut world = World::default();
    world
        .add_scene(&cottage("door"), "house", None)
        .expect("the first load");
    let error = world
        .add_scene(&cottage("door"), "house", None)
        .expect_err("the second should collide");
    // Whichever identity it reaches first, it has to name it: "that scene is
    // already loaded" is the one thing the caller cannot work out alone.
    let reported = format!("{error}");
    assert!(
        reported.contains("house/"),
        "the error should name the identity that collided: {reported}"
    );
}

/// And a refused load leaves nothing behind. Identities are all resolved before
/// anything is spawned for exactly this reason.
#[test]
fn a_refused_load_adds_nothing() {
    let mut world = World::default();
    world
        .add_scene(&cottage("door"), "house", None)
        .expect("the first load");
    let before = world.len();
    let _ = world.add_scene(&cottage("door"), "house", None);
    assert_eq!(
        world.len(),
        before,
        "a collision should leave the world exactly as it was"
    );
}

#[test]
fn parents_inside_the_scene_are_preserved() {
    let mut world = World::default();
    let added = world
        .add_scene(&cottage("door"), "house", None)
        .expect("the scene loads");
    let door = added.entity_map[&id("door")];
    let room = added.entity_map[&id("room")];
    assert_eq!(
        world.get(door).expect("the door").parent,
        Some(room),
        "a child authored under a root stays under it"
    );
    assert!(
        !added.roots.contains(&door),
        "and is not reported as a root of its own"
    );
}

#[test]
fn loading_under_an_entity_the_world_lost_is_refused() {
    let mut world = World::default();
    let gone = world.spawn(sindri_core::EntityData::default());
    world.despawn_recursive(gone).expect("it despawns");
    assert!(
        world
            .add_scene(&cottage("door"), "house", Some(gone))
            .is_err(),
        "a parent that is not there is not a parent"
    );
}
