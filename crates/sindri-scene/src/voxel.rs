//! A tile volume as solid geometry rather than as a picture of one.
//!
//! The other path through a volume resolves its cells into flat quads already
//! arranged for one fixed isometric viewpoint. It draws well from that
//! viewpoint and from nowhere else, because the arrangement *is* the illusion:
//! turn the camera and the faces are still facing where the camera used to be.
//!
//! This is the same cells as a shape. A cell becomes a box, and each of its
//! six sides a quad facing outward. Nothing here knows where the camera is,
//! and that is the point: what covers what stops being an order somebody has
//! to compute and becomes a depth comparison the GPU already does.
//!
//! A cell's coordinates are a column, a row and a level; the world's axes are
//! X across, Y up and Z into the scene. So a level stacks along Y and a row
//! runs along Z, rather than the other way about. That is not arbitrary: the
//! engine's cameras are Y-up -- a camera looks down its own -Z with +Y up, and
//! the editor's orbit turns about Y -- so a volume that stacked along Z would
//! build sideways to every camera that looks at it.

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
pub(super) const fn basis_of(face: TileFace) -> (Vec3, Vec3, Vec3) {
    match face {
        TileFace::Top => (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        TileFace::Bottom => (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        TileFace::South => (Vec3::Z, Vec3::X, Vec3::Y),
        TileFace::North => (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
        TileFace::East => (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        TileFace::West => (Vec3::NEG_X, Vec3::Z, Vec3::Y),
    }
}

/// Where a cell's floor sits, in world units.
///
/// The one place the mapping lives: column across, level up, row into the
/// scene. Everything else asks here rather than doing it again.
pub(super) fn floor_of(cell: GridCoord3, cell_size: [f32; 3]) -> Vec3 {
    let [across, into, up] = cell_size;
    Vec3::new(
        axis(cell.x) * across,
        axis(cell.z) * up,
        axis(cell.y) * into,
    )
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
        let base = floor_of(cell, cell_size);
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
            // A quad's own width runs along whichever world axis `right` picks
            // out, and its height along `up`: across for a column, into the
            // scene for a row, and the tile's own fill for anything vertical.
            let extent = |axis: Vec3| sx * axis.x.abs() + tall * axis.y.abs() + sy * axis.z.abs();
            let (width, height) = (extent(right), extent(up));
            let centre = base
                + Vec3::new(0.0, tall * 0.5, 0.0)
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

mod picking;

pub use picking::{VoxelHit, face_quad, pick};

#[cfg(test)]
mod tests;
