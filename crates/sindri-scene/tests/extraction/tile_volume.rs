//! Stackable cells expanded into only their visible baked faces.

use sindri_core::{SpriteSheetDocument, TileSetDocument};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{
    CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings,
};

use crate::support::{VIEWPORT, scene, world_from};

fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
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
    .unwrap();
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
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
