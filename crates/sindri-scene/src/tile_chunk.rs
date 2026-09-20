//! Chunk coordinates and sparse runtime storage for tile volumes.
//!
//! A scene still serializes one readable sparse list of cells. Runtime systems
//! need a different unit: the square of columns that is generated, rebuilt,
//! culled, loaded, or released together. Keeping that unit in the engine means
//! a game generator and the renderer cannot quietly choose different seams.

use std::collections::BTreeMap;

use crate::{TileCellDocument, TileVolumeComponent};

/// How many columns and rows one streamed tile chunk spans.
pub const TILE_CHUNK_SIZE: i32 = 16;

/// One square of columns in a chunked tile volume.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TileChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl TileChunkCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The chunk containing a cell, including cells west or north of zero.
    #[must_use]
    pub const fn containing(column: i32, row: i32) -> Self {
        Self::new(
            column.div_euclid(TILE_CHUNK_SIZE),
            row.div_euclid(TILE_CHUNK_SIZE),
        )
    }

    #[must_use]
    pub const fn min_column(self) -> i32 {
        self.x * TILE_CHUNK_SIZE
    }

    #[must_use]
    pub const fn min_row(self) -> i32 {
        self.y * TILE_CHUNK_SIZE
    }

    #[must_use]
    pub const fn contains(self, column: i32, row: i32) -> bool {
        Self::containing(column, row).x == self.x && Self::containing(column, row).y == self.y
    }
}

/// Loaded chunks of one tile volume.
///
/// This is deliberately runtime data rather than a second scene format. A
/// save can keep a generator seed plus edits while the live world materializes
/// the chunks its cameras and actors currently need.
#[derive(Clone, Debug, Default)]
pub struct TileChunkStore {
    chunks: BTreeMap<TileChunkCoord, Vec<TileCellDocument>>,
}

impl TileChunkStore {
    #[must_use]
    pub fn from_volume(volume: &TileVolumeComponent) -> Self {
        let mut store = Self::default();
        store.replace_from_volume(volume);
        store
    }

    /// Reconciles runtime chunks with the live component.
    ///
    /// A game calls this before materializing newly generated chunks so edits
    /// made through `Grid.set_block` remain authoritative.
    pub fn replace_from_volume(&mut self, volume: &TileVolumeComponent) {
        self.chunks.clear();
        for cell in &volume.cells {
            let [column, row, _] = cell.position;
            self.chunks
                .entry(TileChunkCoord::containing(column, row))
                .or_default()
                .push(cell.clone());
        }
    }

    #[must_use]
    pub fn contains(&self, chunk: TileChunkCoord) -> bool {
        self.chunks.contains_key(&chunk)
    }

    pub fn insert(
        &mut self,
        chunk: TileChunkCoord,
        cells: Vec<TileCellDocument>,
    ) -> Option<Vec<TileCellDocument>> {
        debug_assert!(cells.iter().all(|cell| {
            let [column, row, _] = cell.position;
            chunk.contains(column, row)
        }));
        self.chunks.insert(chunk, cells)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn chunks(&self) -> impl Iterator<Item = TileChunkCoord> + '_ {
        self.chunks.keys().copied()
    }

    /// Materializes the loaded set in the existing scene component format.
    #[must_use]
    pub fn materialize(&self, template: &TileVolumeComponent) -> TileVolumeComponent {
        let mut volume = template.clone();
        volume.cells = self
            .chunks
            .values()
            .flat_map(|cells| cells.iter().cloned())
            .collect();
        volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_edges_use_euclidean_coordinates() {
        assert_eq!(TileChunkCoord::containing(0, 0), TileChunkCoord::new(0, 0));
        assert_eq!(TileChunkCoord::containing(15, 15), TileChunkCoord::new(0, 0));
        assert_eq!(TileChunkCoord::containing(16, 16), TileChunkCoord::new(1, 1));
        assert_eq!(TileChunkCoord::containing(-1, -1), TileChunkCoord::new(-1, -1));
        assert_eq!(TileChunkCoord::containing(-16, -16), TileChunkCoord::new(-1, -1));
    }
}
