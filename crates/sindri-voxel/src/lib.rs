//! Engine-owned sparse voxel world foundations.
//!
//! Games decide what a coordinate contains. This crate owns how that answer is
//! addressed, stored in sections, revised, and eventually handed to meshers,
//! and offers one portable answer of its own, [`NaturalTerrain`], for scenes
//! that want a world without game code behind it.

mod cache;
mod coord;
mod material;
mod mesh;
#[cfg(test)]
mod mesh_tests;
mod queue;
mod section;
mod source;
mod terrain;
mod world;

pub use cache::{
    MeshingProfile, SectionMeshCache, SectionMeshJob, SectionMeshKey, SectionMeshRevision,
};
pub use coord::{LocalVoxelCoord, SECTION_EDGE, SECTION_VOLUME, SectionCoord, VoxelCoord};
pub use material::{
    DefaultVoxelMaterials, FaceOcclusion, RenderClass, VoxelMaterial, VoxelMaterialSource,
};
pub use mesh::{
    BlockMesh, BlockMeshPart, BlockVertex, SectionBounds, VoxelFace, mesh_block_section,
    mesh_block_section_with_materials,
};
pub use queue::VoxelWorkQueue;
pub use section::{VoxelId, VoxelSection};
pub use source::VoxelSource;
pub use terrain::{NaturalTerrain, NaturalTerrainSettings, TerrainBiome, TerrainPalette};
pub use world::{ResidencyConfig, ResidencyDelta, VoxelWorld};
