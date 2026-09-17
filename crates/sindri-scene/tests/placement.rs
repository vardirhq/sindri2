//! Deriving a transform from a cell, and a depth from a position.

use serde_json::{Value, json};
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, TileSetDocument, Transform3D,
    World,
};
use sindri_scene::{SceneExtractor, TileSetBindings, resolve_grid_placements};

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a valid scene ID")
}

/// A four-by-four isometric grid whose ground is one solid level below the
/// plane, with whatever entities the test wants standing on it.
fn world_with(entities: Vec<SceneEntity>, depth_step: f64) -> (World, SceneExtractor) {
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

fn tile_sets() -> TileSetBindings {
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

fn placed(name: &str, cell: &Value) -> SceneEntity {
    let mut entity = SceneEntity::new(id(name));
    entity.transform_3d = Some(Transform3D::default());
    entity.components.insert(
        "sindri.grid.placement".to_owned(),
        json!({ "grid": "floor", "cell": cell.clone() }),
    );
    entity
}

fn transform_of(world: &World, name: &str) -> Transform3D {
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

#[test]
fn a_placed_prop_lands_on_the_cell_it_names() {
    let (mut world, extractor) = world_with(vec![placed("rock", &json!([2, 1]))], 0.0);
    let resolved = resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets()))
        .expect("the placement resolves");
    assert_eq!(resolved, 1);

    // The ground fills level -1, so its top — and the prop standing on it — is
    // at the grid's own plane.
    let grid: sindri_scene::TileGridComponent = serde_json::from_value(json!({
        "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
        "level_step": [0.0, 0.5], "projection": "isometric"
    }))
    .unwrap();
    let expected = grid
        .cell_to_local(sindri_grid::GridCoord3::new(2, 1, 0))
        .expect("the cell projects");
    let position = transform_of(&world, "rock").position;
    assert!(
        (position[0] - expected[0]).abs() < 1.0e-5 && (position[1] - expected[1]).abs() < 1.0e-5,
        "a prop naming a cell stands in it: {position:?} vs {expected:?}"
    );
}

#[test]
fn raising_the_ground_raises_what_stands_on_it() {
    // The bug this component exists to make impossible: the floor moves and
    // the props stay where a hand-written transform left them.
    let (mut world, extractor) = world_with(vec![placed("rock", &json!([1, 1]))], 0.0);
    let sets = tile_sets();
    resolve_grid_placements(&mut world, extractor.components(), Some(&sets)).unwrap();
    let before = transform_of(&world, "rock").position[1];

    let floor = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|value| value.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .unwrap();
    world
        .get_mut(floor)
        .and_then(|data| data.components.get_mut("sindri.tile_volume"))
        .and_then(|payload| payload.get_mut("cells"))
        .and_then(Value::as_array_mut)
        .unwrap()
        .push(json!({ "position": [1, 1, 0], "tile": "block" }));

    resolve_grid_placements(&mut world, extractor.components(), Some(&sets)).unwrap();
    let after = transform_of(&world, "rock").position[1];
    assert!(
        after > before,
        "a block placed under the prop should lift it: {before} -> {after}"
    );
}

#[test]
fn a_slab_lifts_by_half_of_what_a_block_does() {
    // Height is a fraction, so standing on a slab is standing half a cell up.
    // A level count could not say this.
    let lift = |tile: &str| {
        let (mut world, extractor) = world_with(vec![placed("rock", &json!([1, 1]))], 0.0);
        let floor = world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|value| value.as_str() == "floor")
            })
            .map(|(entity, _)| entity)
            .unwrap();
        world
            .get_mut(floor)
            .and_then(|data| data.components.get_mut("sindri.tile_volume"))
            .and_then(|payload| payload.get_mut("cells"))
            .and_then(Value::as_array_mut)
            .unwrap()
            .push(json!({ "position": [1, 1, 0], "tile": tile }));
        resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets())).unwrap();
        transform_of(&world, "rock").position[1]
    };
    let flat = {
        let (mut world, extractor) = world_with(vec![placed("rock", &json!([1, 1]))], 0.0);
        resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets())).unwrap();
        transform_of(&world, "rock").position[1]
    };
    let block = lift("block") - flat;
    let slab = lift("slab") - flat;
    assert!(
        (block - slab * 2.0).abs() < 1.0e-5,
        "a slab should lift half as far as a block: {slab} vs {block}"
    );
}

#[test]
fn depth_comes_from_the_cell_and_orders_the_diagonal() {
    let (mut world, extractor) = world_with(
        vec![
            placed("near", &json!([3, 3])),
            placed("far", &json!([0, 0])),
            placed("side", &json!([3, 0])),
        ],
        0.01,
    );
    resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets())).unwrap();

    let z = |name: &str| transform_of(&world, name).position[2];
    assert!(
        z("near") > z("side") && z("side") > z("far"),
        "the isometric diagonal orders them: far {}, side {}, near {}",
        z("far"),
        z("side"),
        z("near")
    );
}

#[test]
fn a_grid_with_no_depth_step_derives_no_depth() {
    // The migration boundary: a scene that has not opted in keeps every Z it
    // had, so its ordering cannot change underneath it.
    let (mut world, extractor) = world_with(
        vec![
            placed("near", &json!([3, 3])),
            placed("far", &json!([0, 0])),
        ],
        0.0,
    );
    resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets())).unwrap();
    for name in ["near", "far"] {
        let z = transform_of(&world, name).position[2];
        assert!(z.abs() < f32::EPSILON, "{name} kept the Z it had: {z}");
    }
}

#[test]
fn something_that_moves_keeps_its_position_and_gains_a_depth() {
    // No cell: a walker's X and Y are its script's business, and only the depth
    // is answered here. The two halves are the same rule.
    let mut walker = SceneEntity::new(id("walker"));
    walker.transform_3d = Some(Transform3D {
        position: [0.5, -0.75, 0.0],
        ..Transform3D::default()
    });
    walker.components.insert(
        "sindri.grid.placement".to_owned(),
        json!({ "grid": "floor" }),
    );

    let (mut world, extractor) = world_with(vec![walker], 0.01);
    resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets())).unwrap();
    let after = transform_of(&world, "walker").position;
    assert!(
        (after[0] - 0.5).abs() < 1.0e-6 && (after[1] + 0.75).abs() < 1.0e-6,
        "its own position is left alone: {after:?}"
    );
    assert!(after[2] > 0.0, "and it gains a depth: {}", after[2]);
}

#[test]
fn a_placement_naming_no_grid_says_so() {
    let mut lost = SceneEntity::new(id("lost"));
    lost.transform_3d = Some(Transform3D::default());
    lost.components.insert(
        "sindri.grid.placement".to_owned(),
        json!({ "grid": "nowhere", "cell": [0, 0] }),
    );
    let (mut world, extractor) = world_with(vec![lost], 0.0);
    let error = resolve_grid_placements(&mut world, extractor.components(), Some(&tile_sets()))
        .expect_err("a grid that does not exist is not guessed");
    assert!(
        error.to_string().contains("nowhere"),
        "the failure names the grid it looked for: {error}"
    );
}
