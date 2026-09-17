//! Where everything on the farm stands, and what it stands on.
//!
//! Separate from the play-through tests beside it because these are about the
//! scene at rest: the island is a stack of blocks with props on it, and both
//! halves of that -- what draws in front of what, and what has ground under it
//! -- are properties of the layout rather than of anything the game does when
//! someone plays it.

use sindri_gather::{extractor, world};
use sindri_grid::GridCoord;
use sindri_scene::{GridPlacementComponent, SpriteComponent, WorldGridNavigation};

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
    //
    // Never by more than the clearance, rather than never at all. Something
    // standing on ground no higher than its feet sorts three quarters of a cell
    // forward to get out from under it, and whether it does depends on the
    // terrain a step ahead -- so two props on the same diagonal with different
    // ground ahead of them sit a fraction apart. They are in different places on
    // the same diagonal and never overlap; what the rule is about is the order
    // down the diagonal, and that is what is asserted.
    // `docs/2d-depth-and-placement.md` carries the reasoning.
    let clearance = 0.75 * 0.01 + 1.0e-6;
    standing.sort_by(|a, b| a.0.total_cmp(&b.0));
    for pair in standing.windows(2) {
        let (first, second) = (&pair[0], &pair[1]);
        assert!(
            second.1 >= first.1 - clearance,
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
