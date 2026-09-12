//! Stackable tile-grid geometry and sparse logical cell storage.

use std::collections::BTreeSet;

use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_grid::{
    GridBounds, GridCoord, GridCoord3, GridError, GridSpace, PlanePoint, PlaneYAxis, Projection,
    VolumeSpace,
};
use thiserror::Error;

use super::TileProjection;

/// Projection and bounds shared by every tile volume on an entity.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TileGridComponent {
    pub columns: u32,
    pub rows: u32,
    #[serde(default = "unit_cell")]
    pub cell_size: [f32; 2],
    #[serde(default = "default_level_step")]
    pub level_step: [f32; 2],
    #[serde(default)]
    pub projection: TileProjection,
}

const fn unit_cell() -> [f32; 2] {
    [1.0, 1.0]
}

const fn default_level_step() -> [f32; 2] {
    [0.0, 0.5]
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum TileGridError {
    #[error(transparent)]
    Grid(#[from] GridError),
}

impl TileGridComponent {
    pub fn validate(&self) -> Result<(), TileGridError> {
        self.bounds()?;
        self.volume_space()?;
        Ok(())
    }

    pub fn bounds(&self) -> Result<GridBounds, GridError> {
        GridBounds::new(self.columns, self.rows)
    }

    pub fn volume_space(&self) -> Result<VolumeSpace, GridError> {
        let width = f64::from(self.cell_size[0]);
        let height = f64::from(self.cell_size[1]);
        let (projection, origin) = match self.projection {
            TileProjection::Orthogonal => (
                Projection::Orthogonal,
                PlanePoint::new(width * 0.5, -height * 0.5),
            ),
            TileProjection::Isometric => (Projection::Isometric, PlanePoint::default()),
        };
        let grid = GridSpace::with_origin_and_y_axis(
            projection,
            width,
            height,
            origin,
            PlaneYAxis::Up,
        )?;
        VolumeSpace::new(
            grid,
            PlanePoint::new(
                f64::from(self.level_step[0]),
                f64::from(self.level_step[1]),
            ),
        )
    }

    #[must_use]
    pub fn contains(&self, coord: GridCoord3) -> bool {
        self.bounds()
            .is_ok_and(|bounds| bounds.contains(GridCoord::new(coord.x, coord.y)))
    }

    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn cell_to_local(&self, coord: GridCoord3) -> Option<[f32; 2]> {
        let point = self.volume_space().ok()?.grid_to_plane(coord).ok()?;
        Some([point.x as f32, point.y as f32])
    }

    #[must_use]
    pub fn local_to_cell_at_level(&self, point: [f32; 2], level: i32) -> Option<GridCoord3> {
        let coord = self
            .volume_space()
            .ok()?
            .plane_to_grid_at_level(
                PlanePoint::new(f64::from(point[0]), f64::from(point[1])),
                level,
            )
            .ok()?;
        self.contains(coord).then_some(coord)
    }
}

impl SceneComponent for TileGridComponent {
    const TYPE_NAME: &'static str = "sindri.tile_grid";
}

/// One occupied cell in the serialized sparse volume.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TileCellDocument {
    pub position: [i32; 3],
    pub tile: String,
    #[serde(default)]
    pub visual_override: Option<String>,
}

impl TileCellDocument {
    #[must_use]
    pub const fn coord(&self) -> GridCoord3 {
        GridCoord3::new(self.position[0], self.position[1], self.position[2])
    }
}

/// Sparse tile IDs for a stackable volume.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TileVolumeComponent {
    pub tileset: String,
    #[serde(default)]
    pub cells: Vec<TileCellDocument>,
    #[serde(default)]
    pub layer: i32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TileVolumeError {
    #[error("a tile volume must name a tile set asset")]
    MissingTileSet,
    #[error("tile at ({x}, {y}, {z}) has an empty ID")]
    MissingTileId { x: i32, y: i32, z: i32 },
    #[error("tile at ({x}, {y}, {z}) is outside the grid's X/Y bounds")]
    CellOutsideGrid { x: i32, y: i32, z: i32 },
    #[error("more than one tile occupies ({x}, {y}, {z})")]
    DuplicateCell { x: i32, y: i32, z: i32 },
}

impl TileVolumeComponent {
    pub fn validate(&self, grid: &TileGridComponent) -> Result<(), TileVolumeError> {
        if self.tileset.trim().is_empty() {
            return Err(TileVolumeError::MissingTileSet);
        }
        let mut occupied = BTreeSet::new();
        for cell in &self.cells {
            let coord = cell.coord();
            if cell.tile.trim().is_empty() {
                return Err(TileVolumeError::MissingTileId {
                    x: coord.x,
                    y: coord.y,
                    z: coord.z,
                });
            }
            if !grid.contains(coord) {
                return Err(TileVolumeError::CellOutsideGrid {
                    x: coord.x,
                    y: coord.y,
                    z: coord.z,
                });
            }
            if !occupied.insert(coord) {
                return Err(TileVolumeError::DuplicateCell {
                    x: coord.x,
                    y: coord.y,
                    z: coord.z,
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn tile(&self, coord: GridCoord3) -> Option<&str> {
        self.cells
            .iter()
            .find(|cell| cell.coord() == coord)
            .map(|cell| cell.tile.as_str())
    }

    pub fn occupied(&self) -> impl Iterator<Item = (GridCoord3, &str)> {
        self.cells
            .iter()
            .map(|cell| (cell.coord(), cell.tile.as_str()))
    }
}

impl SceneComponent for TileVolumeComponent {
    const TYPE_NAME: &'static str = "sindri.tile_volume";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> TileGridComponent {
        TileGridComponent {
            columns: 4,
            rows: 3,
            cell_size: [1.0, 0.5],
            level_step: [0.0, 0.5],
            projection: TileProjection::Isometric,
        }
    }

    fn volume(cells: Vec<TileCellDocument>) -> TileVolumeComponent {
        TileVolumeComponent {
            tileset: "world.tileset.json".to_owned(),
            cells,
            layer: 0,
        }
    }

    fn cell(position: [i32; 3], tile: &str) -> TileCellDocument {
        TileCellDocument {
            position,
            tile: tile.to_owned(),
            visual_override: None,
        }
    }

    #[test]
    fn stacked_cells_are_distinct_occupants() {
        let volume = volume(vec![cell([1, 1, 0], "earth"), cell([1, 1, 1], "grass")]);
        assert_eq!(volume.validate(&grid()), Ok(()));
        assert_eq!(volume.tile(GridCoord3::new(1, 1, 0)), Some("earth"));
        assert_eq!(volume.tile(GridCoord3::new(1, 1, 1)), Some("grass"));
    }

    #[test]
    fn duplicate_and_out_of_bounds_cells_are_rejected() {
        let duplicate = volume(vec![cell([1, 1, 0], "earth"), cell([1, 1, 0], "grass")]);
        assert!(matches!(
            duplicate.validate(&grid()),
            Err(TileVolumeError::DuplicateCell { .. })
        ));
        let outside = volume(vec![cell([4, 0, 0], "earth")]);
        assert!(matches!(
            outside.validate(&grid()),
            Err(TileVolumeError::CellOutsideGrid { .. })
        ));
    }

    #[test]
    fn a_known_level_round_trips_through_the_editor_projection() {
        let grid = grid();
        let coord = GridCoord3::new(2, 1, 3);
        let local = grid.cell_to_local(coord).unwrap();
        assert_eq!(grid.local_to_cell_at_level(local, 3), Some(coord));
    }
}
