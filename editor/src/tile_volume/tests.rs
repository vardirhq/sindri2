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
        // Picking is plane arithmetic; depth only decides draw order.
        depth_step: 0.0,
        space: sindri_scene::TileSpace::Projected,
        cell_height: None,
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
    let point = outline.into_iter().fold([0.0, 0.0], |sum, point| {
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
        surface_target_at_viewport(&grid, Transform3D::default(), view, point, &volume, true, 0,),
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
    let point = outline.into_iter().fold([0.0, 0.0], |sum, point| {
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
        surface_target_at_viewport(&grid, Transform3D::default(), view, point, &volume, true, 0,),
        None
    );
}

/// A grid whose cells are boxes, which is what makes a face clickable.
fn solid_grid() -> TileGridComponent {
    TileGridComponent {
        columns: 4,
        rows: 4,
        cell_size: [1.0, 1.0],
        level_step: [0.0, 0.5],
        projection: TileProjection::Isometric,
        depth_step: 0.0,
        space: sindri_scene::TileSpace::Solid,
        cell_height: Some(1.0),
    }
}

/// Looking down at the blocks from straight above, so the top of one is what
/// is under the middle of the viewport.
fn overhead() -> glam::Mat4 {
    glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -4.0, 4.0, 0.1, 20.0)
        * glam::camera::rh::view::look_at_mat4(
            glam::Vec3::new(0.0, 10.0, 0.0),
            glam::Vec3::ZERO,
            // Looking straight down, so "up" on screen has to be something
            // other than the axis being looked along.
            glam::Vec3::NEG_Z,
        )
}

/// What the editor's hover does on a solid grid, without the editor.
///
/// The controls this replaces existed because the old picker met a horizontal
/// plane at a level somebody had to name: it could not say which block was
/// under the pointer, nor which of its sides. This says both.
#[test]
fn a_pointer_over_a_block_finds_that_block_and_the_side_it_is_looking_at() {
    let grid = solid_grid();
    let cell_size = grid.solid_cell().expect("a solid grid gives a cell box");
    let volume: sindri_scene::TileVolumeComponent = serde_json::from_value(json!({
        "tileset": "world.tileset.json",
        "cells": [{ "position": [0, 0, 0], "tile": "earth" }]
    }))
    .expect("the volume parses");

    let (origin, direction) =
        super::ray_at_viewport(Transform3D::default(), overhead(), [0.5, 0.5])
            .expect("the middle of the viewport casts a ray");
    let hit = sindri_scene::voxel::pick(&volume, cell_size, origin, direction, 512.0)
        .expect("the ray reaches the block under it");

    assert_eq!(hit.cell, GridCoord3::new(0, 0, 0));
    assert_eq!(
        hit.face,
        sindri_core::TileFace::Top,
        "looking down at a block is looking at its top"
    );
    // A click places against that face, which is the cell above it. Removing
    // takes the block itself. Two different cells from one pointer, which is
    // the whole of what a Place/Remove toggle used to have to be told.
    assert_eq!(hit.against(), GridCoord3::new(0, 0, 1));
}

/// A pointer over nothing is over nothing.
#[test]
fn a_pointer_off_the_blocks_finds_none_of_them() {
    let grid = solid_grid();
    let cell_size = grid.solid_cell().expect("a solid grid gives a cell box");
    let volume: sindri_scene::TileVolumeComponent = serde_json::from_value(json!({
        "tileset": "world.tileset.json",
        "cells": [{ "position": [0, 0, 0], "tile": "earth" }]
    }))
    .expect("the volume parses");

    let (origin, direction) =
        super::ray_at_viewport(Transform3D::default(), overhead(), [0.05, 0.5])
            .expect("a ray casts from anywhere in the viewport");
    assert!(
        sindri_scene::voxel::pick(&volume, cell_size, origin, direction, 512.0).is_none(),
        "a click into empty space placed something"
    );
}
