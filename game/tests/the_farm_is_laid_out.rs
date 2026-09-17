//! Where everything on the farm stands, and what it stands on.
//!
//! Separate from the play-through tests beside it because these are about the
//! scene at rest: the island is a stack of blocks with props on it, and both
//! halves of that -- what draws in front of what, and what has ground under it
//! -- are properties of the layout rather than of anything the game does when
//! someone plays it.

use sindri_core::{Transform3D, World};
use sindri_gather::{extractor, world};
use sindri_grid::{GridCoord, GridPoint, GridSpace, PlanePoint};
use sindri_scene::{SceneExtractor, SpriteComponent, TileGridComponent, WorldGridNavigation};

/// Where a world position falls on the floor's grid, in whole and part cells.
///
/// Repeated from `the_game_plays.rs` rather than shared: each integration test
/// is its own binary, and a dozen lines of projection maths is cheaper to have
/// twice than a module that exists only to be imported.
fn logical_position(grid: GridSpace, map: Transform3D, world: [f32; 3]) -> GridPoint {
    let (sin, cos) = map.rotation_z_radians().sin_cos();
    let x = world[0] - map.position[0];
    let y = world[1] - map.position[1];
    let local = PlanePoint::new(
        f64::from((cos * x + sin * y) / map.scale[0]),
        f64::from((-sin * x + cos * y) / map.scale[1]),
    );
    grid.unproject(local)
        .expect("the authored point unprojects")
}

fn floor_grid(world: &World, extractor: &SceneExtractor) -> (Transform3D, GridSpace) {
    // The floor is a stacked volume rather than a flat map, and the grid that
    // describes it is the one thing the two always agreed about: a cell is in
    // the same place either way, which is why the layer convention below did
    // not have to change when the floor did.
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

/// Draw order follows where a thing stands, not what it is made of.
///
/// Sprites batch by layer *and texture*, and a frame's passes are ordered by
/// layer alone — so two different textures on one layer are drawn in whichever
/// order their textures happen to sort in, however far apart they stand. That
/// is why every world entity carries a layer derived from its isometric row
/// rather than a hand-picked one: before this, the orbs sat on layer 10 and the
/// player on 20, so both drew over the shrine from anywhere on the island.
///
/// Half rows, because a wall stands on the edge *between* two cells and so
/// falls on an exact half row; rounding that to a whole one decides by coin
/// toss whether the wall occludes what is behind it.
#[test]
fn every_world_sprite_layers_by_where_it_stands() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let (map, grid) = floor_grid(&world, &extractor);

    let mut checked = 0;
    for (entity, sprite) in extractor
        .components()
        .query::<SpriteComponent>(&world)
        .expect("the sprite schema reads")
    {
        let Some(transform) = world.get(entity).and_then(|data| data.transform_3d) else {
            continue;
        };
        let at = logical_position(grid, map, transform.position);
        if !(-1.0_f64..=25.0).contains(&at.x) || !(-1.0_f64..=25.0).contains(&at.y) {
            continue;
        }

        // Kept in floating point rather than casting the row to an integer:
        // the cast would be the only lossy step in the comparison, and it is
        // not the thing under test. Both sides are whole numbers, so the
        // tolerance costs nothing and says so.
        let expected = 1.0 + (2.0 * (at.x + at.y)).round();
        assert!(
            (f64::from(sprite.layer) - expected).abs() < 1.0e-9,
            "{} stands on row {:.2} and belongs on layer {expected}, not {}",
            world
                .get(entity)
                .and_then(|data| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
                .unwrap_or_default(),
            at.x + at.y,
            sprite.layer,
        );
        checked += 1;
    }

    assert!(
        checked >= 70,
        "expected the world's sprites, checked {checked}"
    );
}

/// Solid things are solid: walking at one stops rather than passing through.
///
/// Driven by holding a direction through the real scripts, which is what a
/// player does — not by asking the collision helper whether a point is free,
/// which would only test that the helper agrees with itself.
///
/// The player is put beside the tree rather than walked at whatever happens to
/// be west of its start: a collision test that depends on the layout fails
/// every time someone moves a tree, which is a test about composition wearing a
/// collision test's name.
/// Nothing in the farm stands on a hole.
///
/// The floor used to be a flat map, where every cell was ground by definition
/// and standing somewhere impossible was not expressible. It is a volume now,
/// and a column with nothing solid in it — the moat, the pond — is a hole. So
/// "every occupant has ground under it" became a thing that can be false, and
/// the migration is only honest if it is checked rather than assumed.
#[test]
fn everything_placed_on_the_farm_has_ground_under_it() {
    let (world, _loaded) = world().expect("the scene loads");
    let tile_sets = sindri_gather::bind_tile_sets().expect("the tile sets decode");
    let floor = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .expect("the game has a floor");

    let navigation = WorldGridNavigation::from_world_with_tile_sets(&world, floor, &tile_sets)
        .expect("a volume floor navigates");
    let volume: sindri_scene::TileVolumeComponent = serde_json::from_value(
        world
            .get(floor)
            .and_then(|data| data.components.get("sindri.tile_volume"))
            .expect("the floor is a volume")
            .clone(),
    )
    .expect("the volume payload is valid");
    let surfaces = sindri_scene::TileSurfaces::derive(
        &volume,
        tile_sets.get("gather.tileset.json").expect("bound"),
    )
    .expect("every cell names a tile the set defines");

    let mut stranded = Vec::new();
    for (entity, data) in world.entities() {
        if !data.components.contains_key("sindri.grid.occupant") {
            continue;
        }
        let Some(placement) = navigation.placement(entity) else {
            continue;
        };
        for offset in placement.footprint.offsets() {
            let cell = GridCoord::new(placement.anchor.x + offset.x, placement.anchor.y + offset.y);
            if surfaces.height(cell).is_none() {
                let who = data
                    .source_id
                    .as_ref()
                    .map_or("<unnamed>", sindri_core::SceneEntityId::as_str);
                stranded.push(format!("{who} at ({}, {})", cell.x, cell.y));
            }
        }
    }
    assert!(
        stranded.is_empty(),
        "these occupants are standing on water or on nothing: {stranded:#?}"
    );
}
