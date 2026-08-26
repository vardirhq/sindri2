//! Tilemaps, expanded into the sprites that draw them.

use glam::{Mat4, Vec3};
use sindri_core::World;
use sindri_render::{SpriteInstance, TransparentOrder, UvRect};

use crate::{SpriteAnchor, TextureBindings, TilemapComponent};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::sprite::{SpriteBatches, screen_sprite_matrix};
use super::{SceneExtractError, SceneExtractor, transform_matrix};

impl SceneExtractor {
    /// Turns every tilemap's filled cells into sprite instances, into the same
    /// batches loose sprites use.
    ///
    /// The same batches deliberately: a tilemap is not a second kind of thing
    /// to draw, it is a compact way to author many of the first kind. Sharing
    /// the map means a tilemap and a loose sprite on one layer and one texture
    /// share a draw and sort against each other, so a prop can sit between two
    /// rows of floor without the tilemap being a plane that swallows it.
    pub(super) fn push_tilemaps(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        batches: &mut SpriteBatches,
    ) -> Result<(), SceneExtractError> {
        for (entity, tilemap) in self.components.query::<TilemapComponent>(world)? {
            tilemap.validate()?;
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let texture = textures.resolve(&tilemap.texture);
            // The palette is resolved once and the cells index the answers: a
            // map of 49 tiles names a handful of sprites, so looking each one
            // up per cell would be the same lookup forty-nine times.
            let palette: Vec<UvRect> = tilemap
                .palette
                .iter()
                .map(|sprite| {
                    textures
                        .sheet_sprite(&tilemap.texture, sprite)
                        .unwrap_or(UvRect::FULL)
                })
                .collect();
            for (column, row, index) in tilemap.filled() {
                let rect = palette.get(index as usize).copied().unwrap_or(UvRect::FULL);
                let [offset_x, offset_y] = tilemap.tile_to_local(column, row);
                // One tile is a sprite of the map's tile size, placed by the
                // map's own maths and then by the entity's transform, so moving
                // the entity moves the floor.
                let local = Mat4::from_translation(Vec3::new(offset_x, offset_y, 0.0))
                    * Mat4::from_scale(Vec3::new(tilemap.tile_size[0], tilemap.tile_size[1], 1.0));

                let (model, camera) = if tilemap.is_screen_space() {
                    let extent = cameras
                        .overlay_extent
                        .expect("every resolved view includes screen-space extent");
                    (
                        screen_sprite_matrix(transform, SpriteAnchor::default(), extent) * local,
                        cameras
                            .overlay
                            .expect("every resolved view includes screen-space projection"),
                    )
                } else {
                    (
                        transform_matrix(transform) * local,
                        cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?,
                    )
                };
                let position = model.w_axis.truncate().with_z(transform.position[2]);
                // Row and column break the tie rather than the entity index,
                // because every tile of one map shares an entity. Reading order
                // is the map's order, so the same map extracts the same way
                // every time.
                let order = TransparentOrder::new(
                    tilemap.layer,
                    camera_distance(camera.view, position),
                    row.saturating_mul(tilemap.columns).saturating_add(column),
                )?;
                batches
                    .entry((tilemap.space, tilemap.layer, texture))
                    .or_default()
                    .push((
                        order,
                        SpriteInstance::new(model, tilemap.tint).with_uv_rect(rect),
                    ));
            }
        }
        Ok(())
    }
}
