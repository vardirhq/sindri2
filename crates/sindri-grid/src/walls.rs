use std::{collections::BTreeSet, fmt};

use crate::{GridBounds, GridCoord};

/// A normalized, undirected wall between two edge-sharing cells.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GridWallEdge {
    first: GridCoord,
    second: GridCoord,
}

impl GridWallEdge {
    pub fn between(first: GridCoord, second: GridCoord) -> Result<Self, GridWallError> {
        let x_distance = i64::from(first.x).abs_diff(i64::from(second.x));
        let y_distance = i64::from(first.y).abs_diff(i64::from(second.y));
        if x_distance + y_distance != 1 {
            return Err(GridWallError::CellsDoNotShareEdge { first, second });
        }
        let (first, second) = if first <= second {
            (first, second)
        } else {
            (second, first)
        };
        Ok(Self { first, second })
    }

    #[must_use]
    pub const fn cells(self) -> (GridCoord, GridCoord) {
        (self.first, self.second)
    }

    #[must_use]
    pub const fn first(self) -> GridCoord {
        self.first
    }

    #[must_use]
    pub const fn second(self) -> GridCoord {
        self.second
    }
}

/// A bounded set of symmetric walls between cardinal neighbours.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridWalls {
    bounds: GridBounds,
    edges: BTreeSet<GridWallEdge>,
}

impl GridWalls {
    #[must_use]
    pub fn new(bounds: GridBounds) -> Self {
        Self {
            bounds,
            edges: BTreeSet::new(),
        }
    }

    #[must_use]
    pub const fn bounds(&self) -> GridBounds {
        self.bounds
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = GridWallEdge> + '_ {
        self.edges.iter().copied()
    }

    /// Adds a wall, returning whether the set changed.
    pub fn block(
        &mut self,
        first: GridCoord,
        second: GridCoord,
    ) -> Result<bool, GridWallError> {
        let edge = self.checked_edge(first, second)?;
        Ok(self.edges.insert(edge))
    }

    /// Removes a wall, returning whether the set changed.
    pub fn unblock(
        &mut self,
        first: GridCoord,
        second: GridCoord,
    ) -> Result<bool, GridWallError> {
        let edge = self.checked_edge(first, second)?;
        Ok(self.edges.remove(&edge))
    }

    pub fn is_blocked(
        &self,
        first: GridCoord,
        second: GridCoord,
    ) -> Result<bool, GridWallError> {
        let edge = self.checked_edge(first, second)?;
        Ok(self.edges.contains(&edge))
    }

    pub fn clear(&mut self) {
        self.edges.clear();
    }

    fn checked_edge(
        &self,
        first: GridCoord,
        second: GridCoord,
    ) -> Result<GridWallEdge, GridWallError> {
        if !self.bounds.contains(first) {
            return Err(GridWallError::CellOutsideBounds { cell: first });
        }
        if !self.bounds.contains(second) {
            return Err(GridWallError::CellOutsideBounds { cell: second });
        }
        GridWallEdge::between(first, second)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridWallError {
    CellOutsideBounds {
        cell: GridCoord,
    },
    CellsDoNotShareEdge {
        first: GridCoord,
        second: GridCoord,
    },
}

impl fmt::Display for GridWallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CellOutsideBounds { cell } => write!(
                formatter,
                "wall cell ({}, {}) is outside grid bounds",
                cell.x, cell.y
            ),
            Self::CellsDoNotShareEdge { first, second } => write!(
                formatter,
                "wall cells ({}, {}) and ({}, {}) do not share a cardinal edge",
                first.x, first.y, second.x, second.y
            ),
        }
    }
}

impl std::error::Error for GridWallError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_is_undirected_and_normalized() {
        let left = GridCoord::new(1, 2);
        let right = GridCoord::new(2, 2);
        assert_eq!(
            GridWallEdge::between(left, right).unwrap(),
            GridWallEdge::between(right, left).unwrap()
        );
        assert_eq!(
            GridWallEdge::between(left, right).unwrap().cells(),
            (left, right)
        );
    }

    #[test]
    fn only_cardinal_neighbours_form_an_edge() {
        let origin = GridCoord::new(0, 0);
        for other in [
            origin,
            GridCoord::new(1, 1),
            GridCoord::new(2, 0),
            GridCoord::new(i32::MIN, i32::MAX),
        ] {
            assert_eq!(
                GridWallEdge::between(origin, other),
                Err(GridWallError::CellsDoNotShareEdge {
                    first: origin,
                    second: other
                })
            );
        }
    }

    #[test]
    fn blocking_and_unblocking_are_symmetric_and_idempotent() {
        let mut walls = GridWalls::new(GridBounds::new(3, 2).unwrap());
        let first = GridCoord::new(0, 0);
        let second = GridCoord::new(1, 0);

        assert!(walls.block(first, second).unwrap());
        assert!(!walls.block(second, first).unwrap());
        assert!(walls.is_blocked(first, second).unwrap());
        assert!(walls.is_blocked(second, first).unwrap());
        assert!(walls.unblock(second, first).unwrap());
        assert!(!walls.unblock(first, second).unwrap());
        assert!(walls.is_empty());
    }

    #[test]
    fn a_bounded_wall_set_rejects_outside_cells() {
        let mut walls = GridWalls::new(GridBounds::new(2, 2).unwrap());
        assert_eq!(
            walls.block(GridCoord::new(1, 0), GridCoord::new(2, 0)),
            Err(GridWallError::CellOutsideBounds {
                cell: GridCoord::new(2, 0)
            })
        );
        assert!(walls.is_empty());
    }

    #[test]
    fn walls_iterate_in_normalized_coordinate_order() {
        let mut walls = GridWalls::new(GridBounds::new(3, 3).unwrap());
        walls
            .block(GridCoord::new(2, 1), GridCoord::new(2, 2))
            .unwrap();
        walls
            .block(GridCoord::new(0, 0), GridCoord::new(1, 0))
            .unwrap();

        assert_eq!(
            walls.iter().collect::<Vec<_>>(),
            vec![
                GridWallEdge::between(GridCoord::new(0, 0), GridCoord::new(1, 0)).unwrap(),
                GridWallEdge::between(GridCoord::new(2, 1), GridCoord::new(2, 2)).unwrap(),
            ]
        );
        walls.clear();
        assert!(walls.is_empty());
    }
}
