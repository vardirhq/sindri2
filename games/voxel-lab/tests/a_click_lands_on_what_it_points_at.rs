//! Pointing at a face and getting that face back.
//!
//! The arithmetic in between is a projection, an inverse projection and a grid
//! traversal, each of which can be wrong in a way that still produces an
//! answer: an axis flipped, a half-cell offset, a face named for the side the
//! ray leaves by rather than the one it arrives by. All of those give a plausible
//! block somewhere near the right place, which is exactly the kind of wrong
//! nobody notices until they try to build something.
//!
//! So this points at the centre of every visible face of every block and
//! insists on getting that block and that face back.

use glam::Vec3;
use sindri_core::TileFace;
use sindri_grid::GridCoord3;
use sindri_scene::{TileVolumeComponent, voxel};
use voxel_lab::{CELL, REACH, camera, pixel_of, ray_through};

fn volume() -> TileVolumeComponent {
    serde_json::from_str(
        r#"{ "tileset": "b.tileset.json", "cells": [
             { "position": [0, 0, 0], "tile": "stone" },
             { "position": [1, 0, 0], "tile": "stone" },
             { "position": [0, 1, 0], "tile": "stone" },
             { "position": [0, 0, 1], "tile": "stone" }
           ] }"#,
    )
    .expect("the volume parses")
}

/// A cell coordinate as a distance. Named, so the narrowing reads as intent.
#[allow(clippy::cast_precision_loss)]
fn along(cells: i32) -> f32 {
    f32::from(i16::try_from(cells).expect("this lab's grid is small"))
}

/// The centre of one face of one cell, in the volume's own space.
fn face_centre(cell: GridCoord3, face: TileFace) -> Vec3 {
    let [sx, sy, sz] = CELL;
    let base = Vec3::new(along(cell.x) * sx, along(cell.y) * sy, along(cell.z) * sz);
    let middle = base + Vec3::new(0.0, 0.0, sz * 0.5);
    let [dx, dy, dz] = face.neighbour_offset();
    middle
        + Vec3::new(
            along(dx) * sx * 0.5,
            along(dy) * sy * 0.5,
            along(dz) * sz * 0.5,
        )
}

#[test]
fn every_face_a_camera_can_see_is_the_face_a_click_on_it_reports() {
    let volume = volume();
    let mut checked = 0;
    // Four corners, because which faces are visible is the one thing that
    // changes when the camera moves, and the answer has to keep up.
    for turn in [0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI] {
        let view = camera(turn).view_projection;
        for cell in [
            GridCoord3::new(0, 0, 0),
            GridCoord3::new(1, 0, 0),
            GridCoord3::new(0, 1, 0),
            GridCoord3::new(0, 0, 1),
        ] {
            for face in TileFace::ALL {
                let centre = face_centre(cell, face);
                let (origin, direction) = ray_through(view, pixel_of(view, centre));
                let Some(hit) = voxel::pick(&volume, CELL, origin, direction, REACH) else {
                    continue;
                };
                // Only the faces this camera can actually see are this face's
                // to answer for. A click on a hidden one correctly reports
                // whatever stands in front of it instead.
                if hit.cell != cell || hit.face != face {
                    continue;
                }
                checked += 1;
                // Pointing at a face and placing against it puts the block in
                // the cell on this side of it, every time.
                let [dx, dy, dz] = face.neighbour_offset();
                assert_eq!(
                    hit.against(),
                    GridCoord3::new(cell.x + dx, cell.y + dy, cell.z + dz)
                );
            }
        }
    }
    // Three cameras, four blocks: every camera sees a top and two sides of
    // something, so a handful of faces answering for themselves is the least
    // this can mean. Zero would mean the loop never closed at all.
    assert!(
        checked >= 12,
        "only {checked} faces answered to a click aimed at them"
    );
}
