use serde_json::json;
use sindri_grid::GridCoord3;

use sindri_core::Transform3D;
use sindri_scene::{TileGridComponent, TileProjection};

use super::{
    TileBrush, cell_at_viewport, cell_outline, component, paint, surface_cell_at_viewport,
    surface_target_at_viewport,
};

fn volume() -> serde_json::Value {
    json!({
        "tileset": "world.tileset.json",
        "cells": [{ "position": [1, 1, 0], "tile": "earth" }],
        "layer": 0
    })
}

fn grid() -> TileGridComponent {
    TileGridComponent {
        columns: 4,
        rows: 4,
        cell_size: [1.0, 0.5],
        level_step: [0.0, 0.5],
        projection: TileProjection::Isometric,
    }
}

fn view() -> glam::Mat4 {
    glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -4.0, 4.0, 0.1, 10.0)
        * glam::camera::rh::view::look_at_mat4(
            glam::Vec3::new(0.0, 0.0, 5.0),
            glam::Vec3::ZERO,
            glam::Vec3::Y,
        )
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
    let grid = grid();
    let view = view();
    let point = [0.5, 0.5];
    let low = cell_at_viewport(&grid, Transform3D::default(), view, point, 0).unwrap();
    let high = cell_at_viewport(&grid, Transform3D::default(), view, point, 1).unwrap();
    assert_eq!(low.z, 0);
    assert_eq!(high.z, 1);
    assert_ne!((low.x, low.y), (high.x, high.y));
}

#[test]
fn surface_pick_chooses_the_exposed_top_of_a_stack() {
    let grid = grid();
    let view = view();
    let mut payload = volume();
    paint(
        &mut payload,
        GridCoord3::new(1, 1, 1),
        TileBrush::Tile("grass"),
    )
    .unwrap();
    let volume = component(&payload).unwrap();
    let upper = GridCoord3::new(1, 1, 1);
    let outline = cell_outline(&grid, Transform3D::default(), view, upper).unwrap();
    let point = outline
        .into_iter()
        .fold([0.0, 0.0], |sum, point| {
            [sum[0] + point[0] * 0.25, sum[1] + point[1] * 0.25]
        });

    assert_eq!(
        surface_cell_at_viewport(&grid, Transform3D::default(), view, point, &volume),
        Some(upper)
    );
    assert_eq!(
        surface_target_at_viewport(
            &grid,
            Transform3D::default(),
            view,
            point,
            &volume,
            false,
            0,
        ),
        Some(GridCoord3::new(1, 1, 2))
    );
    assert_eq!(
        surface_target_at_viewport(
            &grid,
            Transform3D::default(),
            view,
            point,
            &volume,
            true,
            0,
        ),
        Some(upper)
    );
}

#[test]
fn empty_surface_space_can_start_a_foundation_but_cannot_erase_it() {
    let grid = grid();
    let view = view();
    let volume = component(&volume()).unwrap();
    let empty = GridCoord3::new(3, 3, 0);
    let outline = cell_outline(&grid, Transform3D::default(), view, empty).unwrap();
    let point = outline
        .into_iter()
        .fold([0.0, 0.0], |sum, point| {
            [sum[0] + point[0] * 0.25, sum[1] + point[1] * 0.25]
        });

    assert_eq!(
        surface_target_at_viewport(
            &grid,
            Transform3D::default(),
            view,
            point,
            &volume,
            false,
            0,
        ),
        Some(empty)
    );
    assert_eq!(
        surface_target_at_viewport(
            &grid,
            Transform3D::default(),
            view,
            point,
            &volume,
            true,
            0,
        ),
        None
    );
}
