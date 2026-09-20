//! Engine-owned sparse voxel world foundations.
//!
//! Games decide what a coordinate contains. This crate owns how that answer is
//! addressed, stored in sections, revised, and eventually handed to meshers.

mod coord;
mod section;
mod source;
mod world;

pub use coord::{LocalVoxelCoord, SECTION_EDGE, SECTION_VOLUME, SectionCoord, VoxelCoord};
pub use section::{VoxelId, VoxelSection};
pub use source::VoxelSource;
pub use world::{ResidencyConfig, ResidencyDelta, VoxelWorld};
