//! Where something is on a grid: a cell, a point, and the bounds around
//! them.

use crate::GridError;

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
