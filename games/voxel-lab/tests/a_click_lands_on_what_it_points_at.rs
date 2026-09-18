//! Pointing at a face and getting that face back.
//!
//! The arithmetic in between is a projection, an inverse projection and a grid
//! traversal, each of which can be wrong in a way that still produces an
//! answer: an axis flipped, a half-cell offset, a face named for the side the
//! ray leaves by rather than the one it arrives by. All of those give a plausible
//! block somewhere near the right place, which is exactly the kind of wrong
//! nobody notices until they try to build something.
//!
//! So this points at every side of a lone block, from four corners, and
//! insists on two things: the three sides facing the camera each answer to a
//! click aimed at them, and the three facing away answer to none. One block
//! rather than a heap, because with neighbours most faces are hidden and a
//! count of how many answered would be measuring the arrangement rather than
//! the arithmetic.

use glam::Vec3;
use sindri_core::TileFace;
use sindri_grid::GridCoord3;
use sindri_scene::{TileVolumeComponent, voxel};
use voxel_lab::{CELL, REACH, camera, pixel_of, ray_through};

fn volume() -> TileVolumeComponent {
    serde_json::from_str(
        r#"{ "tileset": "b.tileset.json", "cells": [
             { "position": [0, 0, 0], "tile": "stone" }
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
///
/// A cell's coordinates are a column, a row and a level; the world's are X
/// across, Y up and Z into the scene. So a level becomes a height and a row
/// becomes a depth, here and everywhere else.
fn face_centre(cell: GridCoord3, face: TileFace) -> Vec3 {
    let [across, into, up] = CELL;
    let floor = Vec3::new(
        along(cell.x) * across,
        along(cell.z) * up,
        along(cell.y) * into,
    );
    let middle = floor + Vec3::new(0.0, up * 0.5, 0.0);
    let [dx, dy, dz] = face.neighbour_offset();
    middle
        + Vec3::new(
            along(dx) * across * 0.5,
            along(dz) * up * 0.5,
            along(dy) * into * 0.5,
        )
}

/// Which way a face points, in the world.
fn normal_of(face: TileFace) -> Vec3 {
    let [dx, dy, dz] = face.neighbour_offset();
    Vec3::new(along(dx), along(dz), along(dy))
}

#[test]
fn the_sides_facing_the_camera_are_the_ones_a_click_can_reach() {
    let volume = volume();
    let cell = GridCoord3::new(0, 0, 0);
    for turn in [
        0.0,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::PI,
        std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
    ] {
        let view = camera(turn).view_projection;
        let mut answered = Vec::new();
        for face in TileFace::ALL {
            let centre = face_centre(cell, face);
            let (origin, direction) = ray_through(view, pixel_of(view, centre));
            // Which way the camera is, from the block: a face pointing back
            // along the ray is one the camera is looking at.
            let towards_camera = -direction.normalize();
            let facing = normal_of(face).dot(towards_camera) > 0.1;

            let hit = voxel::pick(&volume, CELL, origin, direction, REACH);
            match hit {
                Some(hit) if facing => {
                    assert_eq!(hit.cell, cell, "a lone block is the only thing to hit");
                    assert_eq!(hit.face, face, "aimed at {face:?} and got {:?}", hit.face);
                    // Pointing at a face and placing against it puts the new
                    // block on this side of it, which is the whole gesture.
                    let [dx, dy, dz] = face.neighbour_offset();
                    assert_eq!(
                        hit.against(),
                        GridCoord3::new(cell.x + dx, cell.y + dy, cell.z + dz)
                    );
                    answered.push(face);
                }
                // A face pointing away is behind the block. The ray reaches it
                // only by passing through, and reports the side it came in by.
                Some(hit) => assert_ne!(
                    hit.face, face,
                    "{face:?} points away from this camera and answered anyway"
                ),
                None => panic!("a ray aimed at {face:?} of a lone block reached nothing"),
            }
        }
        assert_eq!(
            answered.len(),
            3,
            "a cube shows three sides to any corner; this one showed {answered:?}"
        );
    }
}
