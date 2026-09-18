//! Which block a ray meets, and which of its sides.
//!
//! The other half of this module turns cells into the faces that draw
//! them. This turns a ray back into a cell and a face, which is the same
//! relationship read the other way about and is what a click is.

use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};
use sindri_core::{TileFace, Transform3D};
use sindri_grid::GridCoord3;

use crate::components::TileVolumeComponent;

use super::{basis_of, floor_of};

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

/// The four corners of one side of one cell, in the volume's own space.
///
/// What a hover outline is drawn from. It is the whole cell's side, not the
/// part a half-height tile fills, for the same reason picking treats cells as
/// whole: the outline says which cell and which side the click means, and a
/// slab's is the slab's cell.
#[must_use]
pub fn face_quad(cell: GridCoord3, face: TileFace, cell_size: [f32; 3]) -> [Vec3; 4] {
    let [across, into, up] = cell_size;
    let (normal, right, top) = basis_of(face);
    let extent = |axis: Vec3| across * axis.x.abs() + up * axis.y.abs() + into * axis.z.abs();
    let centre = floor_of(cell, cell_size)
        + Vec3::new(0.0, up * 0.5, 0.0)
        + normal
            * match face {
                TileFace::Top | TileFace::Bottom => up * 0.5,
                TileFace::North | TileFace::South => into * 0.5,
                TileFace::East | TileFace::West => across * 0.5,
            };
    let half_right = right * extent(right) * 0.5;
    let half_top = top * extent(top) * 0.5;
    [
        centre - half_right - half_top,
        centre + half_right - half_top,
        centre + half_right + half_top,
        centre - half_right + half_top,
    ]
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
    let [sx, sy, sz] = cell_size;
    if sx <= 0.0 || sy <= 0.0 || sz <= 0.0 {
        return None;
    }
    // Into cell space -- column, row, level -- where every cell is the unit
    // cube from its own coordinate, and the traversal below need know nothing
    // about which world axis is up.
    let start = Vec3::new(origin.x / sx + 0.5, origin.z / sy + 0.5, origin.y / sz);
    let step_by = Vec3::new(direction.x / sx, direction.z / sy, direction.y / sz);
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

/// The matrix placing a volume's own space in the world.
#[must_use]
pub fn model_of(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.position),
    )
}

/// The ray a pointer sends into a volume's own space.
///
/// `point` is the fraction across and down the picture, so a caller hands over
/// where the pointer is rather than how big its window happens to be. The ray
/// comes back in the volume's space rather than the world's, because that is
/// the space `pick` walks: undoing the model here means the walk never has to
/// know the volume moved.
///
/// Beside `pick` rather than beside the editor that first needed it. A click
/// and a script's aim are the same question, and two unprojections that had to
/// agree would eventually not.
#[must_use]
pub fn ray_at_viewport(
    transform: Transform3D,
    view_projection: Mat4,
    point: [f32; 2],
) -> Option<(Vec3, Vec3)> {
    if !(0.0..=1.0).contains(&point[0]) || !(0.0..=1.0).contains(&point[1]) {
        return None;
    }
    let inverse_view = view_projection.inverse();
    let inverse_model = model_of(transform).inverse();
    if !finite(inverse_view) || !finite(inverse_model) {
        return None;
    }
    let x = point[0] * 2.0 - 1.0;
    let y = 1.0 - point[1] * 2.0;
    let near = inverse_model.transform_point3(inverse_view.project_point3(Vec3::new(x, y, 0.0)));
    let far = inverse_model.transform_point3(inverse_view.project_point3(Vec3::new(x, y, 1.0)));
    let direction = far - near;
    (direction.length_squared() > f32::EPSILON).then_some((near, direction.normalize()))
}

fn finite(matrix: Mat4) -> bool {
    matrix.to_cols_array().into_iter().all(f32::is_finite)
}
