//! Engine-owned sparse voxel world foundations.
//!
//! Games decide what a coordinate contains. This crate owns how that answer is
//! addressed, stored in sections, revised, and eventually handed to meshers.

mod coord;
mod section;
mod source;

pub use coord::{LocalVoxelCoord, SectionCoord, VoxelCoord, SECTION_EDGE, SECTION_VOLUME};
pub use section::{VoxelId, VoxelSection};
pub use source::VoxelSource;
