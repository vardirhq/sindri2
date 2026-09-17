//! Stackable tile-grid geometry and sparse logical cell storage.

use std::collections::BTreeMap;

use serde::Deserialize;
use sindri_core::SceneComponent;
use sindri_grid::{
    GridBounds, GridCoord, GridCoord3, GridError, GridPoint, GridSpace, PlanePoint, PlaneYAxis,
    Projection, VolumeSpace,
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
    /// How much Z one step of projected depth costs.
    ///
    /// Zero, the default, derives no depth and leaves every scene ordering
    /// exactly as it did. Above zero, a cell further into the scene sits at a
    /// Z nearer the camera, and the ordering `docs/2d-model.md` already
    /// describes — Z is a position, the camera sorts by it — starts answering
    /// for this grid without anything authoring a layer.
    ///
    /// Small. Under an orthographic camera Z changes nothing but the order, so
    /// the step only has to survive comparison; the whole map still has to fit
    /// inside the camera's near and far planes, and a big step is how a far
    /// corner gets clipped away.
    #[serde(default)]
    pub depth_step: f32,
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
        let grid =
            GridSpace::with_origin_and_y_axis(projection, width, height, origin, PlaneYAxis::Up)?;
        VolumeSpace::new(
            grid,
            PlanePoint::new(f64::from(self.level_step[0]), f64::from(self.level_step[1])),
        )
    }

    /// Back-to-front order for one cell, in this grid's projection.
    ///
    /// Two places need the same answer: the renderer, deciding what to draw
    /// over what, and the editor, deciding which exposed top face a click
    /// landed on. Splitting that between them is how the two would eventually
    /// disagree about which block is in front.
    ///
    /// Depth is the ground distance from the viewer, which is not the same
    /// question in both projections. An isometric view looks along the grid's
    /// diagonal, so a cell's depth is `x + y` and moving one column east is a
    /// step toward the viewer. An orthogonal view looks straight down the rows:
    /// depth is `y` alone, and a column further east is neither nearer nor
    /// further. Sorting an orthogonal volume by `x + y` puts blocks in front of
    /// their eastern neighbours for no reason at all.
    ///
    /// Level breaks the tie rather than deepening it, in both: raising a block
    /// moves it up the screen without moving it toward the viewer, so a stack
    /// draws bottom-up and still goes behind whatever stands in front of it.
    /// The cell coordinate finishes the key so the order is total, and a scene
    /// draws the same way twice.
    #[must_use]
    pub fn depth_key(&self, coord: GridCoord3) -> (i64, i32, i32, i32) {
        let depth = match self.projection {
            TileProjection::Orthogonal => i64::from(coord.y),
            TileProjection::Isometric => i64::from(coord.x) + i64::from(coord.y),
        };
        (depth, coord.z, coord.y, coord.x)
    }

    /// How far into the scene a continuous grid point is.
    ///
    /// The same quantity `depth_key` orders cells by, kept continuous so a
    /// walker crossing a boundary changes depth smoothly rather than snapping a
    /// whole cell when its anchor rounds.
    #[must_use]
    pub fn depth_at(&self, column: f64, row: f64) -> f64 {
        match self.projection {
            TileProjection::Orthogonal => row,
            TileProjection::Isometric => column + row,
        }
    }

    /// The Z a thing standing at that depth takes.
    ///
    /// Positive depth means nearer the viewer, and the camera sorts larger Z in
    /// front, so the two agree without a sign to remember.
    #[must_use]
    pub fn depth_z(&self, depth: f64) -> f32 {
        if !self.depth_step.is_finite() || self.depth_step <= 0.0 {
            return 0.0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let z = (depth * f64::from(self.depth_step)) as f32;
        z
    }

    /// Where a point on the grid stands when the ground under it is `height`
    /// cells up.
    ///
    /// Column and row are continuous, because a wall stands on the edge between
    /// two cells rather than in either of them, and rounding it to one decides
    /// by coin toss which of them it occludes. Height is a fraction for the
    /// same kind of reason: the top of a slab is half a cell up, and something
    /// standing on it stands there rather than at the level above or below.
    #[must_use]
    pub fn point_to_local_at_height(&self, column: f64, row: f64, height: f32) -> Option<[f32; 2]> {
        let point = self
            .grid_space()
            .ok()?
            .project(GridPoint::new(column, row))
            .ok()?;
        // The plane is computed in f64 and drawn in f32, as every other cell
        // projection here is; a grid big enough to lose a pixel to the cast is
        // one whose far corner left the camera long before.
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (point.x as f32, point.y as f32);
        Some([
            x + height * self.level_step[0],
            y + height * self.level_step[1],
        ])
    }

    /// The volume's plane mapping without its level step.
    ///
    /// Navigation and the `Grid.*` script calls reason about columns and rows
    /// and leave levels to the volume, so they want exactly what a flat map's
    /// `grid_space` gives them. Derived from `volume_space` rather than built
    /// beside it, so the two can never drift into two different planes.
    pub fn grid_space(&self) -> Result<GridSpace, GridError> {
        Ok(self.volume_space()?.grid())
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

/// A volume's occupied cells, in a shape that can answer for one of them.
///
/// The sparse `Vec` a volume is written as is the right thing to serialize and
/// the wrong thing to ask questions of: finding one cell in it is a scan, and
/// resolving a volume asks for six neighbours of every cell to decide which
/// faces are hidden. That is the whole volume walked six times per cell, every
/// frame -- for Gather's island, a few million coordinate comparisons a frame
/// spent deciding something that had not changed.
///
/// Validation already had to visit every cell and remember which coordinates it
/// had seen, so the index is what that pass was building and throwing away.
pub struct TileVolumeIndex<'a> {
    cells: BTreeMap<GridCoord3, &'a str>,
}

impl<'a> TileVolumeIndex<'a> {
    #[must_use]
    pub fn tile(&self, coord: GridCoord3) -> Option<&'a str> {
        self.cells.get(&coord).copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }
}

impl TileVolumeComponent {
    /// Checks the volume against its grid and indexes it in the same pass.
    ///
    /// The checks are exactly the ones `validate` makes, because they are the
    /// same pass: a volume that cannot be indexed is one that names no tile
    /// set, leaves a cell without a tile, puts a cell outside the grid, or
    /// occupies one coordinate twice.
    pub fn index(&self, grid: &TileGridComponent) -> Result<TileVolumeIndex<'_>, TileVolumeError> {
        if self.tileset.trim().is_empty() {
            return Err(TileVolumeError::MissingTileSet);
        }
        let mut cells = BTreeMap::new();
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
            if cells.insert(coord, cell.tile.as_str()).is_some() {
                return Err(TileVolumeError::DuplicateCell {
                    x: coord.x,
                    y: coord.y,
                    z: coord.z,
                });
            }
        }
        Ok(TileVolumeIndex { cells })
    }

    pub fn validate(&self, grid: &TileGridComponent) -> Result<(), TileVolumeError> {
        self.index(grid).map(|_| ())
    }

    /// The tile at one coordinate, by scanning.
    ///
    /// Fine for a single question. Anything asking repeatedly -- resolving a
    /// volume, deriving its surfaces -- wants `index` instead.
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
            depth_step: 0.0,
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
