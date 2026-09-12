use serde_json::json;
use sindri_grid::GridCoord3;

use sindri_core::Transform3D;
use sindri_scene::{TileGridComponent, TileProjection};

use super::{TileBrush, cell_at_viewport, component, paint};

fn volume() -> serde_json::Value {
    json!({
        "tileset": "world.tileset.json",
        "cells": [{ "position": [1, 1, 0], "tile": "earth" }],
        "layer": 0
    })
}

#[test]
fn blocks_place_replace_and_erase_at_an_exact_level() {
    let mut payload = volume();
    let upper = GridCoord3::new(1, 1, 1);
    assert!(paint(&mut payload, upper, TileBrush::Tile("grass")).unwrap());
    assert!(paint(&mut payload, upper, TileBrush::Tile("stone")).unwrap());
    assert!(!paint(&mut payload, upper, TileBrush::Tile("stone")).unwrap());
    assert!(paint(&mut payload, upper, TileBrush::Erase).unwrap());
    assert!(!paint(&mut payload, upper, TileBrush::Erase).unwrap());

    let decoded = component(&payload).unwrap();
    assert_eq!(decoded.cells.len(), 1);
    assert_eq!(decoded.cells[0].tile, "earth");
}

#[test]
fn serialized_cells_have_a_stable_zyx_order() {
    let mut payload = volume();
    paint(
        &mut payload,
        GridCoord3::new(0, 2, 0),
        TileBrush::Tile("grass"),
    )
    .unwrap();
    paint(
        &mut payload,
        GridCoord3::new(0, 0, 2),
        TileBrush::Tile("stone"),
    )
    .unwrap();
    let positions = payload["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cell| cell["position"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![json!([1, 1, 0]), json!([0, 2, 0]), json!([0, 0, 2])]
    );
}

#[test]
fn the_same_screen_point_addresses_the_chosen_level() {
    let grid = TileGridComponent {
        columns: 4,
        rows: 4,
        cell_size: [1.0, 0.5],
        level_step: [0.0, 0.5],
        projection: TileProjection::Isometric,
    };
    let view = glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -4.0, 4.0, 0.1, 10.0)
        * glam::camera::rh::view::look_at_mat4(
            glam::Vec3::new(0.0, 0.0, 5.0),
            glam::Vec3::ZERO,
            glam::Vec3::Y,
        );
    let point = [0.5, 0.5];
    let low = cell_at_viewport(&grid, Transform3D::default(), view, point, 0).unwrap();
    let high = cell_at_viewport(&grid, Transform3D::default(), view, point, 1).unwrap();
    assert_eq!(low.z, 0);
    assert_eq!(high.z, 1);
    assert_ne!((low.x, low.y), (high.x, high.y));
}
