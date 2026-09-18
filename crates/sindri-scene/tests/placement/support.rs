//! Building the small worlds these tests place things on.

use serde_json::{Value, json};
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, TileSetDocument, Transform3D,
    World,
};
use sindri_scene::{SceneExtractor, TileSetBindings};

pub(crate) fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a valid scene ID")
}

/// A four-by-four isometric grid whose ground is one solid level below the
/// plane, with whatever entities the test wants standing on it.
pub(crate) fn world_with(entities: Vec<SceneEntity>, depth_step: f64) -> (World, SceneExtractor) {
    let mut floor = SceneEntity::new(id("floor"));
    floor.transform_3d = Some(Transform3D::default());
    floor.components.insert(
        "sindri.tile_grid".to_owned(),
        json!({
            "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric",
            "depth_step": depth_step
        }),
    );
    floor.components.insert(
        "sindri.tile_volume".to_owned(),
        json!({
            "tileset": "world.tileset.json",
            "cells": (0..4).flat_map(|row| (0..4).map(move |column| {
                json!({ "position": [column, row, -1], "tile": "block" })
            })).collect::<Vec<_>>()
        }),
    );

    let mut all = vec![floor];
    all.extend(entities);
    let document = SceneDocument {
        format_version: SCENE_FORMAT_VERSION,
        metadata: sindri_core::SceneMetadata::default(),
        entities: all,
    };
    let extractor = SceneExtractor::new().expect("the schemas register");
    let mut world = World::default();
    sindri_core::LoadedScenes::new()
        .enter_keeping_identities(&mut world, "test", &document)
        .expect("the scene loads");
    (world, extractor)
}

pub(crate) fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": {
             "block": { "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
             "slab": { "height": 0.5, "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } }
           } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

pub(crate) fn placed(name: &str, cell: &Value) -> SceneEntity {
    let mut entity = SceneEntity::new(id(name));
    entity.transform_3d = Some(Transform3D::default());
    entity.components.insert(
        "sindri.grid.placement".to_owned(),
        json!({ "grid": "floor", "cell": cell.clone() }),
    );
    entity
}

pub(crate) fn transform_of(world: &World, name: &str) -> Transform3D {
    world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|value| value.as_str() == name)
        })
        .and_then(|(_, data)| data.transform_3d)
        .expect("the entity kept a transform")
}
