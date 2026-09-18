//! A grid whose cells are boxes: a cell is a place in the world rather than a
//! picture of one.

use serde_json::json;
use sindri_core::{SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, Transform3D, World};
use sindri_scene::{GridSurfaces, SceneExtractor, resolve_grid_placements};

use crate::support::{id, placed, tile_sets, transform_of};

/// The same four-by-four ground, on a grid whose cells are boxes.
fn solid_world_with(entities: Vec<SceneEntity>) -> (World, SceneExtractor) {
    let mut floor = SceneEntity::new(id("floor"));
    floor.transform_3d = Some(Transform3D {
        position: [3.0, 0.0, -2.0],
        ..Transform3D::default()
    });
    floor.components.insert(
        "sindri.tile_grid".to_owned(),
        json!({
            "columns": 4, "rows": 4, "cell_size": [1.1, 1.1],
            "cell_height": 1.1, "projection": "isometric", "space": "solid"
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

#[test]
fn a_prop_on_a_solid_grid_stands_where_its_cell_is_in_the_world() {
    // No projection, no derived depth, no plane: a column is a distance
    // across, a row is a distance into the scene, and the ground's height is a
    // height. The floor sits away from the origin so that a resolver quietly
    // ignoring the grid's own position would be caught here rather than found
    // later in a picture.
    let (mut world, extractor) = solid_world_with(vec![placed("rock", &json!([2, 1]))]);
    let resolved = resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&tile_sets()),
        &mut GridSurfaces::default(),
    )
    .expect("the placement resolves");
    assert_eq!(resolved, 1);

    let position = transform_of(&world, "rock").position;
    // The ground fills level -1, so its top stands at height zero.
    let expected = [3.0 + 2.0 * 1.1, 0.0, -2.0 + 1.0 * 1.1];
    assert!(
        position
            .iter()
            .zip(expected)
            .all(|(had, want)| (had - want).abs() < 1.0e-5),
        "a prop on a solid grid stands in its cell: {position:?} vs {expected:?}"
    );
}

#[test]
fn a_prop_on_a_solid_grid_rides_the_ground_it_stands_on() {
    // Height is the ground's answer rather than the prop's. A block laid on
    // top of the one the prop names lifts the prop by exactly one cell, which
    // is the whole reason placement consults the surface at all.
    let (mut world, extractor) = solid_world_with(vec![placed("rock", &json!([2, 1]))]);
    let sets = tile_sets();
    resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&sets),
        &mut GridSurfaces::default(),
    )
    .unwrap();
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
    let mut volume = world
        .get(floor)
        .and_then(|data| data.components.get("sindri.tile_volume").cloned())
        .unwrap();
    volume["cells"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "position": [2, 1, 0], "tile": "block" }));
    world
        .get_mut(floor)
        .unwrap()
        .components
        .insert("sindri.tile_volume".to_owned(), volume);

    resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&sets),
        &mut GridSurfaces::default(),
    )
    .unwrap();
    let after = transform_of(&world, "rock").position[1];
    assert!(
        (after - before - 1.1).abs() < 1.0e-5,
        "a block under the prop lifts it by one cell: {before} then {after}"
    );
}
