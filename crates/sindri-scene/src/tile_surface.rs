//! Where a walker stands on a stacked volume.
//!
//! A volume is three-dimensional and grid navigation is not. The bridge is a
//! *surface*: for each column of cells, the height of the ground something
//! walking across it would stand on. Everything navigation needs from a volume
//! reduces to comparing two of those heights.
//!
//! Height is a float rather than a level, because a tile fills part of its cell
//! and the top of a slab is genuinely half a level lower than the top of a
//! block. That is the difference between a step a walker can take and one it
//! cannot, so rounding it away would quietly undo partial heights.
//!
//! A column with nothing solid in it has no surface at all. That is not the
//! same as a surface at level zero: it is a hole, and walking into it is what
//! navigation refuses.

use std::collections::BTreeMap;

use sindri_core::TileSetDocument;
use sindri_grid::{GridCoord, GridCoord3};
use thiserror::Error;

use crate::TileVolumeComponent;

/// The standing height of every column in one volume.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileSurfaces {
    heights: BTreeMap<(i32, i32), f32>,
    tops: BTreeMap<(i32, i32), i32>,
}

impl TileSurfaces {
    /// Reduces a volume's cells to one height per column.
    ///
    /// The surface of a column is the top of its highest solid cell. A tile
    /// that is not solid is not a floor — water and decoration are the reason
    /// the flag exists — so it neither raises the surface nor creates one, and
    /// a column holding nothing else is a hole.
    ///
    /// Cells above the surface are not consulted. An overhang is therefore a
    /// roof over nothing rather than a second walkable level, which is the
    /// limitation to lift when caves and bridges become navigable.
    pub fn derive(
        volume: &TileVolumeComponent,
        tile_set: &TileSetDocument,
    ) -> Result<Self, TileSurfaceError> {
        let mut heights: BTreeMap<(i32, i32), f32> = BTreeMap::new();
        let mut tops: BTreeMap<(i32, i32), i32> = BTreeMap::new();
        for (coord, tile) in volume.occupied() {
            let definition = tile_set
                .tile(tile)
                .ok_or_else(|| TileSurfaceError::UnknownTile {
                    tile: tile.to_owned(),
                })?;
            if !definition.solid {
                continue;
            }
            let column = (coord.x, coord.y);
            if tops.get(&column).is_some_and(|top| *top >= coord.z) {
                continue;
            }
            tops.insert(column, coord.z);
            // A level is an i32 and a height is an f32, so a volume stacked
            // past about eight million levels would start rounding. Nothing
            // authors that, and the value is a height to compare rather than a
            // coordinate to store, so the cast is the honest one here.
            #[allow(clippy::cast_precision_loss)]
            let top = coord.z as f32 + definition.height;
            heights.insert(column, top);
        }
        Ok(Self { heights, tops })
    }

    /// The highest solid cell of a column, which is the one you stand on.
    ///
    /// The height says how far up the ground is; this says which cell made it,
    /// which is what an editor needs to draw a mark on that ground.
    #[must_use]
    pub fn top(&self, coord: GridCoord) -> Option<GridCoord3> {
        self.tops
            .get(&(coord.x, coord.y))
            .map(|z| GridCoord3::new(coord.x, coord.y, *z))
    }

    /// How high the ground stands in this column, or `None` where it is a hole.
    #[must_use]
    pub fn height(&self, coord: GridCoord) -> Option<f32> {
        self.heights.get(&(coord.x, coord.y)).copied()
    }

    /// Every column that has ground in it, in stable coordinate order.
    ///
    /// What a sweep walks: one probe per place something can stand.
    pub fn columns(&self) -> impl Iterator<Item = (GridCoord, f32)> + '_ {
        self.heights
            .iter()
            .map(|((x, y), height)| (GridCoord::new(*x, *y), *height))
    }

    /// Whether something may walk between two columns given a step limit.
    ///
    /// The limit is symmetric, because a wall is. Grid walls block an edge
    /// rather than a direction, so a rule that let a walker drop further than
    /// it can climb has nowhere to be recorded; taking the stricter of the two
    /// is the honest reading of a symmetric edge. Asymmetric climb and drop
    /// need directional edges in `sindri-grid`, which do not exist yet, and
    /// `docs/parity.md` carries the row.
    #[must_use]
    pub fn step_is_walkable(&self, from: GridCoord, to: GridCoord, max_step: f32) -> bool {
        match (self.height(from), self.height(to)) {
            (Some(from), Some(to)) => (from - to).abs() <= max_step,
            _ => false,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TileSurfaceError {
    #[error("a volume cell names tile `{tile}`, which its tile set does not define")]
    UnknownTile { tile: String },
}
