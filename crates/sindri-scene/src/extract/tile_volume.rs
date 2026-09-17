//! Stackable tile volumes, resolved into their visible baked faces.

use glam::{Mat4, Vec3};
use sindri_core::{SpriteRef, TileDefinition, TileFace, TileSetDocument, World};
use sindri_grid::GridCoord3;
use sindri_render::{SpriteInstance, TransparentOrder};

use crate::{
    TextureBindings, TileGridComponent, TileSetBindings, TileVolumeComponent, TileVolumeIndex,
};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::sprite::{DrawSpace, SpriteBatches, SpriteDraw};
use super::{SceneExtractError, SceneExtractor, transform_matrix};

impl SceneExtractor {
    pub(super) fn push_tile_volumes(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        tile_sets: Option<&TileSetBindings>,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        for (entity, volume) in self.components.query::<TileVolumeComponent>(world)? {
            // A freshly added volume is intentionally empty and has no tile-set
            // reference yet. There is nothing to render, so requiring a grid or
            // asset before the author has had a chance to choose either would
            // turn Add Component into an immediate scene error.
            if volume.cells.is_empty() && volume.tileset.is_empty() {
                continue;
            }
            let grid = self
                .components
                .get::<TileGridComponent>(world, entity)?
                .ok_or(SceneExtractError::MissingTileGrid)?;
            grid.validate()?;
            let occupied = volume.index(&grid)?;
            let tile_set = tile_sets
                .and_then(|bindings| bindings.get(&volume.tileset))
                .ok_or_else(|| SceneExtractError::UnboundTileSet(volume.tileset.clone()))?;

            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let mut cells = volume.cells.iter().collect::<Vec<_>>();
            cells.sort_by_key(|cell| grid.depth_key(cell.coord()));

            for (cell_index, cell) in cells.into_iter().enumerate() {
                let coord = cell.coord();
                let definition =
                    tile_set
                        .tile(&cell.tile)
                        .ok_or_else(|| SceneExtractError::UnknownTile {
                            tile_set: volume.tileset.clone(),
                            tile: cell.tile.clone(),
                        })?;
                let [cell_x, cell_y] = grid
                    .cell_to_local(coord)
                    .expect("a validated grid projects finite integer cells");
                // Depth is a property of the *cell*, taken from where its
                // column meets the ground, and every face of it shares that one
                // value. Two things follow, and both were wrong while each face
                // measured its own drawn position.
                //
                // Raising a block moves it up the screen, which is not moving
                // it toward the viewer: a stack has to keep the depth of the
                // column it stands in, or a tower walks in front of everything
                // south of it as it grows.
                //
                // And a block's own faces must not sort against each other.
                // Their offsets differ by a fraction of a cell, which was
                // enough to interleave a top with the side of the block beside
                // it. Which face of a cell is drawn first is decided by the
                // face order, not by arithmetic on where its art happens to
                // sit.
                let [ground_x, ground_y] = grid
                    .cell_to_local(GridCoord3::new(coord.x, coord.y, 0))
                    .expect("a validated grid projects finite integer cells");
                let ground = transform_matrix(transform)
                    * Mat4::from_translation(Vec3::new(ground_x, ground_y, 0.0));
                let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
                // The cell's own Z, by the same rule anything standing on this
                // grid takes: depth is a consequence of position, so a block
                // and a prop are finally measured on one axis instead of two.
                // Zero `depth_step` leaves every cell at the volume's own Z,
                // which is what a backdrop wants and what every scene written
                // before this did.
                // Half a step back, because a cell *is* the ground and anything
                // placed on it rests on top: they share a column, so without
                // the bias they tie and submission order decides whether a
                // shrine stands on its flagstone or under it. This is what the
                // hand-written even-and-odd layers encoded before depth was
                // derived.
                let cell_z = transform.position[2]
                    + grid.depth_z(grid.depth_at(f64::from(coord.x), f64::from(coord.y)) - 0.5);
                let depth = camera_distance(camera.view, ground.w_axis.truncate().with_z(cell_z));
                let visible = grid.projection.visible_faces();
                for (face_index, (face, visual)) in definition.faces.iter().enumerate() {
                    // A face the projection turns away from is not culled by a
                    // neighbour; there is simply no view of it to draw. An
                    // isometric side in an orthogonal volume would otherwise
                    // paint itself flat across the block.
                    if !visible.contains(&face) {
                        continue;
                    }
                    if face_is_occluded(
                        &occupied,
                        &volume.tileset,
                        tile_set,
                        coord,
                        face,
                        definition,
                    )? {
                        continue;
                    }
                    let reference = SpriteRef::parse(&visual.sprite)?;
                    let (texture, rect) = textures.resolve_sprite(&reference);
                    let local =
                        Mat4::from_translation(Vec3::new(
                            cell_x + visual.offset[0],
                            cell_y + visual.offset[1],
                            0.0,
                        )) * Mat4::from_scale(Vec3::new(visual.size[0], visual.size[1], 1.0));
                    let model = transform_matrix(transform) * local;
                    // Cells were sorted back to front before this loop, so the
                    // submission index carries that order and the face index
                    // orders the faces inside one cell. It is what decides
                    // between draws the camera puts at the same depth -- the
                    // levels of one column, and anything a projection lays out
                    // along the view.
                    let stable = cell_index.saturating_mul(6).saturating_add(face_index);
                    let order = TransparentOrder::new(
                        volume.layer,
                        depth,
                        u32::try_from(stable).unwrap_or(u32::MAX),
                    )?;
                    batches.push(SpriteDraw {
                        space: DrawSpace::World,
                        texture,
                        order,
                        sprite: SpriteInstance::new(model, [1.0; 4]).with_uv_rect(rect),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Whether a neighbouring cell hides this face completely.
///
/// "Completely" is the word doing the work once tiles have heights. A face is
/// only dropped when nothing of it could be seen: a neighbour shorter than the
/// face it abuts leaves the rest of that face exposed, which is exactly what
/// makes a slab beside a block read as a slab rather than as a block.
fn face_is_occluded(
    occupied: &TileVolumeIndex<'_>,
    tileset: &str,
    tile_set: &TileSetDocument,
    coord: GridCoord3,
    face: TileFace,
    definition: &TileDefinition,
) -> Result<bool, SceneExtractError> {
    let [x, y, z] = face.neighbour_offset();
    let Some(neighbour) = coord.checked_offset(x, y, z) else {
        return Ok(false);
    };
    let Some(tile) = occupied.tile(neighbour) else {
        return Ok(false);
    };
    let neighbour = tile_set
        .tile(tile)
        .ok_or_else(|| SceneExtractError::UnknownTile {
            tile_set: tileset.to_owned(),
            tile: tile.to_owned(),
        })?;
    Ok(match face {
        // The cell above starts at this cell's ceiling, so it can only cover a
        // top face that reaches the ceiling. A slab's top sits below it, with
        // the gap an isometric camera looks into.
        TileFace::Top => neighbour.occludes && definition.fills_cell(),
        // Symmetrically, the cell below only reaches this floor when it is
        // full: a slab underneath leaves this cell's underside exposed.
        TileFace::Bottom => neighbour.occludes && neighbour.fills_cell(),
        TileFace::North | TileFace::West | TileFace::East | TileFace::South => {
            neighbour.hides_side_of(definition.height)
        }
    })
}
