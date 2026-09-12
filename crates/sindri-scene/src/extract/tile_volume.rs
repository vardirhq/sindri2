//! Stackable tile volumes, resolved into their visible baked faces.

use glam::{Mat4, Vec3};
use sindri_core::{SpriteRef, TileFace, TileSetDocument, World};
use sindri_grid::GridCoord3;
use sindri_render::{SpriteInstance, TransparentOrder};

use crate::{TextureBindings, TileGridComponent, TileSetBindings, TileVolumeComponent};

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
            let grid = self
                .components
                .get::<TileGridComponent>(world, entity)?
                .ok_or(SceneExtractError::MissingTileGrid)?;
            grid.validate()?;
            volume.validate(&grid)?;
            let tile_set = tile_sets
                .and_then(|bindings| bindings.get(&volume.tileset))
                .ok_or_else(|| SceneExtractError::UnboundTileSet(volume.tileset.clone()))?;
            tile_set.validate()?;

            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let mut cells = volume.cells.iter().collect::<Vec<_>>();
            cells.sort_by_key(|cell| {
                let coord = cell.coord();
                (coord.x.saturating_add(coord.y), coord.z, coord.y, coord.x)
            });

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
                for (face_index, (face, visual)) in definition.faces.iter().enumerate() {
                    if face_is_occluded(volume, tile_set, coord, face)? {
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
                    let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
                    let position = model.w_axis.truncate().with_z(transform.position[2]);
                    let stable = cell_index.saturating_mul(6).saturating_add(face_index);
                    let order = TransparentOrder::new(
                        volume.layer,
                        camera_distance(camera.view, position),
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

fn face_is_occluded(
    volume: &TileVolumeComponent,
    tile_set: &TileSetDocument,
    coord: GridCoord3,
    face: TileFace,
) -> Result<bool, SceneExtractError> {
    let [x, y, z] = face.neighbour_offset();
    let Some(neighbour) = coord.checked_offset(x, y, z) else {
        return Ok(false);
    };
    let Some(tile) = volume.tile(neighbour) else {
        return Ok(false);
    };
    let definition = tile_set
        .tile(tile)
        .ok_or_else(|| SceneExtractError::UnknownTile {
            tile_set: volume.tileset.clone(),
            tile: tile.to_owned(),
        })?;
    Ok(definition.occludes)
}
