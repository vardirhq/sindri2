//! Renderer-independent coordinates and projection math for grid-based games.
//!
//! This crate deliberately knows nothing about scenes, cameras, sprites, or
//! editors. [`GridSpace`] maps a logical grid onto a two-dimensional plane; a
//! consumer decides whether that plane is world XY, world XZ, or screen space.

use std::fmt;

/// An integer cell in logical grid space.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
}

impl GridCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Applies an offset, returning `None` at the edge of the integer domain.
    #[must_use]
    pub fn checked_offset(self, x: i32, y: i32) -> Option<Self> {
        Some(Self {
            x: self.x.checked_add(x)?,
            y: self.y.checked_add(y)?,
        })
    }

    /// The four edge-sharing neighbours, in north/east/south/west order.
    pub fn cardinal_neighbours(self) -> impl Iterator<Item = Self> {
        const OFFSETS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        OFFSETS
            .into_iter()
            .filter_map(move |(x, y)| self.checked_offset(x, y))
    }

    /// All eight surrounding neighbours, in stable row-major order.
    pub fn surrounding_neighbours(self) -> impl Iterator<Item = Self> {
        const OFFSETS: [(i32, i32); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        OFFSETS
            .into_iter()
            .filter_map(move |(x, y)| self.checked_offset(x, y))
    }
}

/// A continuous position expressed in grid axes.
///
/// Integer values name cell centres. Half-integers therefore lie on cell
/// boundaries, independently of how the grid is projected.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GridPoint {
    pub x: f64,
    pub y: f64,
}

impl GridPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<GridCoord> for GridPoint {
    fn from(coord: GridCoord) -> Self {
        Self::new(f64::from(coord.x), f64::from(coord.y))
    }
}

/// A point on the two-dimensional plane a grid is projected onto.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlanePoint {
    pub x: f64,
    pub y: f64,
}

impl PlanePoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

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

/// A finite rectangular region beginning at `(0, 0)`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GridBounds {
    width: i32,
    height: i32,
}

impl GridBounds {
    pub fn new(width: u32, height: u32) -> Result<Self, GridError> {
        let width = i32::try_from(width)
            .map_err(|_| GridError::BoundsOutsideIntegerRange { width, height })?;
        let height = i32::try_from(height).map_err(|_| GridError::BoundsOutsideIntegerRange {
            width: u32::try_from(width).expect("the width was checked above"),
            height,
        })?;
        Ok(Self { width, height })
    }

    #[must_use]
    pub fn width(self) -> u32 {
        u32::try_from(self.width).expect("grid bounds store non-negative widths")
    }

    #[must_use]
    pub fn height(self) -> u32 {
        u32::try_from(self.height).expect("grid bounds store non-negative heights")
    }

    #[must_use]
    pub fn contains(self, coord: GridCoord) -> bool {
        coord.x >= 0 && coord.y >= 0 && coord.x < self.width && coord.y < self.height
    }

    /// Coordinates in deterministic row-major order.
    pub fn iter(self) -> impl Iterator<Item = GridCoord> {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| GridCoord::new(x, y)))
    }

    pub fn cardinal_neighbours(self, coord: GridCoord) -> impl Iterator<Item = GridCoord> {
        coord
            .cardinal_neighbours()
            .filter(move |neighbour| self.contains(*neighbour))
    }

    pub fn surrounding_neighbours(self, coord: GridCoord) -> impl Iterator<Item = GridCoord> {
        coord
            .surrounding_neighbours()
            .filter(move |neighbour| self.contains(*neighbour))
    }
}

/// A validated mapping from logical grid coordinates to a 2D plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSpace {
    projection: Projection,
    cell_size: PlanePoint,
    origin: PlanePoint,
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

    /// Projects a continuous grid position onto the configured plane.
    pub fn project(self, point: GridPoint) -> Result<PlanePoint, GridError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(GridError::NonFiniteGridPoint(point));
        }
        let projected = match self.projection {
            Projection::Orthogonal => PlanePoint::new(
                point.x * self.cell_size.x + self.origin.x,
                point.y * self.cell_size.y + self.origin.y,
            ),
            Projection::Isometric => PlanePoint::new(
                (point.x - point.y) * self.cell_size.x * 0.5 + self.origin.x,
                (point.x + point.y) * self.cell_size.y * 0.5 + self.origin.y,
            ),
        };
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
        let y = point.y - self.origin.y;
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
    /// The boundary rule is deterministic: exact half-cell ties round away
    /// from zero, matching [`f64::round`].
    pub fn plane_to_grid(self, point: PlanePoint) -> Result<GridCoord, GridError> {
        let point = self.unproject(point)?;
        let x = rounded_i32(point.x)?;
        let y = rounded_i32(point.y)?;
        Ok(GridCoord::new(x, y))
    }
}

fn rounded_i32(value: f64) -> Result<i32, GridError> {
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(GridError::CoordinateOutsideIntegerRange(value));
    }
    // The bounds check above makes this narrowing exact.
    #[allow(clippy::cast_possible_truncation)]
    Ok(rounded as i32)
}

/// A projection input that cannot be represented safely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridError {
    InvalidCellWidth(f64),
    InvalidCellHeight(f64),
    NonFinitePoint(PlanePoint),
    NonFiniteGridPoint(GridPoint),
    CoordinateOutsideIntegerRange(f64),
    BoundsOutsideIntegerRange { width: u32, height: u32 },
    ProjectionOverflow,
}

impl fmt::Display for GridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCellWidth(width) => {
                write!(
                    formatter,
                    "grid cell width must be finite and positive, got {width}"
                )
            }
            Self::InvalidCellHeight(height) => {
                write!(
                    formatter,
                    "grid cell height must be finite and positive, got {height}"
                )
            }
            Self::NonFinitePoint(point) => write!(
                formatter,
                "plane point must be finite, got ({}, {})",
                point.x, point.y
            ),
            Self::NonFiniteGridPoint(point) => write!(
                formatter,
                "grid point must be finite, got ({}, {})",
                point.x, point.y
            ),
            Self::CoordinateOutsideIntegerRange(value) => {
                write!(
                    formatter,
                    "grid coordinate {value} is outside the i32 range"
                )
            }
            Self::BoundsOutsideIntegerRange { width, height } => write!(
                formatter,
                "grid bounds {width}x{height} exceed the signed coordinate range"
            ),
            Self::ProjectionOverflow => formatter.write_str("grid projection overflowed f64"),
        }
    }
}

impl std::error::Error for GridError {}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-10;

    fn assert_point(actual: PlanePoint, expected: PlanePoint) {
        assert!((actual.x - expected.x).abs() < EPSILON, "x: {actual:?}");
        assert!((actual.y - expected.y).abs() < EPSILON, "y: {actual:?}");
    }

    #[test]
    fn orthogonal_projection_uses_full_cell_steps() {
        let grid = GridSpace::new(Projection::Orthogonal, 64.0, 32.0).unwrap();
        assert_point(
            grid.grid_to_plane(GridCoord::new(3, -2)).unwrap(),
            PlanePoint::new(192.0, -64.0),
        );
    }

    #[test]
    fn isometric_projection_uses_half_cell_diagonals() {
        let grid = GridSpace::new(Projection::Isometric, 64.0, 32.0).unwrap();
        assert_point(
            grid.grid_to_plane(GridCoord::new(3, 2)).unwrap(),
            PlanePoint::new(32.0, 80.0),
        );
    }

    #[test]
    fn origin_moves_both_projection_directions_together() {
        let grid = GridSpace::with_origin(
            Projection::Isometric,
            64.0,
            32.0,
            PlanePoint::new(400.0, 200.0),
        )
        .unwrap();
        assert_point(
            grid.grid_to_plane(GridCoord::new(1, 0)).unwrap(),
            PlanePoint::new(432.0, 216.0),
        );
    }

    #[test]
    fn both_projections_round_trip_cells_across_negative_and_positive_space() {
        for projection in [Projection::Orthogonal, Projection::Isometric] {
            let grid =
                GridSpace::with_origin(projection, 63.5, 29.25, PlanePoint::new(173.0, -91.0))
                    .unwrap();
            for y in -128..=128 {
                for x in -128..=128 {
                    let coord = GridCoord::new(x, y);
                    let plane = grid.grid_to_plane(coord).unwrap();
                    assert_eq!(grid.plane_to_grid(plane).unwrap(), coord);
                }
            }
        }
    }

    #[test]
    fn continuous_projection_is_its_own_inverse() {
        let points = [
            GridPoint::new(-100.25, 37.75),
            GridPoint::new(-0.49, 0.49),
            GridPoint::new(0.0, 0.0),
            GridPoint::new(17.125, -9.875),
        ];
        for projection in [Projection::Orthogonal, Projection::Isometric] {
            let grid = GridSpace::new(projection, 57.0, 31.0).unwrap();
            for point in points {
                let actual = grid.unproject(grid.project(point).unwrap()).unwrap();
                assert!((actual.x - point.x).abs() < EPSILON);
                assert!((actual.y - point.y).abs() < EPSILON);
            }
        }
    }

    #[test]
    fn a_point_inside_each_cell_resolves_to_that_cell() {
        for projection in [Projection::Orthogonal, Projection::Isometric] {
            let grid = GridSpace::new(projection, 64.0, 32.0).unwrap();
            for y in -16..=16 {
                for x in -16..=16 {
                    let expected = GridCoord::new(x, y);
                    for offset in [
                        GridPoint::new(-0.49, -0.49),
                        GridPoint::new(0.49, -0.49),
                        GridPoint::new(-0.49, 0.49),
                        GridPoint::new(0.49, 0.49),
                    ] {
                        let sample =
                            GridPoint::new(f64::from(x) + offset.x, f64::from(y) + offset.y);
                        assert_eq!(
                            grid.plane_to_grid(grid.project(sample).unwrap()).unwrap(),
                            expected
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bounded_neighbours_never_leave_the_grid() {
        let bounds = GridBounds::new(3, 2).unwrap();
        assert_eq!(
            bounds
                .cardinal_neighbours(GridCoord::new(0, 0))
                .collect::<Vec<_>>(),
            vec![GridCoord::new(1, 0), GridCoord::new(0, 1)]
        );
        assert_eq!(
            bounds
                .surrounding_neighbours(GridCoord::new(2, 1))
                .collect::<Vec<_>>(),
            vec![
                GridCoord::new(1, 0),
                GridCoord::new(2, 0),
                GridCoord::new(1, 1)
            ]
        );
    }

    #[test]
    fn bounds_iterate_in_row_major_order() {
        assert_eq!(
            GridBounds::new(2, 2).unwrap().iter().collect::<Vec<_>>(),
            vec![
                GridCoord::new(0, 0),
                GridCoord::new(1, 0),
                GridCoord::new(0, 1),
                GridCoord::new(1, 1),
            ]
        );
    }

    #[test]
    fn coordinate_neighbours_do_not_overflow() {
        assert_eq!(GridCoord::new(i32::MAX, 0).cardinal_neighbours().count(), 3);
        assert_eq!(
            GridCoord::new(i32::MIN, i32::MIN)
                .surrounding_neighbours()
                .count(),
            3
        );
    }

    #[test]
    fn invalid_projection_inputs_are_rejected() {
        assert!(matches!(
            GridSpace::new(Projection::Orthogonal, 0.0, 1.0),
            Err(GridError::InvalidCellWidth(0.0))
        ));
        assert!(matches!(
            GridSpace::new(Projection::Orthogonal, 1.0, f64::NAN),
            Err(GridError::InvalidCellHeight(value)) if value.is_nan()
        ));
        let grid = GridSpace::new(Projection::Orthogonal, 1.0, 1.0).unwrap();
        assert!(matches!(
            grid.plane_to_grid(PlanePoint::new(f64::INFINITY, 0.0)),
            Err(GridError::NonFinitePoint(_))
        ));
        assert!(matches!(
            GridBounds::new(u32::MAX, 1),
            Err(GridError::BoundsOutsideIntegerRange { .. })
        ));
    }

    #[test]
    fn arbitrary_continuous_points_round_trip_on_the_plane() {
        let grid = GridSpace::with_origin(
            Projection::Isometric,
            70.0,
            34.0,
            PlanePoint::new(900.0, 450.0),
        )
        .unwrap();
        let expected = PlanePoint::new(947.25, 412.75);
        assert_point(
            grid.project(grid.unproject(expected).unwrap()).unwrap(),
            expected,
        );
    }
}
