//! Projecting integer levels through the same plane geometry as flat grids.

use crate::{GridCoord3, GridError, GridSpace, PlanePoint};

/// A grid plane plus the plane-space basis vector for one logical level.
///
/// The level step is explicit because the same logical volume may be viewed
/// in world XY, pixels, or an editor preview whose positive visual Y differs.
/// It is geometry, not a measurement recovered from any tile image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolumeSpace {
    grid: GridSpace,
    level_step: PlanePoint,
}

impl VolumeSpace {
    pub fn new(grid: GridSpace, level_step: PlanePoint) -> Result<Self, GridError> {
        if !level_step.x.is_finite() || !level_step.y.is_finite() {
            return Err(GridError::NonFinitePoint(level_step));
        }
        Ok(Self { grid, level_step })
    }

    #[must_use]
    pub const fn grid(self) -> GridSpace {
        self.grid
    }

    #[must_use]
    pub const fn level_step(self) -> PlanePoint {
        self.level_step
    }

    /// Projects one 3D cell centre onto the configured 2D plane.
    pub fn grid_to_plane(self, coord: GridCoord3) -> Result<PlanePoint, GridError> {
        let base = self.grid.grid_to_plane(coord.column())?;
        let level = f64::from(coord.z);
        let point = PlanePoint::new(
            base.x + self.level_step.x * level,
            base.y + self.level_step.y * level,
        );
        if point.x.is_finite() && point.y.is_finite() {
            Ok(point)
        } else {
            Err(GridError::ProjectionOverflow)
        }
    }

    /// Resolves a point to X/Y on one known level.
    ///
    /// A 2D point cannot identify a 3D cell without a visible face or known
    /// slice. Render picking supplies the face; slice tools supply `level`.
    pub fn plane_to_grid_at_level(
        self,
        point: PlanePoint,
        level: i32,
    ) -> Result<GridCoord3, GridError> {
        let level_offset = f64::from(level);
        let base = PlanePoint::new(
            point.x - self.level_step.x * level_offset,
            point.y - self.level_step.y * level_offset,
        );
        let coord = self.grid.plane_to_grid(base)?;
        Ok(GridCoord3::new(coord.x, coord.y, level))
    }
}
