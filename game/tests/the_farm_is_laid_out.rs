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
use sindri_scene::{
    GridPlacementComponent, SceneExtractor, SpriteComponent, TileGridComponent, WorldGridNavigation,
};

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

/// Draw order follows where a thing stands, and nothing says so in the scene.
///
/// This test used to assert the formula `1 + 2 * (column + row)` against every
/// world sprite's authored `layer`, because that formula was the only depth a
/// 2D scene had: viewed straight on through an orthographic camera every draw
/// is the same distance away, so the render layer decided everything and
/// seventy-four integers had to be right by hand.
///
/// They are gone. `docs/2d-depth-and-placement.md` makes depth a consequence of
/// position: a prop names the cell it stands on, the transform is derived from
/// it — Z included — and the camera orders by the Z it already sorted by. What
/// is asserted now is the absence, and then that the order survives it.
#[test]
fn no_world_sprite_authors_its_own_depth() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");

    let authored: Vec<String> = extractor
        .components()
        .query::<SpriteComponent>(&world)
        .expect("the sprite schema reads")
        .into_iter()
        .filter(|(entity, _)| {
            world
                .get(*entity)
                .is_some_and(|data| data.components.contains_key("sindri.grid.placement"))
        })
        .filter(|(entity, _)| {
            world
                .get(*entity)
                .and_then(|data| data.components.get("sindri.sprite"))
                .is_some_and(|payload| payload.get("layer").is_some())
        })
        .map(|(entity, _)| {
            world
                .get(entity)
                .and_then(|data| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
                .unwrap_or_default()
        })
        .collect();
    assert!(
        authored.is_empty(),
        "a prop standing on the grid has no business authoring a layer: {authored:?}"
    );
}

/// And the order the deleted integers used to produce still comes out.
///
/// The one step of the acceptance test an author cannot fake: every layer is
/// gone, so if the ordering is right it is the engine producing it.
///
/// Depth is read from the cell a prop names rather than by unprojecting where
/// it ended up. Those differ on purpose: a prop standing on the plateau is
/// lifted up the screen, which *looks* further away, and its cell is the thing
/// that says it is not. Measuring the screen position would assert the bug
/// this design exists to avoid.
#[test]
fn depth_still_runs_down_the_isometric_diagonal() {
    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let tile_sets = sindri_gather::bind_tile_sets().expect("the tile sets decode");
    sindri_scene::resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets))
        .expect("every placement resolves");

    let mut standing: Vec<(f64, f32, String)> = Vec::new();
    for (entity, placement) in extractor
        .components()
        .query::<GridPlacementComponent>(&world)
        .expect("the placement schema reads")
    {
        // Only what names a cell. A walker's depth follows whatever moved it
        // this step, which is a different promise tested elsewhere.
        let Some([column, row]) = placement.cell else {
            continue;
        };
        let Some(data) = world.get(entity) else {
            continue;
        };
        standing.push((
            f64::from(column)
                + f64::from(placement.offset[0])
                + f64::from(row)
                + f64::from(placement.offset[1]),
            data.transform_3d.unwrap_or_default().position[2],
            data.source_id
                .as_ref()
                .map_or_else(String::new, |id| id.as_str().to_owned()),
        ));
    }
    assert!(
        standing.len() > 60,
        "the farm's fixed props should all name a cell: {}",
        standing.len()
    );

    // Sorted by where they stand, Z never goes backwards: further down the
    // diagonal is nearer the camera, which is the whole of the rule.
    standing.sort_by(|a, b| a.0.total_cmp(&b.0));
    for pair in standing.windows(2) {
        let (first, second) = (&pair[0], &pair[1]);
        assert!(
            second.1 >= first.1 - 1.0e-6,
            "{} at depth {:.2} has Z {} but {} at depth {:.2} has {}",
            first.2,
            first.0,
            first.1,
            second.2,
            second.0,
            second.1
        );
    }
}

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
