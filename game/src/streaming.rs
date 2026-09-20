//! Camera-driven materialization of Causeway's generated terrain.

use sindri_core::World;
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
    focus: Option<TileChunkCoord>,
    chunks: TileChunkStore,
}

impl TerrainStream {
    /// Ensures terrain around the camera after gameplay has moved it.
    pub(crate) fn update(&mut self, world: &mut World) -> Result<bool, CausewayError> {
        let Some(camera_position) = world
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some("World Camera"))
            .and_then(|(_, data)| data.transform_3d.map(|transform| transform.position))
        else {
            return Ok(false);
        };
        let Some((floor_entity, grid, origin, volume)) = world
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
                    data.transform_3d.unwrap_or_default().position,
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
        #[allow(clippy::cast_possible_truncation)]
        let focus = TileChunkCoord::containing(
            ((camera_position[0] - origin[0]) / across).round() as i32,
            ((camera_position[2] - origin[2]) / into).round() as i32,
        );
        if self.focus == Some(focus) {
            return Ok(false);
        }

        // The component may have been edited since the last boundary crossing.
        // It wins before new generated chunks are added, so building is never
        // undone by streaming.
        self.chunks.replace_from_volume(&volume);
        let changed = load_around(&mut self.chunks, world_shape(), focus);
        self.focus = Some(focus);
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
    let mut changed = false;
    let chunk_limit = WORLD_EDGE / TILE_CHUNK_SIZE;
    for y in focus.y - LOAD_RADIUS..=focus.y + LOAD_RADIUS {
        for x in focus.x - LOAD_RADIUS..=focus.x + LOAD_RADIUS {
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
}
