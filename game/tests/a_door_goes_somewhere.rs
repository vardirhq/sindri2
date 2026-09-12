//! Walking through a door goes somewhere, and coming back finds it as it was.
//!
//! The integration test for multiple scenes. Every piece below is asserted
//! elsewhere on its own; this is the one that runs them together in the real
//! session, because that is where the pieces disagree if they are going to.

use sindri_core::World;
use sindri_gather::{Session, extractor, scenes, world};
use sindri_platform::{InputEvent, InputState, Key};
use sindri_scene::CameraComponent;

const STEP: f32 = 1.0 / 60.0;
const VIEWPORT: (f32, f32) = (960.0, 600.0);

fn opened() -> (World, Session) {
    let (world, loaded) = world().expect("the project opens");
    let extractor = extractor().expect("the schemas register");
    let session = Session::new(extractor.components().clone())
        .with_scenes(scenes().expect("the scenes parse"), loaded);
    (world, session)
}

fn hold(key: Option<Key>) -> InputState {
    let mut held = InputState::default();
    if let Some(key) = key {
        held.apply(InputEvent::KeyPressed(key));
    }
    held
}

/// Walks until the scene changes, or gives up.
fn walk_until_elsewhere(world: &mut World, session: &mut Session, key: Key, limit: u32) -> bool {
    let from = session.scene().map(str::to_owned);
    for _ in 0..limit {
        session
            .step(world, &hold(Some(key)), VIEWPORT, STEP)
            .expect("the game steps");
        if session.scene().map(str::to_owned) != from {
            return true;
        }
    }
    false
}

#[test]
fn the_game_opens_on_its_main_scene() {
    let (_world, session) = opened();
    assert_eq!(session.scene(), Some("gather.scene.json"));
}

#[test]
fn walking_into_the_door_goes_to_the_shed() {
    let (mut world, mut session) = opened();
    // The door sits south of the plaza, so walking down reaches it.
    assert!(
        walk_until_elsewhere(&mut world, &mut session, Key::ArrowDown, 600),
        "the player never reached the door"
    );
    assert_eq!(session.scene(), Some("shed.scene.json"));
}

fn active_cameras(world: &World) -> Vec<CameraComponent> {
    let extractor = extractor().expect("the schemas register");
    extractor
        .components()
        .query::<CameraComponent>(world)
        .expect("camera payloads read")
        .into_iter()
        .filter(|(entity, _)| world.is_active(*entity))
        .map(|(_, camera)| camera)
        .collect()
}

fn assert_one_parallel_camera(world: &World) {
    let cameras = active_cameras(world);
    assert_eq!(cameras.len(), 1, "a playable place has one active camera");
    assert!(
        matches!(cameras[0], CameraComponent::Orthographic { .. }),
        "baked ground and props need parallel projection so their hidden Z \n+         separation cannot become parallax"
    );
}

/// Scene switching must leave a renderable world, not merely a world whose
/// scripts still step. The first interior had no camera, which the door tests
/// could not see because they never extracted a frame.
#[test]
fn every_place_has_exactly_one_parallel_world_camera() {
    let (mut world, mut session) = opened();
    assert_one_parallel_camera(&world);

    assert!(walk_until_elsewhere(
        &mut world,
        &mut session,
        Key::ArrowDown,
        600
    ));
    assert_eq!(session.scene(), Some("shed.scene.json"));
    assert_one_parallel_camera(&world);

    assert!(walk_until_elsewhere(
        &mut world,
        &mut session,
        Key::ArrowUp,
        600
    ));
    assert_eq!(session.scene(), Some("gather.scene.json"));
    assert_one_parallel_camera(&world);
}

/// The property the whole design rests on, in the running game rather than in
/// a unit test: the island is switched off, not unloaded, so what was true out
/// there is still true when the player comes back.
///
/// Witnessed with a mark written straight into the world rather than with the
/// player's position, because arriving through a door deliberately moves the
/// player — so position is the one thing that is *supposed* to change.
#[test]
fn coming_back_finds_the_island_as_it_was() {
    let (mut world, mut session) = opened();
    let island_player = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Player"))
        .map(|(entity, _)| entity)
        .expect("the island has a player");

    // Something only the played world knows, which no scene file says.
    world
        .get_mut(island_player)
        .expect("the player")
        .components
        .insert("was-here".to_owned(), serde_json::json!(true));

    assert!(
        walk_until_elsewhere(&mut world, &mut session, Key::ArrowDown, 600),
        "the player never reached the door"
    );
    assert_eq!(session.scene(), Some("shed.scene.json"));
    assert!(
        !world.is_active(island_player),
        "the island is out of play while indoors"
    );
    assert!(
        world.get(island_player).is_some(),
        "and still in the world, which is the difference between switching a \
         scene off and unloading it"
    );

    // Back out through the shed's own door. Arriving put the player north-east
    // of it, so the way out is up.
    assert!(
        walk_until_elsewhere(&mut world, &mut session, Key::ArrowUp, 600),
        "the player never found the way out"
    );
    assert_eq!(session.scene(), Some("gather.scene.json"));
    assert!(
        world.is_active(island_player),
        "the island is in play again"
    );
    assert_eq!(
        world
            .get(island_player)
            .expect("the player")
            .components
            .get("was-here"),
        Some(&serde_json::json!(true)),
        "the island was reloaded from its file instead of being switched back on"
    );
}
