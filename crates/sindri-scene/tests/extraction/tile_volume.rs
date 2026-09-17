//! Stackable cells expanded into only their visible baked faces.

use sindri_core::{SpriteSheetDocument, TileSetDocument};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings};

use crate::support::{VIEWPORT, scene, world_from};

fn tile_sets() -> TileSetBindings {
    bind(
        r#"{
          "format_version": 1,
          "tiles": { "grass": { "faces": {
            "top": {
              "sprite": "blocks.png#0", "size": [1.0, 0.5]
            },
            "south": {
              "sprite": "blocks.png#1", "size": [1.0, 0.5],
              "offset": [0.0, -0.25]
            }
          } } }
        }"#,
    )
}

/// The same two tiles, plus a half-height one. A slab is its own tile with its
/// own baked faces, the way a slab in a voxel game is its own block.
fn tile_sets_with_a_slab() -> TileSetBindings {
    bind(
        r#"{
          "format_version": 1,
          "tiles": {
            "grass": { "faces": {
              "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] },
              "south": { "sprite": "blocks.png#1", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
            } },
            "grass-slab": { "height": 0.5, "faces": {
              "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5], "offset": [0.0, -0.25] },
              "south": { "sprite": "blocks.png#1", "size": [1.0, 0.25], "offset": [0.0, -0.375] }
            } }
          }
        }"#,
    )
}

fn bind(json: &str) -> TileSetBindings {
    let document = TileSetDocument::from_json(json).unwrap();
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

/// How many sprite instances a volume of these cells draws.
fn drawn(cells: &str, tile_sets: &TileSetBindings) -> usize {
    let world = world_from(&scene(&format!(
        r#",
        {{ "id": "blocks", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 2, "rows": 2, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "cells": [{cells}]
          }}
        }} }}"#
    )));
    SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(tile_sets),
        )
        .expect("the volume extracts")
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.len(),
            _ => 0,
        })
        .sum()
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("blocks.png", TextureId::new(9));
    bindings
        .bind_sheet("blocks.png", &SpriteSheetDocument::from_grid(2, 1))
        .unwrap();
    bindings
}

#[test]
fn stacking_culls_the_shared_face_and_keeps_exposed_faces() {
    let world = world_from(&scene(
        r#",
        { "id": "blocks", "transform_3d": {}, "components": {
          "sindri.tile_grid": {
            "columns": 2, "rows": 2, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          },
          "sindri.tile_volume": {
            "tileset": "world.tileset.json",
            "cells": [
              { "position": [0, 0, 0], "tile": "grass" },
              { "position": [0, 0, 1], "tile": "grass" }
            ]
          }
        } }"#,
    ));
    let tile_sets = tile_sets();
    let frame = SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(&tile_sets),
        )
        .expect("the volume extracts");

    let instances = frame
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.len(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(
        instances, 3,
        "the lower top is hidden; two south faces and the upper top remain"
    );
}

#[test]
fn a_volume_requires_its_reusable_tile_set_binding() {
    let world = world_from(&scene(
        r#",
        { "id": "blocks", "transform_3d": {}, "components": {
          "sindri.tile_grid": { "columns": 1, "rows": 1 },
          "sindri.tile_volume": {
            "tileset": "missing.tileset.json",
            "cells": [{ "position": [0, 0, 0], "tile": "grass" }]
          }
        } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("an unbound tile set is not guessed");
    assert!(error.to_string().contains("missing.tileset.json"));
}

#[test]
fn a_full_block_above_a_slab_leaves_the_slabs_top_showing() {
    // The slab's top sits half a cell below the block's floor, so the gap
    // between them is something an isometric camera looks into. Culling that
    // top face is what would make a slab indistinguishable from a block.
    let stacked = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" },
           { "position": [0, 0, 1], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(
        stacked, 4,
        "both tops and both south faces should survive: {stacked}"
    );

    // A full block in the same place hides the top beneath it, which is the
    // behaviour heights must not have broken.
    let solid = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 0, 1], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(solid, 3, "the lower top is hidden by a full block: {solid}");
}

#[test]
fn a_slab_does_not_hide_the_side_of_a_block_beside_it() {
    // The slab covers the bottom half of its neighbour's south face and leaves
    // the top half exposed, so the face has to be drawn.
    let slab_neighbour = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 1, 0], "tile": "grass-slab" }"#,
        &tile_sets_with_a_slab(),
    );
    let block_neighbour = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass" },
           { "position": [0, 1, 0], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    assert!(
        slab_neighbour > block_neighbour,
        "a short neighbour should hide less than a full one: {slab_neighbour} vs {block_neighbour}"
    );
}

#[test]
fn a_block_beside_a_slab_still_hides_the_slabs_side() {
    // The other direction: the neighbour is taller than the face it abuts, so
    // nothing of that face could be seen and it is dropped.
    let hidden = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" },
           { "position": [0, 1, 0], "tile": "grass" }"#,
        &tile_sets_with_a_slab(),
    );
    let exposed = drawn(
        r#"{ "position": [0, 0, 0], "tile": "grass-slab" }"#,
        &tile_sets_with_a_slab(),
    );
    assert_eq!(
        hidden,
        exposed + 1,
        "the block brings two faces of its own and takes the slab's south face          away, so the pair draws one more than the slab alone: {hidden} vs {exposed}"
    );
}

#[test]
fn a_height_outside_one_cell_is_refused_when_the_asset_decodes() {
    let error = TileSetDocument::from_json(
        r#"{
          "format_version": 1,
          "tiles": { "tall": { "height": 1.5, "faces": {
            "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] }
          } } }
        }"#,
    )
    .expect_err("a tile taller than its cell is not a tile");
    assert!(
        error.to_string().contains("1.5"),
        "the failure should quote the height written: {error}"
    );
}
