//! Asking the ground whether it can be stood on, from a script.
//!
//! The question a script could not ask. Occupancy was the engine's own answer
//! and the pathfinder read it directly, but a script could only ask about a
//! route between two entities — so a game moving its own player had to tag
//! every solid thing and compare positions, which knows nothing about the
//! ground. Gather's player walked over its pond and through its hill for
//! exactly that reason: neither is an entity.

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

/// Ground you can stand on, water you cannot, and a flower that is only a
/// picture — the three answers a single solid flag could not give.
fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": {
             "grass": { "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
             "water": { "supports": true, "walkable": false,
                        "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } },
             "flower": { "supports": false, "walkable": false,
                         "faces": { "top": { "sprite": "b.png#0", "size": [1.0, 1.0] } } }
           } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

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
            json!({ "source": "walk.decay", "script": "Walker" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("walk.decay", script);
    (world, actor, sources)
}

fn run(world: &mut World, sources: &ScriptSources) -> Vec<ScriptFailure> {
    let sets = tile_sets();
    let input = InputState::default();
    let frame = ScriptFrame::new(sources, &input, 1.0 / 60.0).with_tile_sets(&sets);
    Scripts::new().advance(world, &registry(), frame).failures
}

/// The scale the script wrote, one axis per answer it was checking.
fn scale(world: &World, actor: EntityId) -> [f32; 3] {
    world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .scale
}

const GROUND: &str = r#"[
    { "position": [1, 1, 0], "tile": "grass" },
    { "position": [2, 1, 0], "tile": "water" },
    { "position": [3, 1, 0], "tile": "flower" }
]"#;

#[test]
fn grass_is_walkable_and_water_and_decoration_and_the_void_are_not() {
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Walker {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.walkable(floor, 1.0, 1.0) {
                    this.transform.scale.x = 2.0;
                }
                // Water holds a boat up and holds a walker up not at all.
                if !Grid.walkable(floor, 2.0, 1.0) {
                    this.transform.scale.y = 3.0;
                }
                // A flower is a picture in a cell; nothing rests on it, so the
                // column under it is a hole. And an empty column is a hole
                // whatever the reason, including being off the grid entirely.
                if !Grid.walkable(floor, 3.0, 1.0) && !Grid.walkable(floor, 9.0, 9.0) {
                    this.transform.scale.z = 4.0;
                }
            }
        }
        "#,
        &serde_json::from_str(GROUND).unwrap(),
    );
    assert!(run(&mut world, &sources).is_empty());
    let scale = scale(&world, actor);
    assert!((scale[0] - 2.0).abs() < 1.0e-5, "grass is walkable");
    assert!((scale[1] - 3.0).abs() < 1.0e-5, "water is not walkable");
    assert!(
        (scale[2] - 4.0).abs() < 1.0e-5,
        "decoration and the void are not walkable"
    );
}

#[test]
fn the_cell_a_point_falls_in_is_the_one_answered_for() {
    // A walker moves in continuous coordinates, so it asks about 1.4 rather
    // than 1. The grid decides which cell that is, by the same rule that
    // decides which column the walker's own placement reads from.
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Walker {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.walkable(floor, 1.4, 0.6) {
                    this.transform.scale.x = 2.0;
                }
                if !Grid.walkable(floor, 1.6, 1.4) {
                    this.transform.scale.y = 3.0;
                }
            }
        }
        "#,
        &serde_json::from_str(GROUND).unwrap(),
    );
    assert!(run(&mut world, &sources).is_empty());
    let scale = scale(&world, actor);
    assert!(
        (scale[0] - 2.0).abs() < 1.0e-5,
        "a point just inside the grass cell is on the grass"
    );
    assert!(
        (scale[1] - 3.0).abs() < 1.0e-5,
        "a point that rounds into the water cell is on the water"
    );
}

#[test]
fn a_cell_a_script_just_filled_in_is_walkable_at_once() {
    // The surface is derived at most once an update and kept, so a script that
    // writes a cell and then asks about it must not be told what was true
    // before the write.
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Walker {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if !Grid.walkable(floor, 0.0, 3.0) {
                    this.transform.scale.x = 2.0;
                }
                Grid.set_block(floor, 0.0, 3.0, 0.0, "grass");
                if Grid.walkable(floor, 0.0, 3.0) {
                    this.transform.scale.y = 3.0;
                }
            }
        }
        "#,
        &serde_json::from_str(GROUND).unwrap(),
    );
    assert!(run(&mut world, &sources).is_empty());
    let scale = scale(&world, actor);
    assert!((scale[0] - 2.0).abs() < 1.0e-5, "the column started empty");
    assert!(
        (scale[1] - 3.0).abs() < 1.0e-5,
        "a cell written this update is walkable in the same update"
    );
}

#[test]
fn a_host_that_cannot_read_the_ground_does_not_block_on_it() {
    // The convention `Grid.can_reach` set: a host binding no tile sets is one
    // where nothing could draw the volume either, so the answer comes from
    // what is knowable rather than from an error. Every game that moved its
    // own player before this call existed did so with nothing stopping it at
    // the ground, and that is what it keeps doing here.
    let (mut world, actor, sources) = scripted_volume(
        r#"
        script Walker {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.walkable(floor, 2.0, 1.0) {
                    this.transform.scale.x = 2.0;
                }
            }
        }
        "#,
        &serde_json::from_str(GROUND).unwrap(),
    );
    let input = InputState::default();
    // No `with_tile_sets`, which is the whole point.
    let frame = ScriptFrame::new(&sources, &input, 1.0 / 60.0);
    let failures = Scripts::new()
        .advance(&mut world, &registry(), frame)
        .failures;
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        (scale(&world, actor)[0] - 2.0).abs() < 1.0e-5,
        "water read as unwalkable in a host that cannot know it is water"
    );
}
