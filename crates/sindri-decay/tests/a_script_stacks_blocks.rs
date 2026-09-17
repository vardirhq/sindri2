//! Reading and writing one cell of a stacked volume from a script.

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
             "stone": { "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } }
           } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

/// A world with one volume and one scripted entity.
fn scripted_volume(script: &str, cells: &serde_json::Value) -> (World, EntityId, ScriptSources) {
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
                json!({ "tileset": "world.tileset.json", "cells": cells.clone() }),
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
            json!({ "source": "blocks.decay", "script": "Builder" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("blocks.decay", script);
    (world, actor, sources)
}

fn run(world: &mut World, sources: &ScriptSources, bind: bool) -> Vec<ScriptFailure> {
    let sets = tile_sets();
    let input = InputState::default();
    let frame = ScriptFrame::new(sources, &input, 1.0 / 60.0);
    let frame = if bind {
        frame.with_tile_sets(&sets)
    } else {
        frame
    };
    Scripts::new().advance(world, &registry(), frame).failures
}

/// The cells the floor holds, as `(position, tile)` pairs.
fn cells(world: &World) -> Vec<(Vec<i64>, String)> {
    world
        .entities()
        .find_map(|(_, data)| data.components.get("sindri.tile_volume"))
        .and_then(|payload| payload.get("cells"))
        .and_then(serde_json::Value::as_array)
        .expect("the volume kept its cells")
        .iter()
        .map(|cell| {
            (
                cell["position"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap())
                    .collect(),
                cell["tile"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_script_reads_the_tile_in_one_cell_and_the_empty_string_elsewhere() {
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.block(floor, 1.0, 1.0, 0.0) == "grass" {
                    this.transform.scale.x = 2.0;
                }
                if Grid.block(floor, 3.0, 3.0, 0.0) == "" {
                    this.transform.scale.y = 3.0;
                }
            }
        }
        "#,
        &json!([{ "position": [1, 1, 0], "tile": "grass" }]),
    );
    assert!(run(&mut world, &sources, true).is_empty());
    let scale = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .scale;
    assert!(
        (scale[0] - 2.0).abs() < 1.0e-5,
        "an occupied cell answers with its tile name"
    );
    assert!(
        (scale[1] - 3.0).abs() < 1.0e-5,
        "an empty cell answers with the empty string rather than a sentinel"
    );
}

#[test]
fn a_script_places_and_clears_a_block() {
    let (mut world, _actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.set_block(floor, 2.0, 0.0, 1.0, "stone");
                Grid.set_block(floor, 1.0, 1.0, 0.0, "");
            }
        }
        "#,
        &json!([{ "position": [1, 1, 0], "tile": "grass" }]),
    );
    assert!(run(&mut world, &sources, true).is_empty());
    assert_eq!(
        cells(&world),
        vec![(vec![2, 0, 1], "stone".to_owned())],
        "the written cell is there and the cleared one is gone rather than blank"
    );
}

#[test]
fn writing_over_a_cell_keeps_what_else_it_carried() {
    // The payload is edited in place, so a field this build does not know about
    // -- a deliberate visual override among them -- survives a script changing
    // the tile beside it.
    let (mut world, _actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                Grid.set_block(World.find("Floor"), 1.0, 1.0, 0.0, "stone");
            }
        }
        "#,
        &json!([{ "position": [1, 1, 0], "tile": "grass", "visual_override": "mossy" }]),
    );
    assert!(run(&mut world, &sources, true).is_empty());
    let stored = world
        .entities()
        .find_map(|(_, data)| data.components.get("sindri.tile_volume"))
        .and_then(|payload| payload.get("cells"))
        .and_then(serde_json::Value::as_array)
        .expect("cells")
        .first()
        .cloned()
        .expect("the cell is still there");
    assert_eq!(stored["tile"], "stone");
    assert_eq!(
        stored["visual_override"], "mossy",
        "editing one field must not rewrite the cell from a partial view"
    );
}

#[test]
fn a_tile_the_set_does_not_define_is_refused_at_the_call() {
    let (mut world, _actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                Grid.set_block(World.find("Floor"), 0.0, 0.0, 0.0, "cheese");
            }
        }
        "#,
        &json!([]),
    );
    let reported = run(&mut world, &sources, true)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        reported.contains("cheese") && reported.contains("world.tileset.json"),
        "the failure should name the tile and the set that does not define it: {reported}"
    );
    assert!(
        cells(&world).is_empty(),
        "and nothing should have been written"
    );
}

#[test]
fn a_host_with_no_tile_sets_refuses_to_write_rather_than_guessing() {
    let (mut world, _actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                Grid.set_block(World.find("Floor"), 0.0, 0.0, 0.0, "grass");
            }
        }
        "#,
        &json!([]),
    );
    let reported = run(&mut world, &sources, false)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        reported.contains("no tile sets"),
        "a host that cannot check a tile name should say so: {reported}"
    );
}

#[test]
fn a_level_between_two_levels_is_refused() {
    let (mut world, _actor, sources) = scripted_volume(
        r#"
        script Builder {
            fn update(dt: f32) {
                Grid.set_block(World.find("Floor"), 0.0, 0.0, 0.5, "grass");
            }
        }
        "#,
        &json!([]),
    );
    let reported = run(&mut world, &sources, true)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        reported.contains("whole number"),
        "half a level is not a cell: {reported}"
    );
}
