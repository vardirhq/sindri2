//! What a step costs, and which steps are allowed at all.

use super::path::GridPathError;

/// The neighbours A* may traverse.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GridMovement {
    /// Move only across cell edges.
    #[default]
    Cardinal,
    /// Move across edges and corners.
    EightWay {
        /// Whether a diagonal may pass between two blocked cardinal neighbours.
        allow_corner_cutting: bool,
    },
}

/// Integer movement costs used by the pathfinder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridPathCosts {
    pub(super) cardinal: u32,
    pub(super) diagonal: u32,
}

impl GridPathCosts {
    pub fn new(cardinal: u32, diagonal: u32) -> Result<Self, GridPathError> {
        if cardinal == 0 {
            return Err(GridPathError::ZeroCardinalCost);
        }
        if diagonal == 0 {
            return Err(GridPathError::ZeroDiagonalCost);
        }
        Ok(Self { cardinal, diagonal })
    }

    #[must_use]
    pub const fn cardinal(self) -> u32 {
        self.cardinal
    }

    #[must_use]
    pub const fn diagonal(self) -> u32 {
        self.diagonal
    }
}

impl Default for GridPathCosts {
    fn default() -> Self {
        Self {
            cardinal: 10,
            diagonal: 14,
        }
    }
}
