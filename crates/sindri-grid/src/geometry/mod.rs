//! Where a cell is, where that puts it on a plane, and what is refused.

mod coord;
mod error;
mod space;

#[cfg(test)]
mod tests;

pub use coord::{GridBounds, GridCoord, GridPoint, PlanePoint};
pub use error::GridError;
pub use space::{GridSpace, PlaneYAxis, Projection};
