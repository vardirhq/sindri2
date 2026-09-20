//! Camera-driven materialization of Causeway's generated terrain.

use std::collections::BTreeMap;

use sindri_core::{ComponentSchemaRegistry, Transform3D, World};
use sindri_scene::TileCellDocument;
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
const MESH: &str = "sindri.mesh";
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
    /// Chunks whose live cells differ from deterministic generation.
    ///
    /// Residency stays bounded to the camera while player-built paths survive
    /// leaving and returning to a chunk.
    edits: BTreeMap<TileChunkCoord, Vec<TileCellDocument>>,
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
                (entity, grid, data.transform_3d.unwrap_or_default(), volume)
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
        let Some(window) = visible_chunk_window(transform, camera.view_projection, across, into)
        else {
            return Ok(false);
        };
        if self.window == Some(window) {
            return Ok(false);
        }

        // Compare the live component with the last materialized residency
        // before replacing anything. Differences are player edits, not terrain
        // the stream is allowed to forget when a chunk leaves the viewport.
        if self.window.is_none() {
            self.chunks.replace_from_volume(&volume);
        } else {
            remember_edits(&mut self.edits, &self.chunks, &volume);
        }

        // Residency is the current visible window, not every chunk the camera
        // has ever visited. The old append-only store made a long pan steadily
        // more expensive and forced the renderer to rebake an ever-growing
        // volume on a phone's main thread.
        let mut resident = TileChunkStore::default();
        load_window(&mut resident, world_shape(), window, &self.edits);
        self.chunks = resident;
        self.window = Some(window);
        let materialized = self.chunks.materialize(&volume);
        let payload = serde_json::to_value(materialized)
            .map_err(|error| CausewayError::Generated(error.to_string()))?;
        if let Some(data) = world.get_mut(floor_entity) {
            data.components.insert(VOLUME.to_owned(), payload);
            data.components.insert(
                MESH.to_owned(),
                surface_mesh_payload(world_shape(), centre_chunk(window)),
            );
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
        let ground = origin + direction * distance;

        // A mountain is met somewhere between the near plane and the ground
        // intersection. Loading only the latter fixed the old eye-centred bug
        // but dropped the camera-side half of that segment, which is exactly
        // the missing foreground visible on tall terrain.
        for sample in [origin, ground] {
            let column = cell_coordinate(sample.x / across);
            let row = cell_coordinate(sample.z / into);
            let chunk = TileChunkCoord::containing(column, row);
            min.x = min.x.min(chunk.x);
            min.y = min.y.min(chunk.y);
            max.x = max.x.max(chunk.x);
            max.y = max.y.max(chunk.y);
        }
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
        &BTreeMap::new(),
    )
}

fn load_window(
    chunks: &mut TileChunkStore,
    shape: WorldShape,
    window: (TileChunkCoord, TileChunkCoord),
    edits: &BTreeMap<TileChunkCoord, Vec<TileCellDocument>>,
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
            let cells = edits
                .get(&chunk)
                .cloned()
                .unwrap_or_else(|| generate_chunk(shape, chunk));
            chunks.insert(chunk, cells);
            changed = true;
        }
    }
    changed
}

fn centre_chunk(window: (TileChunkCoord, TileChunkCoord)) -> TileChunkCoord {
    TileChunkCoord::new(
        (window.0.x + window.1.x) / 2,
        (window.0.y + window.1.y) / 2,
    )
}

/// Builds one deliberately small meshy patch over the authoritative voxel
/// terrain. Causeway keeps collision, picking and construction on the voxels;
/// this patch exists to prove that their generated surface can drive a second
/// visual representation before that idea is promoted into the engine's voxel
/// subsystem.
#[allow(clippy::cast_precision_loss)]
fn surface_mesh_payload(shape: WorldShape, chunk: TileChunkCoord) -> serde_json::Value {
    let min_x = chunk.min_column();
    let min_y = chunk.min_row();
    let mut vertices = Vec::with_capacity(usize::try_from(TILE_CHUNK_SIZE.pow(2) * 4).unwrap());
    let mut uvs = Vec::with_capacity(vertices.capacity());
    let mut indices = Vec::with_capacity(usize::try_from(TILE_CHUNK_SIZE.pow(2) * 6).unwrap());

    for row in 0..TILE_CHUNK_SIZE {
        for column in 0..TILE_CHUNK_SIZE {
            let x = min_x + column;
            let y = min_y + row;
            let corners = [
                smooth_corner_height(shape, x, y),
                smooth_corner_height(shape, x + 1, y),
                smooth_corner_height(shape, x, y + 1),
                smooth_corner_height(shape, x + 1, y + 1),
            ];
            let left = x as f32 - 0.5;
            let near = y as f32 - 0.5;
            vertices.extend([
                [left, corners[0], near],
                [left + 1.0, corners[1], near],
                [left, corners[2], near + 1.0],
                [left + 1.0, corners[3], near + 1.0],
            ]);
            uvs.extend([[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]]);
            let base = u16::try_from(vertices.len() - 4)
                .expect("one Causeway surface patch stays below u16 vertex capacity");
            // Winding faces upward in Sindri's Y-up world.
            indices.extend([base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }

    serde_json::json!({
        "primitive": "surface",
        "texture": "textures/blocks-top.png#ground-0",
        "layer": 1,
        "surface": {
            "vertices": vertices,
            "uvs": uvs,
            "indices": indices
        }
    })
}

#[allow(clippy::cast_precision_loss)]
fn smooth_corner_height(shape: WorldShape, corner_x: i32, corner_y: i32) -> f32 {
    let mut total = 0.0;
    let mut samples = 0.0;
    for dy in -1..=0 {
        for dx in -1..=0 {
            let x = (corner_x + dx).clamp(0, shape.columns - 1);
            let y = (corner_y + dy).clamp(0, shape.rows - 1);
            total += shape.column(x, y).ground.max(crate::worldgen::SEA) as f32 + 0.515;
            samples += 1.0;
        }
    }
    total / samples
}

fn remember_edits(
    edits: &mut BTreeMap<TileChunkCoord, Vec<TileCellDocument>>,
    previous: &TileChunkStore,
    live: &TileVolumeComponent,
) {
    let live = TileChunkStore::from_volume(live);
    for chunk in live.chunks() {
        let Some(cells) = live.get(chunk) else {
            continue;
        };
        if previous.get(chunk) != Some(cells) {
            edits.insert(chunk, cells.to_vec());
        }
    }
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
    fn meshy_patch_is_one_chunk_of_smoothed_quads() {
        let payload = surface_mesh_payload(
            world_shape(),
            TileChunkCoord::containing(WORLD_CENTRE, WORLD_CENTRE),
        );
        let surface = &payload["surface"];
        assert_eq!(surface["vertices"].as_array().unwrap().len(), 16 * 16 * 4);
        assert_eq!(surface["indices"].as_array().unwrap().len(), 16 * 16 * 6);
        assert_eq!(payload["primitive"], "surface");
        assert_eq!(payload["texture"], "textures/blocks-top.png#ground-0");
    }

    #[test]
    fn cell_coordinate_uses_cell_centres() {
        assert_eq!(cell_coordinate(12.49), 12);
        assert_eq!(cell_coordinate(12.51), 13);
        assert_eq!(cell_coordinate(-0.49), 0);
    }
}
