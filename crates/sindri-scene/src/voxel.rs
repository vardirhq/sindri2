//! A tile volume as solid geometry rather than as a picture of one.
//!
//! The other path through a volume resolves its cells into flat quads already
//! arranged for one fixed isometric viewpoint. It draws well from that
//! viewpoint and from nowhere else, because the arrangement *is* the illusion:
//! turn the camera and the faces are still facing where the camera used to be.
//!
//! This is the same cells as a shape. A cell becomes a box in the volume's own
//! space, X across, Y into the scene and Z up, and each of its six sides
//! becomes a quad facing outward. Nothing here knows where the camera is, and
//! that is the point: what covers what stops being an order somebody has to
//! compute and becomes a depth comparison the GPU already does.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use sindri_core::{TileFace, TileSetDocument};
use sindri_grid::GridCoord3;
use thiserror::Error;

use crate::components::TileVolumeComponent;

/// How much light a face gets for facing the way it does.
///
/// A cube of one flat colour reads as a hexagon, not a block: the eye takes
/// the shading, not the silhouette, as the evidence of a third dimension. So
/// the top is lit fully, the two pairs of sides progressively less, and the
/// underside least.
///
/// Applied here rather than baked into the art, which matters as soon as the
/// camera can move: baked shading belongs to the angle it was baked from, and
/// a block lit for the south-east would keep its bright face pointing there
/// while the viewer walked round to the north-west.
const fn shade_of(face: TileFace) -> f32 {
    match face {
        TileFace::Top => 1.0,
        TileFace::South | TileFace::North => 0.82,
        TileFace::East | TileFace::West => 0.68,
        TileFace::Bottom => 0.55,
    }
}

/// The outward normal, and the two axes the quad's own X and Y run along.
///
/// `right` crossed with `up` gives the normal in every case, so every face is
/// wound the same way round and a renderer that culls backfaces culls the ones
/// pointing into the block.
const fn basis_of(face: TileFace) -> (Vec3, Vec3, Vec3) {
    match face {
        TileFace::Top => (Vec3::Z, Vec3::X, Vec3::Y),
        TileFace::Bottom => (Vec3::NEG_Z, Vec3::X, Vec3::NEG_Y),
        TileFace::South => (Vec3::Y, Vec3::NEG_X, Vec3::Z),
        TileFace::North => (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        TileFace::East => (Vec3::X, Vec3::Y, Vec3::Z),
        TileFace::West => (Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z),
    }
}

/// One side of one cell, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct VoxelFace {
    pub cell: GridCoord3,
    pub face: TileFace,
    /// Places the renderer's unit quad — centred, one unit across — onto this
    /// side of this cell, in the volume's own space.
    pub model: Mat4,
    /// The sprite this side draws, as the tile set names it.
    pub sprite: String,
    /// What the face's own direction does to its brightness.
    pub shade: f32,
}

#[derive(Debug, Error)]
pub enum VoxelError {
    #[error("cell {cell:?} holds tile `{tile}`, which the tile set does not define")]
    UnknownTile { cell: GridCoord3, tile: String },
}

/// Every face of every cell that something could see.
///
/// A face with a neighbour against it is not returned at all. That is the
/// whole economy of drawing blocks: an island is mostly its own inside, and
/// the inside is never looked at. The test is not "is there a neighbour" but
/// "does the neighbour cover this", so a slab beside a full block hides the
/// bottom half of its side and leaves the top half to be drawn.
///
/// `cell_size` is the box one cell occupies, in the volume's own units. It is
/// three numbers rather than a tilemap's two because a cell is a box here, not
/// a diamond that a projection has already flattened.
pub fn cube_faces(
    volume: &TileVolumeComponent,
    tile_set: &TileSetDocument,
    cell_size: [f32; 3],
) -> Result<Vec<VoxelFace>, VoxelError> {
    let [sx, sy, sz] = cell_size;
    // Its own map rather than the volume's index, which is keyed off a grid:
    // a grid describes the diamond a cell projects to, and nothing here
    // projects anything.
    let cells: BTreeMap<GridCoord3, &str> = volume.occupied().collect();
    let height_at = |cell: GridCoord3| -> Option<f32> {
        let definition = tile_set.tile(cells.get(&cell).copied()?)?;
        definition.occludes.then_some(definition.height)
    };

    let mut faces = Vec::new();
    for (cell, tile) in volume.occupied() {
        let definition = tile_set.tile(tile).ok_or_else(|| VoxelError::UnknownTile {
            cell,
            tile: tile.to_owned(),
        })?;
        let fill = definition.height.clamp(0.0, 1.0);
        if fill <= 0.0 {
            continue;
        }
        // The cell's box: centred on its column and row, standing on its level.
        let base = Vec3::new(axis(cell.x) * sx, axis(cell.y) * sy, axis(cell.z) * sz);
        let tall = fill * sz;

        for face in TileFace::ALL {
            let [dx, dy, dz] = face.neighbour_offset();
            let neighbour = GridCoord3::new(cell.x + dx, cell.y + dy, cell.z + dz);
            if covers(face, fill, height_at(neighbour)) {
                continue;
            }
            let Some((_, visual)) = definition.faces.resolved(face) else {
                continue;
            };
            let (normal, right, up) = basis_of(face);
            // A side is as tall as the tile fills its cell; a top and a bottom
            // are the cell's full footprint however thin the tile is.
            let (width, height) = match face {
                TileFace::Top | TileFace::Bottom => (sx * right.x.abs() + sy * right.y.abs(), sy),
                _ => (sx * right.x.abs() + sy * right.y.abs(), tall),
            };
            let centre = base
                + Vec3::new(0.0, 0.0, tall * 0.5)
                + normal
                    * match face {
                        // Up and down are half the tile's own height away,
                        // which is what makes a slab's top lower than a block's.
                        TileFace::Top | TileFace::Bottom => tall * 0.5,
                        TileFace::North | TileFace::South => sy * 0.5,
                        TileFace::East | TileFace::West => sx * 0.5,
                    };
            faces.push(VoxelFace {
                cell,
                face,
                model: Mat4::from_cols(
                    (right * width).extend(0.0),
                    (up * height).extend(0.0),
                    normal.extend(0.0),
                    centre.extend(1.0),
                ),
                sprite: visual.sprite.clone(),
                shade: shade_of(face),
            });
        }
    }
    Ok(faces)
}

/// A grid coordinate as a distance.
///
/// Named so the narrowing reads as the intent it is. A volume whose cells run
/// past eight million in any direction has lost more than float precision.
#[allow(clippy::cast_precision_loss)]
fn axis(value: i32) -> f32 {
    value as f32
}

/// Whether a neighbour filling `neighbour` hides this face of a tile that
/// fills `fill` of its own cell.
///
/// Sideways, a neighbour has to be at least as tall to cover the whole side.
/// Up and down, only a tile filling its cell reaches the boundary at all: a
/// slab's top is in the middle of its cell, so whatever sits in the cell above
/// is not against it.
fn covers(face: TileFace, fill: f32, neighbour: Option<f32>) -> bool {
    let Some(neighbour) = neighbour else {
        return false;
    };
    match face {
        TileFace::Top => fill >= 1.0 && neighbour > 0.0,
        TileFace::Bottom => neighbour >= 1.0,
        _ => neighbour >= fill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(
                (centre - (Vec3::new(0.0, 0.0, 0.5) + normal * 0.5)).length() < 1e-5,
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
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, -1.0),
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
            Vec3::new(-5.0, 0.0, 0.5),
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
            Vec3::new(-5.0, 0.0, 0.5),
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
                Vec3::new(-5.0, 0.0, 1.5),
                Vec3::new(1.0, 0.0, 0.0),
                32.0,
            )
            .is_none()
        );
    }

    #[test]
    fn a_block_beyond_reach_is_not_picked() {
        let volume = volume(r#"{ "position": [20, 0, 0], "tile": "stone" }"#);
        let ray = (Vec3::new(-1.0, 0.0, 0.5), Vec3::new(1.0, 0.0, 0.0));
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
            Vec3::new(-5.0, 0.0, 0.25),
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
}

/// Which block a ray reaches, and the side it arrives through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoxelHit {
    /// The block the ray struck.
    pub cell: GridCoord3,
    /// The side it entered by — the face you are looking at.
    pub face: TileFace,
}

impl VoxelHit {
    /// The empty cell against this face: where a block placed here would go.
    ///
    /// This is the whole of what "click a face to attach a block" means, and
    /// the reason picking has to report a face rather than a cell. A cell
    /// alone cannot say which of its six neighbours you meant.
    #[must_use]
    pub fn against(self) -> GridCoord3 {
        let [dx, dy, dz] = self.face.neighbour_offset();
        GridCoord3::new(self.cell.x + dx, self.cell.y + dy, self.cell.z + dz)
    }
}

/// The first block a ray meets, walking the grid cell by cell.
///
/// Amanatides and Woo's traversal: step to whichever axis boundary is nearest,
/// which visits every cell the ray passes through and no others. The cost is
/// the length of the ray in cells rather than the number of cells there are,
/// so picking against a large volume costs what picking against a small one
/// does.
///
/// This replaces intersecting a horizontal plane at a level the caller had to
/// name. That is why the editor grew a `Level` field, a `Target` mode and a
/// `Place`/`Remove` pair: a plane cannot say which block you clicked, whether
/// a block is there at all, or which of its sides you are looking at, so every
/// one of those had to be answered by hand first.
///
/// Blocks are picked as whole cells even where a tile fills less of one. A
/// slab's cell answers for the slab, which is right for placing against it and
/// wrong only for a ray passing through the empty air above it -- an error of
/// half a cell, on a shape you can see, in exchange for the traversal staying
/// the simple thing it is.
///
/// `reach` bounds the walk in cells. Nothing is hit beyond it, which is what
/// stops a ray aimed at the sky from walking to the horizon.
#[must_use]
pub fn pick(
    volume: &TileVolumeComponent,
    cell_size: [f32; 3],
    origin: Vec3,
    direction: Vec3,
    reach: f32,
) -> Option<VoxelHit> {
    let cells: BTreeMap<GridCoord3, &str> = volume.occupied().collect();
    if cells.is_empty() {
        return None;
    }
    // Into a space where every cell is the unit cube from its own coordinate:
    // X and Y are centred on their column and row, Z stands on its level, and
    // the traversal below need know none of that.
    let [sx, sy, sz] = cell_size;
    if sx <= 0.0 || sy <= 0.0 || sz <= 0.0 {
        return None;
    }
    let start = Vec3::new(origin.x / sx + 0.5, origin.y / sy + 0.5, origin.z / sz);
    let step_by = Vec3::new(direction.x / sx, direction.y / sy, direction.z / sz);
    if !start.is_finite() || !step_by.is_finite() || step_by.length_squared() <= 0.0 {
        return None;
    }

    let mut cell = [cell_of(start.x), cell_of(start.y), cell_of(start.z)];
    let mut next = [0.0_f32; 3];
    let mut delta = [0.0_f32; 3];
    let mut step = [0_i64; 3];
    // Which face the ray enters by, per axis, given the way it is going: a ray
    // travelling east enters through a block's west side.
    let entering = [
        [TileFace::East, TileFace::West],
        [TileFace::South, TileFace::North],
        [TileFace::Top, TileFace::Bottom],
    ];
    for axis in 0..3 {
        let at = start[axis];
        let towards = step_by[axis];
        if towards > 0.0 {
            step[axis] = 1;
            next[axis] = (boundary(cell[axis] + 1) - at) / towards;
            delta[axis] = 1.0 / towards;
        } else if towards < 0.0 {
            step[axis] = -1;
            next[axis] = (at - boundary(cell[axis])) / -towards;
            delta[axis] = -1.0 / towards;
        } else {
            // Never crosses a boundary on this axis, so it never decides one.
            step[axis] = 0;
            next[axis] = f32::INFINITY;
            delta[axis] = f32::INFINITY;
        }
    }

    let mut face = None;
    let mut travelled = 0.0_f32;
    while travelled <= reach {
        if let Some(coord) = coord_of(cell)
            && cells.contains_key(&coord)
        {
            return Some(VoxelHit {
                cell: coord,
                // A ray starting inside a block has entered by no face; the
                // block it is in is still the answer, and the side it would
                // have come through is its top.
                face: face.unwrap_or(TileFace::Top),
            });
        }
        // Whichever boundary is nearest is the one crossed next.
        let axis = if next[0] < next[1] && next[0] < next[2] {
            0
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        if !next[axis].is_finite() {
            return None;
        }
        travelled = next[axis];
        cell[axis] += step[axis];
        next[axis] += delta[axis];
        face = Some(entering[axis][usize::from(step[axis] > 0)]);
    }
    None
}

/// Which cell a coordinate falls in, as the traversal counts them.
///
/// Named so the narrowing reads as the intent it is: a ray aimed far enough
/// outside any grid to overflow this is a ray that hits nothing, which is the
/// answer it gets.
#[allow(clippy::cast_possible_truncation)]
fn cell_of(at: f32) -> i64 {
    at.floor() as i64
}

/// Where one cell's edge sits along its own axis.
#[allow(clippy::cast_precision_loss)]
fn boundary(cell: i64) -> f32 {
    cell as f32
}

/// A traversal coordinate as a grid one, where it fits in a grid at all.
fn coord_of(cell: [i64; 3]) -> Option<GridCoord3> {
    Some(GridCoord3::new(
        i32::try_from(cell[0]).ok()?,
        i32::try_from(cell[1]).ok()?,
        i32::try_from(cell[2]).ok()?,
    ))
}
