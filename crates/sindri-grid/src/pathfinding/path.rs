//! The answer a search gives, and the ways it can fail to give one.

use std::fmt;

use crate::{GridBounds, GridCoord};

/// A least-cost route including its start and goal cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridPath {
    pub(super) nodes: Vec<GridCoord>,
    pub(super) cost: u64,
}

impl GridPath {
    #[must_use]
    pub fn nodes(&self) -> &[GridCoord] {
        &self.nodes
    }

    #[must_use]
    pub fn into_nodes(self) -> Vec<GridCoord> {
        self.nodes
    }

    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridPathError {
    ZeroCardinalCost,
    ZeroDiagonalCost,
    StartOutsideBounds { start: GridCoord },
    GoalOutsideBounds { goal: GridCoord },
    WallBoundsMismatch { path: GridBounds, walls: GridBounds },
    CostOverflow,
    SearchOverflow,
}

impl fmt::Display for GridPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCardinalCost => formatter.write_str("cardinal path cost must be positive"),
            Self::ZeroDiagonalCost => formatter.write_str("diagonal path cost must be positive"),
            Self::StartOutsideBounds { start } => write!(
                formatter,
                "path start ({}, {}) is outside grid bounds",
                start.x, start.y
            ),
            Self::GoalOutsideBounds { goal } => write!(
                formatter,
                "path goal ({}, {}) is outside grid bounds",
                goal.x, goal.y
            ),
            Self::WallBoundsMismatch { path, walls } => write!(
                formatter,
                "path bounds {}x{} do not match wall bounds {}x{}",
                path.width(),
                path.height(),
                walls.width(),
                walls.height()
            ),
            Self::CostOverflow => formatter.write_str("path cost exceeded the u64 range"),
            Self::SearchOverflow => formatter.write_str("path search exceeded the u64 node range"),
        }
    }
}

impl std::error::Error for GridPathError {}
