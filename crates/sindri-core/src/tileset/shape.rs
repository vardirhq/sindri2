//! What part of its cell a tile fills.
//!
//! Split out of its parent because a tile set document has enough to say about
//! names, faces, looks and flags without also carrying the arithmetic of
//! boxes.

use serde::{Deserialize, Serialize};

/// The part of its cell a tile actually fills.
///
/// `height` says how far up a tile reaches and nothing about the other two
/// axes, so every tile written with one is a full-footprint slab: there is no
/// way to say post, fence, kerb, rail or step. This says it as a box instead,
/// in fractions of the cell, `[across, up, into]` -- the world's own axes, so
/// `max[1]` is the height a `height` would have given.
///
/// The whole cell is `min [0, 0, 0]`, `max [1, 1, 1]`. A fence post is thin
/// across and into and tall up; a kerb is low and full; a rail is a slice
/// partway up, which is the case `height` cannot express at all because a
/// height always starts at the floor.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct TileBox {
    /// The low corner, in fractions of the cell.
    pub min: [f32; 3],
    /// The high corner, in fractions of the cell.
    pub max: [f32; 3],
}

impl TileBox {
    /// The whole cell.
    pub const FULL: Self = Self {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 1.0, 1.0],
    };

    /// The box a bare `height` means: the full footprint, from the floor up.
    #[must_use]
    pub const fn from_height(height: f32) -> Self {
        Self {
            min: [0.0, 0.0, 0.0],
            max: [1.0, height, 1.0],
        }
    }

    /// How far up this tile reaches, which is what something stands on.
    #[must_use]
    pub const fn top(self) -> f32 {
        self.max[1]
    }

    /// How far across each axis this box reaches.
    #[must_use]
    pub fn size(self) -> [f32; 3] {
        [0, 1, 2].map(|axis| self.max[axis] - self.min[axis])
    }

    /// How close two fractions of a cell have to be to count as the same.
    ///
    /// These numbers are authored by hand and read out of JSON, so a box meant
    /// to reach the wall of its cell can arrive as 0.999999. Comparing them
    /// exactly would make whether a face is culled depend on how somebody
    /// typed a number, which is the sort of difference that shows up as one
    /// missing face in one frame.
    pub const TOUCHING: f32 = 1.0e-4;

    /// Whether two fractions of a cell are the same place.
    #[must_use]
    pub fn same(a: f32, b: f32) -> bool {
        (a - b).abs() < Self::TOUCHING
    }

    /// Whether this box is the whole cell.
    #[must_use]
    pub fn is_full(self) -> bool {
        (0..3).all(|axis| Self::same(self.min[axis], 0.0) && Self::same(self.max[axis], 1.0))
    }

    /// Whether this box covers all of `other` in the plane across `axis`.
    ///
    /// The two axes that are not `axis` are the plane a shared face lies in. A
    /// neighbour hides a face only if it covers the whole of it: a post
    /// against a block's side hides a sliver, which is to say it hides nothing
    /// the renderer can drop a face for.
    #[must_use]
    pub fn spans(self, other: Self, axis: usize) -> bool {
        (0..3).filter(|plane| *plane != axis).all(|plane| {
            self.min[plane] <= other.min[plane] + Self::TOUCHING
                && self.max[plane] + Self::TOUCHING >= other.max[plane]
        })
    }
}

impl Default for TileBox {
    fn default() -> Self {
        Self::FULL
    }
}
