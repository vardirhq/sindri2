//! Turning a cell into a place on a plane, and back.
//!
//! A projection and a Y axis together decide the mapping. Both
//! directions live on one type so an inverse cannot drift from what it
//! inverts.

use crate::{GridCoord, GridError, GridPoint, PlanePoint};

/// The layout of logical grid axes on a two-dimensional plane.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Projection {
    /// Columns run right and rows run down, one full cell per step.
    #[default]
    Orthogonal,
    /// Columns and rows run along the two diamond diagonals, one half-cell per
    /// step on each plane axis.
    Isometric,
}

/// Which way increasing logical Y travels on the projected plane.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaneYAxis {
    /// Positive grid Y is positive plane Y, as in pixel coordinates.
    #[default]
    Down,
    /// Positive grid Y is negative plane Y, as in Sindri's world XY plane.
    Up,
}

/// A validated mapping from logical grid coordinates to a 2D plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSpace {
    projection: Projection,
    cell_size: PlanePoint,
    origin: PlanePoint,
    y_axis: PlaneYAxis,
}

impl GridSpace {
    /// Creates a grid whose `(0, 0)` cell is centred on the plane origin.
    pub fn new(
        projection: Projection,
        cell_width: f64,
        cell_height: f64,
    ) -> Result<Self, GridError> {
        Self::with_origin(projection, cell_width, cell_height, PlanePoint::default())
    }

    /// Creates a grid whose `(0, 0)` cell is centred on `origin`.
    pub fn with_origin(
        projection: Projection,
        cell_width: f64,
        cell_height: f64,
        origin: PlanePoint,
    ) -> Result<Self, GridError> {
        Self::with_origin_and_y_axis(
            projection,
            cell_width,
            cell_height,
            origin,
            PlaneYAxis::Down,
        )
    }

    /// Creates a grid with an explicit plane-Y direction.
    pub fn with_origin_and_y_axis(
        projection: Projection,
        cell_width: f64,
        cell_height: f64,
        origin: PlanePoint,
        y_axis: PlaneYAxis,
    ) -> Result<Self, GridError> {
        if !cell_width.is_finite() || cell_width <= 0.0 {
            return Err(GridError::InvalidCellWidth(cell_width));
        }
        if !cell_height.is_finite() || cell_height <= 0.0 {
            return Err(GridError::InvalidCellHeight(cell_height));
        }
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(GridError::NonFinitePoint(origin));
        }
        Ok(Self {
            projection,
            cell_size: PlanePoint::new(cell_width, cell_height),
            origin,
            y_axis,
        })
    }

    #[must_use]
    pub const fn projection(self) -> Projection {
        self.projection
    }

    #[must_use]
    pub const fn cell_size(self) -> PlanePoint {
        self.cell_size
    }

    #[must_use]
    pub const fn origin(self) -> PlanePoint {
        self.origin
    }

    #[must_use]
    pub const fn y_axis(self) -> PlaneYAxis {
        self.y_axis
    }

    /// Projects a continuous grid position onto the configured plane.
    pub fn project(self, point: GridPoint) -> Result<PlanePoint, GridError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(GridError::NonFiniteGridPoint(point));
        }
        let (x, y) = match self.projection {
            Projection::Orthogonal => (point.x * self.cell_size.x, point.y * self.cell_size.y),
            Projection::Isometric => (
                (point.x - point.y) * self.cell_size.x * 0.5,
                (point.x + point.y) * self.cell_size.y * 0.5,
            ),
        };
        let y = match self.y_axis {
            PlaneYAxis::Down => y,
            PlaneYAxis::Up => -y,
        };
        let projected = PlanePoint::new(x + self.origin.x, y + self.origin.y);
        if projected.x.is_finite() && projected.y.is_finite() {
            Ok(projected)
        } else {
            Err(GridError::ProjectionOverflow)
        }
    }

    /// Reverses [`Self::project`] without choosing a cell.
    pub fn unproject(self, point: PlanePoint) -> Result<GridPoint, GridError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(GridError::NonFinitePoint(point));
        }
        let x = point.x - self.origin.x;
        let y = match self.y_axis {
            PlaneYAxis::Down => point.y - self.origin.y,
            PlaneYAxis::Up => self.origin.y - point.y,
        };
        let unprojected = match self.projection {
            Projection::Orthogonal => GridPoint::new(x / self.cell_size.x, y / self.cell_size.y),
            Projection::Isometric => {
                let horizontal = x / (self.cell_size.x * 0.5);
                let vertical = y / (self.cell_size.y * 0.5);
                GridPoint::new((horizontal + vertical) * 0.5, (vertical - horizontal) * 0.5)
            }
        };
        if unprojected.x.is_finite() && unprojected.y.is_finite() {
            Ok(unprojected)
        } else {
            Err(GridError::ProjectionOverflow)
        }
    }

    /// Returns the centre of a logical cell on the configured plane.
    pub fn grid_to_plane(self, coord: GridCoord) -> Result<PlanePoint, GridError> {
        self.project(coord.into())
    }

    /// Returns the cell containing a point on the configured plane.
    ///
    /// Cells are half-open on both axes: a boundary belongs to the cell in the
    /// positive direction. That keeps the rule identical on either side of
    /// zero and matches an array's left/top-inclusive cell bounds.
    pub fn plane_to_grid(self, point: PlanePoint) -> Result<GridCoord, GridError> {
        let point = self.unproject(point)?;
        let x = containing_cell_i32(point.x)?;
        let y = containing_cell_i32(point.y)?;
        Ok(GridCoord::new(x, y))
    }
}

fn containing_cell_i32(value: f64) -> Result<i32, GridError> {
    let cell = (value + 0.5).floor();
    if cell < f64::from(i32::MIN) || cell > f64::from(i32::MAX) {
        return Err(GridError::CoordinateOutsideIntegerRange(value));
    }
    // The bounds check above makes this narrowing exact.
    #[allow(clippy::cast_possible_truncation)]
    Ok(cell as i32)
}
