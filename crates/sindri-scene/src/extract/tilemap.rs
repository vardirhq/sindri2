//! Tilemaps, expanded into the sprites that draw them.

use glam::{Mat4, Vec3};
use sindri_core::World;
use sindri_render::{SpriteInstance, TransparentOrder, UvRect};

use crate::{TextureBindings, TileProjection, TilemapComponent};

use super::camera::ResolvedCameras;
use super::camera::view::camera_distance;
use super::sprite::{DrawSpace, SpriteBatches};
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
            let world_transform = transform_matrix(transform);
            let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;

            // A tilemap owns one place in world ordering. Its cells have their
            // own internal order below, but moving across the map must not make
            // one cell escape the map's authored world layer/depth and sort as
            // though it were a separate world entity.
            let map_distance = camera_distance(camera.view, world_transform.w_axis.truncate());

            // Zero is the legacy/default authored value. When the sheet knows
            // how far its art hangs below a logical tile, let the asset say it
            // once and scale that fact to this map's world-space tile height.
            // A positive component value remains an explicit override.
            let overhang = resolved_tile_overhang(&tilemap, textures);
            let draw_size = [tilemap.tile_size[0], tilemap.tile_size[1] + overhang];
            let draw_offset_y = -overhang / 2.0;

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
                //
                // A map whose tiles hang below their cells draws each on a
                // taller quad, dropped by half the overhang so the top of the
                // art stays on the cell. The cell itself never moves: it is what
                // the grid, picking and gameplay all measure in.
                let local = Mat4::from_translation(Vec3::new(
                    offset_x,
                    offset_y + draw_offset_y,
                    0.0,
                )) * Mat4::from_scale(Vec3::new(draw_size[0], draw_size[1], 1.0));
                let model = world_transform * local;

                // The map's layer and world distance place the tilemap as one
                // object in the scene. The submission index then gives cells a
                // deterministic internal back-to-front order. Isometric rows
                // are diagonals (column + row), not row-major array rows: using
                // row-major order lets the underhang of a far-south tile draw
                // over the top face of a much more northern tile.
                let order = TransparentOrder::new(
                    tilemap.layer,
                    map_distance,
                    tile_submission_index(&tilemap, column, row),
                )?;
                batches
                    .entry((DrawSpace::World, tilemap.layer, texture))
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

fn resolved_tile_overhang(tilemap: &TilemapComponent, textures: &TextureBindings) -> f32 {
    if tilemap.tile_overhang > 0.0 {
        return tilemap.tile_overhang;
    }
    textures
        .tile_overhang_ratio(&tilemap.texture)
        .map_or(0.0, |ratio| ratio * tilemap.tile_size[1])
}

/// Stable ordering inside one tilemap.
///
/// Orthogonal maps are authored top-to-bottom, left-to-right, so row-major is
/// already their visual order. Isometric maps advance visually downward on
/// diagonals: `(0, 1)` and `(1, 0)` share a row, while `(24, 0)` is far below
/// `(0, 1)` despite appearing earlier in a row-major array. The column is only
/// a deterministic tie-break inside one diagonal.
fn tile_submission_index(tilemap: &TilemapComponent, column: u32, row: u32) -> u32 {
    match tilemap.projection {
        TileProjection::Orthogonal => row.saturating_mul(tilemap.columns).saturating_add(column),
        TileProjection::Isometric => {
            let diagonal = column.saturating_add(row);
            let stride = tilemap.columns.max(tilemap.rows).max(1);
            diagonal.saturating_mul(stride).saturating_add(column)
        }
    }
}

#[cfg(test)]
mod tests {
    use sindri_core::SpriteSheetDocument;

    use super::*;

    fn map(projection: TileProjection, columns: u32, rows: u32) -> TilemapComponent {
        TilemapComponent {
            texture: "tiles.png".to_owned(),
            palette: vec!["tile".to_owned()],
            columns,
            rows,
            tile_size: [1.0, 1.0],
            tile_overhang: 0.0,
            projection,
            tiles: vec![],
            tint: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
        }
    }

    #[test]
    fn sheet_metadata_supplies_default_tile_overhang() {
        let mut bindings = TextureBindings::new();
        let mut sheet = SpriteSheetDocument::from_grid(1, 1);
        sheet.tile_overhang_ratio = Some(0.25);
        bindings
            .bind_sheet("tiles.png", &sheet)
            .expect("the one-cell sheet binds");
        let mut map = map(TileProjection::Isometric, 1, 1);
        map.tile_size = [2.0, 0.8];

        assert!((resolved_tile_overhang(&map, &bindings) - 0.2).abs() < 1.0e-6);

        map.tile_overhang = 0.3;
        assert!((resolved_tile_overhang(&map, &bindings) - 0.3).abs() < 1.0e-6);
    }

    #[test]
    fn orthogonal_tiles_keep_row_major_order() {
        let map = map(TileProjection::Orthogonal, 4, 4);
        assert!(tile_submission_index(&map, 3, 0) < tile_submission_index(&map, 0, 1));
    }

    #[test]
    fn isometric_tiles_order_by_visual_diagonal_not_array_row() {
        let map = map(TileProjection::Isometric, 25, 25);

        // Row-major gets this backwards: (24, 0) has array index 24 and
        // (0, 1) has 25, but the first cell is twenty-three diagonals further
        // south and therefore has to draw later.
        assert!(tile_submission_index(&map, 24, 0) > tile_submission_index(&map, 0, 1));
        assert!(tile_submission_index(&map, 1, 0) < tile_submission_index(&map, 0, 2));
    }

    #[test]
    fn overhang_does_not_change_internal_tile_order() {
        let mut map = map(TileProjection::Isometric, 8, 8);
        let before = tile_submission_index(&map, 3, 4);
        map.tile_overhang = 10.0;
        assert_eq!(tile_submission_index(&map, 3, 4), before);
    }
}
