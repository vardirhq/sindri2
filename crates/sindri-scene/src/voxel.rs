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
use sindri_core::{TileBox, TileFace, TileSetDocument};
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

/// How bright a corner is, by how many blocks crowd it.
///
/// The voxel lighting trick, and it is not lighting: no light is traced and no
/// light source exists. A corner is dark in proportion to how enclosed it is,
/// which is a property of the blocks alone, so it is as static as they are and
/// costs nothing once baked.
///
/// The two edge neighbours closing on a corner shut it completely -- the
/// diagonal behind them cannot make it darker, and asking would let a block
/// nobody can see change one that is visible.
const AO_LEVELS: [f32; 4] = [0.48, 0.66, 0.84, 1.0];

fn corner_light(side_a: bool, side_b: bool, diagonal: bool) -> f32 {
    if side_a && side_b {
        return AO_LEVELS[0];
    }
    let crowding = usize::from(side_a) + usize::from(side_b) + usize::from(diagonal);
    AO_LEVELS[3 - crowding]
}

/// A direction in the world, as the step it is between cells.
///
/// Column across, level up, row into the scene -- the same mapping `floor_of`
/// uses, read the other way about.
fn step_of(direction: Vec3) -> [i32; 3] {
    #[allow(clippy::cast_possible_truncation)]
    fn whole(value: f32) -> i32 {
        value.round() as i32
    }
    [whole(direction.x), whole(direction.z), whole(direction.y)]
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
    /// How much light reaches each corner, from the quad's own `[0, 0]` round
    /// to `[0, 1]`: the crease where blocks meet, which is what tells the eye
    /// these are solid things touching rather than lit shapes side by side.
    pub corners: [f32; 4],
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
    // Only a tile that occludes is here at all: a fence says it does not hide
    // what is behind it, and that one flag keeps it from culling its
    // neighbours' faces and from darkening their corners.
    let box_at = |cell: GridCoord3| -> Option<TileBox> {
        let definition = tile_set.tile(cells.get(&cell).copied()?)?;
        definition.occludes.then(|| definition.bounds())
    };

    let mut faces = Vec::new();
    for (cell, tile) in volume.occupied() {
        let definition = tile_set.tile(tile).ok_or_else(|| VoxelError::UnknownTile {
            cell,
            tile: tile.to_owned(),
        })?;
        let shape = definition.bounds();
        // The cell's own corner, and the box's corner within it. A cell is
        // `[across, into, up]` and a box is `[across, up, into]`, because one
        // counts a grid and the other describes a shape in the world.
        let base = floor_of(cell, cell_size);
        // A cell is centred on its column and row and stands on its level, so
        // a fraction across or into the cell is measured from the middle and a
        // fraction up is measured from the floor.
        let low = Vec3::new(
            (shape.min[0] - 0.5) * sx,
            shape.min[1] * sz,
            (shape.min[2] - 0.5) * sy,
        );
        let [wide, tall, deep] = shape.size();
        let span = Vec3::new(wide * sx, tall * sz, deep * sy);
        // The middle of the box, which is what the faces are placed around.
        let middle = base + low + span * 0.5;
        // Asked once a cell rather than once a face: whether this block is
        // under another is a fact about the block.
        let [up_x, up_y, up_z] = TileFace::Top.neighbour_offset();
        let covered_above =
            box_at(GridCoord3::new(cell.x + up_x, cell.y + up_y, cell.z + up_z)).is_some();

        for face in TileFace::ALL {
            let [dx, dy, dz] = face.neighbour_offset();
            let neighbour = GridCoord3::new(cell.x + dx, cell.y + dy, cell.z + dz);
            if covers(face, shape, box_at(neighbour)) {
                continue;
            }
            // Which of a tile's looks this cell wears. Without this every
            // block of one kind is the same block, and whatever pattern its
            // faces carry repeats on the grid -- a field of grass reads as
            // tiling rather than as grass.
            // A block with something standing on it wears its buried look,
            // where it has one. Grass is why: a course of it halfway up a
            // cliff still carrying a green fringe reads as a stack of lawns.
            let buried = definition.covered.as_ref().filter(|_| covered_above);
            let Some((_, visual)) = definition
                .faces_at(volume.variant_seed, [cell.x, cell.y, cell.z], tile)
                .resolved_in(face, buried)
            else {
                continue;
            };
            let (normal, right, up) = basis_of(face);
            // Every quad is a side of the tile's own box, so its width, its
            // height and how far it stands from the middle all read off the
            // same three numbers. A slab's top is lower than a block's and a
            // post's sides are close together for one reason rather than
            // three special cases.
            let reach =
                |axis: Vec3| span.x * axis.x.abs() + span.y * axis.y.abs() + span.z * axis.z.abs();
            let (width, height) = (reach(right), reach(up));
            let centre = middle + normal * reach(normal) * 0.5;
            // The four corners of this face, in the quad's own order. Each is
            // crowded by the two blocks along its edges and the one diagonally
            // between them -- all of them in front of the face, since a block
            // behind it is inside the one being drawn.
            let ahead = face.neighbour_offset();
            let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(
                |(along, up_by): (f32, f32)| {
                    let step_right = step_of(right * along);
                    let step_up = step_of(up * up_by);
                    let at = |offset: [i32; 3]| {
                        let neighbour = GridCoord3::new(
                            cell.x + ahead[0] + offset[0],
                            cell.y + ahead[1] + offset[1],
                            cell.z + ahead[2] + offset[2],
                        );
                        box_at(neighbour).is_some_and(TileBox::is_full)
                    };
                    let diagonal = [
                        step_right[0] + step_up[0],
                        step_right[1] + step_up[1],
                        step_right[2] + step_up[2],
                    ];
                    corner_light(at(step_right), at(step_up), at(diagonal))
                },
            );
            faces.push(VoxelFace {
                cell,
                face,
                corners,
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

/// Whether the tile in the next cell hides this face of this one.
///
/// Two things have to be true, and a height could only ever express the
/// second. The faces have to *meet*: this box has to reach the wall of its
/// cell that the neighbour is through, and the neighbour has to reach that
/// same wall from its side. A slab's top is in the middle of its cell, so
/// whatever sits in the cell above is not against it; a post set back from the
/// edge is not against the block beside it either.
///
/// And the neighbour has to cover the whole face, not merely touch it. A post
/// against a wall hides a sliver of it, which is to say it hides nothing that
/// can be dropped: drop the face and the wall has a hole where the post is
/// thinner than the cell.
fn covers(face: TileFace, shape: TileBox, neighbour: Option<TileBox>) -> bool {
    let Some(neighbour) = neighbour else {
        return false;
    };
    // Which axis the face looks along, and which end of it the face is.
    let (axis, mine, theirs) = match face {
        TileFace::East => (0, shape.max[0], neighbour.min[0]),
        TileFace::West => (0, shape.min[0], neighbour.max[0]),
        TileFace::Top => (1, shape.max[1], neighbour.min[1]),
        TileFace::Bottom => (1, shape.min[1], neighbour.max[1]),
        TileFace::South => (2, shape.max[2], neighbour.min[2]),
        TileFace::North => (2, shape.min[2], neighbour.max[2]),
    };
    // The wall between the two cells, in each one's own fractions: this box
    // reaches it at 1 or 0 depending on which way it is looking, and the
    // neighbour reaches it from the other side.
    let outward = matches!(face, TileFace::East | TileFace::Top | TileFace::South);
    let (wall, opposite) = if outward { (1.0, 0.0) } else { (0.0, 1.0) };
    TileBox::same(mine, wall) && TileBox::same(theirs, opposite) && neighbour.spans(shape, axis)
}

mod aim;
mod picking;

pub use aim::{REACH, VolumeAim, aim_at};
pub use picking::{VoxelHit, face_quad, model_of, pick, ray_at_viewport};

#[cfg(test)]
mod tests;
