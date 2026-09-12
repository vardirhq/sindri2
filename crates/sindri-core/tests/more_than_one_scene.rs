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
use sindri_core::{LoadedScenes, SceneDocument, SceneEntityId, SceneSwitchError, World};

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

// --------------------------------------------------------------------------
// Which scene is being played is a game's idea rather than a world's, so it is
// kept beside the world rather than in it.

/// The property the whole design rests on: a scene you walk out of and back
/// into is the scene you left, not the scene its file describes.
#[test]
fn leaving_a_scene_and_returning_finds_it_as_it_was() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();

    scenes
        .enter(&mut world, "farm", &cottage("door"))
        .expect("the farm opens");
    let lamp = scenes
        .root("farm")
        .and_then(|_| world.entity_for_source_id(&id("farm/lamp")))
        .expect("the farm's lamp");

    // Something the player did, which is in the world and not in the file.
    world
        .get_mut(lamp)
        .expect("the lamp")
        .components
        .insert("lit".to_owned(), json!(true));

    scenes
        .enter(&mut world, "house", &cottage("door"))
        .expect("the house opens");
    assert!(
        !world.is_active(lamp),
        "the farm is out of play while indoors"
    );

    scenes.go_to(&mut world, "farm").expect("back outside");
    assert!(world.is_active(lamp), "and in play again");
    assert_eq!(
        world.get(lamp).expect("the lamp").components.get("lit"),
        Some(&json!(true)),
        "what the player changed is still changed"
    );
}

#[test]
fn exactly_one_scene_is_in_play() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    for name in ["farm", "house", "barn"] {
        scenes
            .enter(&mut world, name, &cottage("door"))
            .expect("it opens");
    }
    assert_eq!(scenes.active(), Some("barn"));
    for name in ["farm", "house"] {
        let root = scenes.root(name).expect("a root");
        assert!(!world.is_active(root), "{name} should be out of play");
    }
    assert!(world.is_active(scenes.root("barn").expect("a root")));
}

/// A scene arrives switched off. Live-on-arrival would draw it over whatever is
/// being played for the frame between loading and switching.
#[test]
fn a_loaded_scene_is_not_a_played_one() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    let root = scenes
        .load(&mut world, "barn", &cottage("door"))
        .expect("it loads");
    assert!(scenes.holds("barn"));
    assert_eq!(scenes.active(), None, "loading is not entering");
    assert!(!world.is_active(root));
}

#[test]
fn loading_the_same_scene_twice_is_the_same_scene() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    let first = scenes
        .load(&mut world, "barn", &cottage("door"))
        .expect("it loads");
    let before = world.len();
    let again = scenes
        .load(&mut world, "barn", &cottage("door"))
        .expect("it is already there");
    assert_eq!(first, again);
    assert_eq!(world.len(), before, "and was not loaded a second time");
}

#[test]
fn going_somewhere_that_was_never_loaded_says_so() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    assert_eq!(
        scenes.go_to(&mut world, "attic"),
        Err(SceneSwitchError::NotLoaded("attic".to_owned()))
    );
}

/// A name that half-exists is worse than one that does not: `holds` would
/// answer yes for a scene with nothing in it.
#[test]
fn a_scene_that_fails_to_load_is_not_remembered() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    let before = world.len();
    // The same namespace twice is refused by `add_scene`, which is the easiest
    // way to make a load fail without an invalid document.
    world
        .add_scene(&cottage("door"), "barn", None)
        .expect("something already holds that namespace");
    let held = world.len();
    assert!(scenes.load(&mut world, "barn", &cottage("door")).is_err());
    assert!(!scenes.holds("barn"));
    assert_eq!(
        world.len(),
        held,
        "and left nothing behind, including the root it would have used"
    );
    assert!(held > before);
}

/// Unloading is the one verb that throws played state away, which is why it is
/// not what leaving a scene does.
#[test]
fn unloading_takes_a_scene_out_of_the_world() {
    let mut world = World::default();
    let mut scenes = LoadedScenes::new();
    scenes
        .enter(&mut world, "barn", &cottage("door"))
        .expect("it opens");
    let root = scenes.root("barn").expect("a root");
    assert!(scenes.unload(&mut world, "barn").expect("it unloads"));
    assert!(!scenes.holds("barn"));
    assert_eq!(scenes.active(), None, "and is no longer being played");
    assert!(world.get(root).is_none(), "its entities are gone");
    assert!(
        !scenes.unload(&mut world, "barn").expect("a second unload"),
        "unloading what is not there is not an error"
    );
}
