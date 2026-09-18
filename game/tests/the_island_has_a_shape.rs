//! The ground stops the player, not just the props on it.
//!
//! Gather tagged every solid thing and compared positions, which is the only
//! thing a script could do: occupancy was the engine's answer and a script
//! could ask only about a route between entities. Neither the pond nor the
//! outcrop is an entity, so the player walked over water and into the hill
//! while the Wisp — reading the same walkable surface from Rust — went round
//! them. These stand where a player stands and ask the ground itself.

use sindri_core::{Transform3D, World};
use sindri_decay::{ScriptFrame, Scripts};
use sindri_gather::{bind_tile_sets, extractor, sources, world};
use sindri_grid::{GridPoint, GridSpace, PlanePoint};
use sindri_platform::{InputEvent, InputState, Key};
use sindri_scene::{
    SceneExtractor, TileGridComponent, TileSurfaces, TileVolumeComponent, nearest_cell,
};

fn logical_position(grid: GridSpace, map: Transform3D, world: [f32; 3]) -> GridPoint {
    let (sin, cos) = map.rotation_z_radians().sin_cos();
    let x = world[0] - map.position[0];
    let y = world[1] - map.position[1];
    let local = PlanePoint::new(
        f64::from((cos * x + sin * y) / map.scale[0]),
        f64::from((-sin * x + cos * y) / map.scale[1]),
    );
    grid.unproject(local).expect("the point unprojects")
}

fn floor_grid(world: &World, extractor: &SceneExtractor) -> (Transform3D, GridSpace) {
    let (floor, grid) = extractor
        .components()
        .query::<TileGridComponent>(world)
        .expect("the tile grid schema reads")
        .into_iter()
        .next()
        .expect("Gather has a floor");
    let map = world
        .get(floor)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default();
    (map, grid.grid_space().expect("the floor has a valid grid"))
}

/// The walkable tops of Gather's own island.
fn surfaces(world: &World, extractor: &SceneExtractor) -> TileSurfaces {
    let tile_sets = bind_tile_sets().expect("the tile sets decode");
    let (entity, volume) = extractor
        .components()
        .query::<TileVolumeComponent>(world)
        .expect("the volume schema reads")
        .into_iter()
        .next()
        .expect("Gather's floor is a volume");
    let _ = entity;
    let tile_set = tile_sets
        .get(&volume.tileset)
        .expect("Gather binds its own tile set");
    TileSurfaces::derive(&volume, tile_set).expect("the island reduces to surfaces")
}

fn player_of(world: &World) -> sindri_core::EntityId {
    world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "player")
        })
        .map(|(entity, _)| entity)
        .expect("the scene names the player")
}

#[test]
fn the_player_starts_on_ground_it_can_stand_on() {
    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let (map, grid) = floor_grid(&world, &extractor);
    let ground = surfaces(&world, &extractor);
    let player = player_of(&world);

    // One frame, which is all `start` needs. Before it there was no spawn
    // point at all: the player's transform was the world origin, and the world
    // origin is the middle of a 25x25 island, and the middle of the island is
    // the outcrop -- so a new game began with the player inside the hill.
    let mut scripts = Scripts::new();
    let report = scripts.advance(
        &mut world,
        extractor.components(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let at = logical_position(
        grid,
        map,
        world
            .get(player)
            .and_then(|data| data.transform_3d)
            .expect("the player has a transform")
            .position,
    );
    let cell = nearest_cell(at.x, at.y);
    assert!(
        ground.walkable_height(cell).is_some(),
        "the player starts at {cell:?}, which is not ground it can stand on"
    );
}

#[test]
fn the_player_cannot_walk_into_the_water() {
    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let tile_sets = bind_tile_sets().expect("the tile sets decode");
    let (map, grid) = floor_grid(&world, &extractor);
    let ground = surfaces(&world, &extractor);
    let player = player_of(&world);

    let mut scripts = Scripts::new();
    let mut step = |world: &mut World, held: &InputState| {
        let report = scripts.advance(
            world,
            extractor.components(),
            ScriptFrame::new(&sources, held, 1.0 / 60.0).with_tile_sets(&tile_sets),
        );
        // Walking the whole island reaches the shed door, and a door asks to
        // change scene. This harness plays one scene on purpose -- following
        // the door would be a test of the shed -- so that one refusal is
        // expected and everything else is not.
        let unexpected: Vec<_> = report
            .failures
            .iter()
            .filter(|failure| !format!("{failure:?}").contains("Scene.go"))
            .collect();
        assert!(unexpected.is_empty(), "{unexpected:?}");
    };

    step(&mut world, &InputState::default());
    let cell_now = |world: &World| {
        let at = logical_position(
            grid,
            map,
            world
                .get(player)
                .and_then(|data| data.transform_3d)
                .expect("the player has a transform")
                .position,
        );
        nearest_cell(at.x, at.y)
    };

    // Held long enough to cross the whole island in each direction in turn, so
    // whichever edge is nearest, the moat is reached and leaned on.
    for key in [
        Key::ArrowUp,
        Key::ArrowDown,
        Key::ArrowLeft,
        Key::ArrowRight,
    ] {
        let mut held = InputState::default();
        held.apply(InputEvent::KeyPressed(key));
        for _ in 0..600 {
            step(&mut world, &held);
            let cell = cell_now(&world);
            assert!(
                ground.walkable_height(cell).is_some(),
                "walking {key:?}, the player reached {cell:?}, which is water or \
                 a hole"
            );
        }
    }
}
