//! Cells as geometry, and rays as clicks.

use super::*;
use crate::voxel::picking::pick;

fn tile_set(extra: &str) -> TileSetDocument {
    TileSetDocument::from_json(&format!(
        r#"{{ "format_version": 1, "tiles": {{
             "stone": {{ "faces": {{
               "top":    {{ "sprite": "b.png#top",  "size": [1.0, 1.0] }},
               "south":  {{ "sprite": "b.png#side", "size": [1.0, 1.0] }},
               "east":   {{ "sprite": "b.png#side", "size": [1.0, 1.0] }},
               "bottom": {{ "sprite": "b.png#top",  "size": [1.0, 1.0] }}
             }} }}{extra}
           }} }}"#
    ))
    .expect("the tile set parses")
}

fn volume(cells: &str) -> TileVolumeComponent {
    serde_json::from_str(&format!(
        r#"{{ "tileset": "b.tileset.json", "cells": [{cells}] }}"#
    ))
    .expect("the volume parses")
}

fn faces_of(cells: &str, extra: &str) -> Vec<VoxelFace> {
    cube_faces(&volume(cells), &tile_set(extra), [1.0, 1.0, 1.0]).expect("the cells resolve")
}

#[test]
fn a_lone_block_shows_all_six_of_its_sides() {
    let faces = faces_of(r#"{ "position": [0, 0, 0], "tile": "stone" }"#, "");
    assert_eq!(faces.len(), 6);
    // Every side of a cell at the origin is half a cell from its centre,
    // in the direction that side faces.
    for face in &faces {
        let centre = face.model.col(3).truncate();
        let (normal, _, _) = basis_of(face.face);
        // The cell at the origin stands on y = 0 and fills one unit, so its
        // middle is half a unit up and each side half a unit from there.
        assert!(
            (centre - (Vec3::new(0.0, 0.5, 0.0) + normal * 0.5)).length() < 1e-5,
            "{:?} sits at {centre:?}",
            face.face
        );
    }
}

#[test]
fn a_face_with_a_block_against_it_is_not_drawn_at_all() {
    // Two stacked cells: ten sides, not twelve. The pair that meet in the
    // middle is the economy the whole approach depends on -- an island is
    // mostly its own inside.
    let faces = faces_of(
        r#"{ "position": [0, 0, 0], "tile": "stone" },
           { "position": [0, 0, 1], "tile": "stone" }"#,
        "",
    );
    assert_eq!(faces.len(), 10);
    assert!(
        !faces
            .iter()
            .any(|face| face.cell.z == 0 && face.face == TileFace::Top),
        "the lower block's top is under the upper one"
    );
    assert!(
        !faces
            .iter()
            .any(|face| face.cell.z == 1 && face.face == TileFace::Bottom),
        "the upper block's underside is on the lower one"
    );
}

#[test]
fn a_half_height_neighbour_hides_only_what_it_covers() {
    // A slab does not reach the cell above it, so the block resting on the
    // slab still has an underside to draw, and the slab still has a top.
    let extra = r#", "slab": { "height": 0.5, "faces": {
         "top":    { "sprite": "b.png#top",  "size": [1.0, 1.0] },
         "south":  { "sprite": "b.png#side", "size": [1.0, 1.0] },
         "east":   { "sprite": "b.png#side", "size": [1.0, 1.0] },
         "bottom": { "sprite": "b.png#top",  "size": [1.0, 1.0] }
       } }"#;
    let faces = faces_of(
        r#"{ "position": [0, 0, 0], "tile": "slab" },
           { "position": [0, 0, 1], "tile": "stone" }"#,
        extra,
    );
    assert_eq!(faces.len(), 12, "nothing is hidden between them");

    // And the slab is half as tall, which its sides have to say.
    let side = faces
        .iter()
        .find(|face| face.cell.z == 0 && face.face == TileFace::South)
        .expect("the slab has a south face");
    assert!(
        (side.model.col(1).length() - 0.5).abs() < 1e-5,
        "a slab's side is half a cell tall, not {}",
        side.model.col(1).length()
    );
}

/// Looking down at a block from above.
#[test]
fn a_ray_from_above_arrives_at_the_top_of_the_block_under_it() {
    let volume = volume(r#"{ "position": [0, 0, 0], "tile": "stone" }"#);
    let hit = pick(
        &volume,
        [1.0, 1.0, 1.0],
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        32.0,
    )
    .expect("the ray reaches the block");
    assert_eq!(hit.cell, GridCoord3::new(0, 0, 0));
    assert_eq!(hit.face, TileFace::Top);
    // Which is what "click the top of a block to stack on it" means.
    assert_eq!(hit.against(), GridCoord3::new(0, 0, 1));
}

/// And at its side, which is the half a flat plane could never answer.
#[test]
fn a_ray_from_the_west_arrives_at_the_west_side() {
    let volume = volume(r#"{ "position": [0, 0, 0], "tile": "stone" }"#);
    let hit = pick(
        &volume,
        [1.0, 1.0, 1.0],
        Vec3::new(-5.0, 0.5, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        32.0,
    )
    .expect("the ray reaches the block");
    assert_eq!(hit.cell, GridCoord3::new(0, 0, 0));
    assert_eq!(hit.face, TileFace::West);
    assert_eq!(hit.against(), GridCoord3::new(-1, 0, 0));
}

/// The near block, not the one behind it.
#[test]
fn a_ray_stops_at_the_first_block_it_meets() {
    let volume = volume(
        r#"{ "position": [0, 0, 0], "tile": "stone" },
           { "position": [3, 0, 0], "tile": "stone" }"#,
    );
    let hit = pick(
        &volume,
        [1.0, 1.0, 1.0],
        Vec3::new(-5.0, 0.5, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        32.0,
    )
    .expect("the ray reaches a block");
    assert_eq!(hit.cell, GridCoord3::new(0, 0, 0), "the far block answered");
}

#[test]
fn a_ray_that_misses_everything_hits_nothing() {
    let volume = volume(r#"{ "position": [0, 0, 0], "tile": "stone" }"#);
    // Parallel to the ground, a level above the only block there is.
    assert!(
        pick(
            &volume,
            [1.0, 1.0, 1.0],
            Vec3::new(-5.0, 1.5, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            32.0,
        )
        .is_none()
    );
}

#[test]
fn a_block_beyond_reach_is_not_picked() {
    let volume = volume(r#"{ "position": [20, 0, 0], "tile": "stone" }"#);
    let ray = (Vec3::new(-1.0, 0.5, 0.0), Vec3::new(1.0, 0.0, 0.0));
    assert!(pick(&volume, [1.0, 1.0, 1.0], ray.0, ray.1, 5.0).is_none());
    assert!(pick(&volume, [1.0, 1.0, 1.0], ray.0, ray.1, 64.0).is_some());
}

/// Cells are not cubes when the grid is not cubic, and the traversal has
/// to answer in cells rather than in distance.
#[test]
fn a_grid_of_flatter_cells_still_picks_the_right_one() {
    let volume = volume(r#"{ "position": [2, 0, 0], "tile": "stone" }"#);
    let hit = pick(
        &volume,
        [1.0, 1.0, 0.5],
        Vec3::new(-5.0, 0.25, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        32.0,
    )
    .expect("the ray reaches the block");
    assert_eq!(hit.cell, GridCoord3::new(2, 0, 0));
    assert_eq!(hit.face, TileFace::West);
}

#[test]
fn a_face_the_art_never_drew_is_still_drawn() {
    // The tile set above names `top`, `south`, `east` and `bottom` only,
    // which is what a block drawn for one fixed viewpoint has. All six
    // sides still come back, because a cube a camera can go round needs
    // them and the missing ones borrow their opposite.
    let faces = faces_of(r#"{ "position": [0, 0, 0], "tile": "stone" }"#, "");
    for face in TileFace::ALL {
        assert!(
            faces.iter().any(|drawn| drawn.face == face),
            "{face:?} was left undrawn"
        );
    }
}

/// Occlusion is counted, not lit.
///
/// No light is traced and no light source exists: a corner is dark in
/// proportion to how many blocks crowd it, which is a property of the blocks
/// and so as static as they are.
#[test]
fn a_corner_darkens_with_the_blocks_that_crowd_it() {
    // Nothing near it: every corner of a lone block is fully lit.
    let alone = faces_of(r#"{ "position": [0, 0, 0], "tile": "stone" }"#, "");
    for face in &alone {
        assert!(
            face.corners.iter().all(|value| *value >= 1.0),
            "{:?} of a block standing on its own is shaded: {:?}",
            face.face,
            face.corners
        );
    }

    // A block beside it at the same level shades nothing: the two tops are
    // level with each other, so neither stands over the other.
    let level = faces_of(
        r#"{ "position": [0, 0, 0], "tile": "stone" },
           { "position": [1, 0, 0], "tile": "stone" }"#,
        "",
    );
    let flat = level
        .iter()
        .find(|face| face.cell == GridCoord3::new(0, 0, 0) && face.face == TileFace::Top)
        .expect("the block has a top");
    assert!(
        flat.corners.iter().all(|value| *value >= 1.0),
        "a floor shaded its own neighbour: {:?}",
        flat.corners
    );

    // Raise that neighbour and it does: the top's two corners along the edge
    // it now stands over darken, and the two away from it do not.
    let pair = faces_of(
        r#"{ "position": [0, 0, 0], "tile": "stone" },
           { "position": [1, 0, 1], "tile": "stone" }"#,
        "",
    );
    let top = pair
        .iter()
        .find(|face| face.cell == GridCoord3::new(0, 0, 0) && face.face == TileFace::Top)
        .expect("the lower block has a top");
    let lit = top.corners.iter().filter(|value| **value >= 1.0).count();
    assert_eq!(lit, 2, "one neighbour shades one edge: {:?}", top.corners);

    // And an inside corner, where two blocks and the one diagonally between
    // them all meet: the darkest a corner gets.
    let corner = faces_of(
        r#"{ "position": [0, 0, 0], "tile": "stone" },
           { "position": [1, 0, 1], "tile": "stone" },
           { "position": [0, 1, 1], "tile": "stone" },
           { "position": [1, 1, 1], "tile": "stone" }"#,
        "",
    );
    let top = corner
        .iter()
        .find(|face| face.cell == GridCoord3::new(0, 0, 0) && face.face == TileFace::Top)
        .expect("the floor block has a top");
    assert!(
        top.corners
            .iter()
            .any(|value| (*value - AO_LEVELS[0]).abs() < 1e-6),
        "a corner boxed in on both sides is not at its darkest: {:?}",
        top.corners
    );
}

/// Two blocks meeting shut a corner completely, and the diagonal behind them
/// cannot make it darker. Without this a block nobody can see changes one that
/// is visible.
#[test]
fn two_blocks_closing_a_corner_hide_whatever_is_behind_them() {
    assert!((corner_light(true, true, false) - AO_LEVELS[0]).abs() < 1e-6);
    assert!((corner_light(true, true, true) - AO_LEVELS[0]).abs() < 1e-6);
    // And the steps between: nothing, a diagonal only, one side, one side and
    // a diagonal.
    assert!((corner_light(false, false, false) - AO_LEVELS[3]).abs() < 1e-6);
    assert!((corner_light(false, false, true) - AO_LEVELS[2]).abs() < 1e-6);
    assert!((corner_light(true, false, false) - AO_LEVELS[2]).abs() < 1e-6);
    assert!((corner_light(true, false, true) - AO_LEVELS[1]).abs() < 1e-6);
}
