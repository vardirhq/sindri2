//! Asking a block what a game says it is.
//!
//! The engine knows whether a block can be stood on; it cannot know that this
//! game burns whoever stands in lava. A tile set gives its blocks words, and a
//! script asks for one.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, TileSetDocument, Transform3D,
    World,
};
use sindri_decay::{ScriptComponent, ScriptFailure, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::TileSetBindings;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": {
             "grass": { "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
             "lava": { "walkable": false, "tags": ["hot", "liquid"],
                       "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } }
           } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

fn scripted_volume(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Floor".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [
            (
                "sindri.tile_grid".to_owned(),
                json!({ "columns": 4, "rows": 4, "cell_size": [1.0, 1.0] }),
            ),
            (
                "sindri.tile_volume".to_owned(),
                json!({ "tileset": "world.tileset.json", "cells": [
                    { "position": [1, 1, 0], "tile": "grass" },
                    { "position": [2, 1, 0], "tile": "lava" }
                ] }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let actor = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "burn.decay", "script": "Burner" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("burn.decay", script);
    (world, actor, sources)
}

fn run(world: &mut World, sources: &ScriptSources) -> Vec<ScriptFailure> {
    let sets = tile_sets();
    let input = InputState::default();
    let frame = ScriptFrame::new(sources, &input, 1.0 / 60.0).with_tile_sets(&sets);
    Scripts::new().advance(world, &registry(), frame).failures
}

#[test]
fn lava_is_hot_and_grass_and_air_are_not() {
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Burner {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.tagged(floor, 2.0, 1.0, 0.0, "hot") {
                    this.transform.scale.x = 2.0;
                }
                if !Grid.tagged(floor, 1.0, 1.0, 0.0, "hot") {
                    this.transform.scale.y = 3.0;
                }
                // Nothing is in the cell above the lava, and nothing carries
                // no tag.
                if !Grid.tagged(floor, 2.0, 1.0, 1.0, "hot") {
                    this.transform.scale.z = 4.0;
                }
            }
        }
        "#,
    );
    assert_eq!(run(&mut world, &sources), Vec::new());
    let scale = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .scale;
    assert_eq!(scale, [2.0, 3.0, 4.0]);
}
