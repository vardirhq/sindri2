//! Camera-driven materialization of Causeway's generated terrain.

use sindri_core::{ComponentSchemaRegistry, Transform3D, World};
use sindri_scene::{
    TILE_CHUNK_SIZE, TileChunkCoord, TileChunkStore, TileGridComponent, TileVolumeComponent,
};

use crate::{
    error::CausewayError,
    worldgen::{WorldShape, generate_chunk},
};

pub(crate) const WORLD_EDGE: i32 = 65_536;
pub(crate) const WORLD_CENTRE: i32 = WORLD_EDGE / 2;
const LOAD_RADIUS: i32 = 3;
const VIEW_MARGIN: i32 = 1;
const FLOOR: &str = "sindri.tile_grid";
const VOLUME: &str = "sindri.tile_volume";
pub(crate) const TILE_SET: &str = "causeway.tileset.json";

#[must_use]
pub(crate) const fn world_shape() -> WorldShape {
    WorldShape {
        columns: WORLD_EDGE,
        rows: WORLD_EDGE,
        seed: 0x5E11_A1D2_0C4F_1907,
        sample_offset: [WORLD_CENTRE - 80; 2],
        climate_rows: 160,
    }
}

/// The chunks currently present in the live tile-volume component.
#[derive(Debug, Default)]
pub(crate) struct TerrainStream {
    window: Option<(TileChunkCoord, TileChunkCoord)>,
    chunks: TileChunkStore,
}

impl TerrainStream {
    /// Ensures terrain covers the ground footprint the camera can actually see.
    pub(crate) fn update(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        viewport: (f32, f32),
    ) -> Result<bool, CausewayError> {
        if viewport.0 <= 0.0 || viewport.1 <= 0.0 {
            return Ok(false);
        }
        let Some((floor_entity, grid, transform, volume)) = world
            .entities()
            .find(|(_, data)| data.components.contains_key(FLOOR))
            .map(|(entity, data)| {
                let grid =
                    serde_json::from_value::<TileGridComponent>(data.components[FLOOR].clone());
                let volume =
                    serde_json::from_value::<TileVolumeComponent>(data.components[VOLUME].clone());
                (
                    entity,
                    grid,
                    data.transform_3d.unwrap_or_default(),
                    volume,
                )
            })
        else {
            return Ok(false);
        };
        let grid = grid.map_err(|error| CausewayError::Generated(error.to_string()))?;
        let volume = volume.map_err(|error| CausewayError::Generated(error.to_string()))?;
        let Some([across, into, _]) = grid.solid_cell() else {
            return Ok(false);
        };
        let camera = sindri_scene::world_camera_of(world, components, viewport.0 / viewport.1)
            .map_err(|error| CausewayError::Generated(error.to_string()))?;
        let Some(camera) = camera else {
            return Ok(false);
        };
        let Some(window) =
            visible_chunk_window(transform, camera.view_projection, across, into)
        else {
            return Ok(false);
        };
        if self.window == Some(window) {
            return Ok(false);
        }

        // The component may have been edited since the last window change. It
        // wins before generated chunks are added, so building is never undone
        // by streaming.
        self.chunks.replace_from_volume(&volume);
        let changed = load_window(&mut self.chunks, world_shape(), window);
        self.window = Some(window);
        if !changed {
            return Ok(false);
        }
        let materialized = self.chunks.materialize(&volume);
        let payload = serde_json::to_value(materialized)
            .map_err(|error| CausewayError::Generated(error.to_string()))?;
        if let Some(data) = world.get_mut(floor_entity) {
            data.components.insert(VOLUME.to_owned(), payload);
        }
        Ok(true)
    }
}

/// The chunk rectangle under the viewport, plus one preload chunk on every side.
///
/// Rays are intersected with the volume's local ground plane. That is the point
/// the camera is looking at, unlike the elevated eye position, and using all
/// four corners means portrait and landscape windows both load what they frame.
fn visible_chunk_window(
    transform: Transform3D,
    view_projection: glam::Mat4,
    across: f32,
    into: f32,
) -> Option<(TileChunkCoord, TileChunkCoord)> {
    if across <= 0.0 || into <= 0.0 {
        return None;
    }
    let mut min = TileChunkCoord::new(i32::MAX, i32::MAX);
    let mut max = TileChunkCoord::new(i32::MIN, i32::MIN);
    for point in [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]] {
        let (origin, direction) =
            sindri_scene::voxel::ray_at_viewport(transform, view_projection, point)?;
        if direction.y.abs() <= f32::EPSILON {
            return None;
        }
        let distance = -origin.y / direction.y;
        if !distance.is_finite() || distance < 0.0 {
            return None;
        }
        let hit = origin + direction * distance;
        let column = cell_coordinate(hit.x / across);
        let row = cell_coordinate(hit.z / into);
        let chunk = TileChunkCoord::containing(column, row);
        min.x = min.x.min(chunk.x);
        min.y = min.y.min(chunk.y);
        max.x = max.x.max(chunk.x);
        max.y = max.y.max(chunk.y);
    }
    Some((
        TileChunkCoord::new(min.x - VIEW_MARGIN, min.y - VIEW_MARGIN),
        TileChunkCoord::new(max.x + VIEW_MARGIN, max.y + VIEW_MARGIN),
    ))
}

#[allow(clippy::cast_possible_truncation)]
fn cell_coordinate(value: f32) -> i32 {
    value.round() as i32
}

#[must_use]
pub(crate) fn initial_volume(focus_cell: [i32; 2]) -> TileVolumeComponent {
    let mut chunks = TileChunkStore::default();
    load_around(
        &mut chunks,
        world_shape(),
        TileChunkCoord::containing(focus_cell[0], focus_cell[1]),
    );
    chunks.materialize(&TileVolumeComponent {
        tileset: TILE_SET.to_owned(),
        variant_seed: world_shape().seed,
        ..TileVolumeComponent::default()
    })
}

fn load_around(chunks: &mut TileChunkStore, shape: WorldShape, focus: TileChunkCoord) -> bool {
    load_window(
        chunks,
        shape,
        (
            TileChunkCoord::new(focus.x - LOAD_RADIUS, focus.y - LOAD_RADIUS),
            TileChunkCoord::new(focus.x + LOAD_RADIUS, focus.y + LOAD_RADIUS),
        ),
    )
}

fn load_window(
    chunks: &mut TileChunkStore,
    shape: WorldShape,
    window: (TileChunkCoord, TileChunkCoord),
) -> bool {
    let mut changed = false;
    let chunk_limit = WORLD_EDGE / TILE_CHUNK_SIZE;
    for y in window.0.y..=window.1.y {
        for x in window.0.x..=window.1.x {
            if x < 0 || y < 0 || x >= chunk_limit || y >= chunk_limit {
                continue;
            }
            let chunk = TileChunkCoord::new(x, y);
            if chunks.contains(chunk) {
                continue;
            }
            chunks.insert(chunk, generate_chunk(shape, chunk));
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_initial_window_is_bounded_and_biomed() {
        let volume = initial_volume([WORLD_CENTRE, WORLD_CENTRE]);
        assert_eq!(
            TileChunkStore::from_volume(&volume).len(),
            usize::try_from((LOAD_RADIUS * 2 + 1).pow(2)).unwrap()
        );
        assert!(volume.cells.iter().any(|cell| cell.tile == "water"));
        assert!(volume.cells.iter().any(|cell| cell.tile != "water"));
    }

    #[test]
    fn cell_coordinate_uses_cell_centres() {
        assert_eq!(cell_coordinate(12.49), 12);
        assert_eq!(cell_coordinate(12.51), 13);
        assert_eq!(cell_coordinate(-0.49), 0);
    }
}
