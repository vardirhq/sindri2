//! What the shape of a stacked volume says about where a walker may go.

use serde_json::{Value, json};
use sindri_core::TileSetDocument;
use sindri_grid::{GridCoord, GridPathfinder};
use sindri_scene::{TileSetBindings, WorldGridNavigation};

use crate::support::{entity, id, world};

/// A volume floor, its tile set, and one occupant standing on a named column.
fn volume_world(cells: &str, max_step: f32) -> (sindri_core::LoadedScene, TileSetBindings) {
    let floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [
            (
                "sindri.tile_grid",
                json!({ "columns": 4, "rows": 1, "cell_size": [1.0, 1.0], "level_step": [0.0, 1.0] }),
            ),
            (
                "sindri.tile_volume",
                json!({ "tileset": "world.tileset.json", "cells": serde_json::from_str::<Value>(cells).unwrap() }),
            ),
            (
                "sindri.grid.navigation",
                json!({ "walls": [], "max_step": max_step }),
            ),
        ],
    );
    let actor = entity(
        "actor",
        Some([0.5, -0.5, 0.0]),
        [(
            "sindri.grid.occupant",
            json!({ "grid": "floor", "footprint": [[0, 0]] }),
        )],
    );

    let document = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": {
            "block": { "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
            "slab": { "height": 0.5, "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
            "water": { "solid": false, "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } }
          }
        }"#,
    )
    .expect("the tile set decodes");
    let mut tile_sets = TileSetBindings::new();
    tile_sets.bind("world.tileset.json", document).unwrap();
    (world(vec![floor, actor]), tile_sets)
}

/// Whether the actor can walk from column 0 to the far end of the row.
fn can_cross(cells: &str, max_step: f32) -> bool {
    let (loaded, tile_sets) = volume_world(cells, max_step);
    let floor_id = loaded.entity_map[&id("floor")];
    let actor_id = loaded.entity_map[&id("actor")];
    WorldGridNavigation::from_world_with_tile_sets(&loaded.world, floor_id, &tile_sets)
        .expect("a volume floor navigates")
        .find_path(GridPathfinder::default(), actor_id, GridCoord::new(3, 0))
        .expect("the query runs")
        .is_some()
}

#[test]
fn a_column_with_nothing_solid_in_it_is_a_hole() {
    // Level ground the whole way is crossable; take one column out and the
    // walker has nowhere to put its feet.
    assert!(can_cross(
        r#"[{ "position": [0, 0, 0], "tile": "block" },
            { "position": [1, 0, 0], "tile": "block" },
            { "position": [2, 0, 0], "tile": "block" },
            { "position": [3, 0, 0], "tile": "block" }]"#,
        1.0
    ));
    assert!(
        !can_cross(
            r#"[{ "position": [0, 0, 0], "tile": "block" },
                { "position": [2, 0, 0], "tile": "block" },
                { "position": [3, 0, 0], "tile": "block" }]"#,
            1.0
        ),
        "a column with no cells at all is a hole rather than ground at level zero"
    );
}

#[test]
fn a_tile_that_is_not_solid_is_not_a_floor() {
    // Water fills the gap and changes nothing: the flag exists to say a cell is
    // not something to stand on, so a column holding only water is still a hole.
    assert!(
        !can_cross(
            r#"[{ "position": [0, 0, 0], "tile": "block" },
                { "position": [1, 0, 0], "tile": "water" },
                { "position": [2, 0, 0], "tile": "block" },
                { "position": [3, 0, 0], "tile": "block" }]"#,
            1.0
        ),
        "water is not a floor"
    );
}

#[test]
fn a_step_bigger_than_the_limit_is_a_wall() {
    let two_high = r#"[{ "position": [0, 0, 0], "tile": "block" },
                       { "position": [1, 0, 0], "tile": "block" },
                       { "position": [1, 0, 1], "tile": "block" },
                       { "position": [2, 0, 0], "tile": "block" },
                       { "position": [3, 0, 0], "tile": "block" }]"#;
    assert!(
        can_cross(two_high, 1.0),
        "one cell up and one cell down is the default step"
    );
    assert!(
        !can_cross(two_high, 0.5),
        "half a cell of step turns the same block into a wall"
    );
}

#[test]
fn a_slab_is_a_step_where_a_block_would_be_a_wall() {
    // The reason height is a float rather than a level. Raise the middle column
    // by a slab and its surface is half a cell up; raise it by a block and it is
    // a whole cell up. A walker limited to half a cell clears the first and not
    // the second, which is the distinction a level count cannot make.
    let raised = |tile: &str| {
        format!(
            r#"[{{ "position": [0, 0, 0], "tile": "block" }},
                {{ "position": [1, 0, 0], "tile": "block" }},
                {{ "position": [1, 0, 1], "tile": "{tile}" }},
                {{ "position": [2, 0, 0], "tile": "block" }},
                {{ "position": [3, 0, 0], "tile": "block" }}]"#
        )
    };
    assert!(
        can_cross(&raised("slab"), 0.5),
        "half a cell of step should clear a slab"
    );
    assert!(
        !can_cross(&raised("block"), 0.5),
        "and should not clear a full block raised in the same column"
    );
}

#[test]
fn a_volume_beside_a_flat_map_does_not_close_the_floor() {
    // A migration in progress: the scene still carries sindri.tilemap, so the
    // flat map is the floor and the volume beside it has no say. Closing the
    // ground under a game the moment a volume is added would make trying one
    // impossible.
    let mut floor = entity(
        "floor",
        Some([0.0, 0.0, 0.0]),
        [
            (
                "sindri.tile_grid",
                json!({ "columns": 4, "rows": 1, "cell_size": [1.0, 1.0] }),
            ),
            (
                "sindri.tile_volume",
                json!({ "tileset": "world.tileset.json",
                        "cells": [{ "position": [0, 0, 0], "tile": "block" }] }),
            ),
        ],
    );
    floor.components.insert(
        "sindri.tilemap".to_owned(),
        json!({
            "texture": "tiles", "palette": [], "columns": 4, "rows": 1,
            "tiles": vec![Value::Null; 4], "tile_size": [1.0, 1.0]
        }),
    );
    let actor = entity(
        "actor",
        Some([0.5, -0.5, 0.0]),
        [(
            "sindri.grid.occupant",
            json!({ "grid": "floor", "footprint": [[0, 0]] }),
        )],
    );
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": { "block": { "faces": {
             "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } } } }"#,
    )
    .unwrap();
    let mut tile_sets = TileSetBindings::new();
    tile_sets.bind("world.tileset.json", document).unwrap();

    let loaded = world(vec![floor, actor]);
    let floor_id = loaded.entity_map[&id("floor")];
    let actor_id = loaded.entity_map[&id("actor")];
    let navigation =
        WorldGridNavigation::from_world_with_tile_sets(&loaded.world, floor_id, &tile_sets)
            .expect("navigation derives");
    assert!(
        navigation
            .find_path(GridPathfinder::default(), actor_id, GridCoord::new(3, 0))
            .expect("the query runs")
            .is_some(),
        "the flat map is still the floor, so its three empty columns stay walkable"
    );
}

#[test]
fn sparse_navigation_cost_follows_loaded_ground_not_declared_bounds() {
    let (mut loaded, tile_sets) =
        volume_world(r#"[{ "position": [0, 0, 0], "tile": "block" }]"#, 1.0);
    let floor_id = loaded.entity_map[&id("floor")];
    let floor = loaded.world.get_mut(floor_id).expect("the floor remains");
    floor.components["sindri.tile_grid"]["columns"] = json!(65_536);
    floor.components["sindri.tile_grid"]["rows"] = json!(65_536);

    let navigation =
        WorldGridNavigation::from_world_with_tile_sets(&loaded.world, floor_id, &tile_sets)
            .expect("sparse navigation derives");
    assert_eq!(
        navigation.walls().len(),
        2,
        "one loaded corner cell has only its two in-bounds frontier edges"
    );
}
