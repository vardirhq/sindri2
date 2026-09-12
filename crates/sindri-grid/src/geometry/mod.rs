//! Where a cell is, where that puts it on a plane, and what is refused.

mod coord;
mod error;
mod space;
mod volume;

#[cfg(test)]
mod tests;

pub use coord::{GridBounds, GridCoord, GridCoord3, GridPoint, PlanePoint};
pub use error::GridError;
pub use space::{GridSpace, PlaneYAxis, Projection};
pub use volume::VolumeSpace;
