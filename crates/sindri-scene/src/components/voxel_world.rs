//! Scene-authored configuration for an engine-owned voxel world.

use serde::{Deserialize, Serialize};
use sindri_core::SceneComponent;

use super::voxel_terrain::NaturalTerrainDocument;

/// How a scene deterministically supplies untouched voxel data.
///
/// Both built-in sources are portable: the editor constructs them without
/// loading game-specific Rust code. `layered_terrain` is a modest rolling
/// field of three layers; `natural_terrain` is a whole world of continents,
/// ranges, rivers, biomes, caves and trees. Games may still own richer
/// `VoxelSource` implementations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VoxelGeneratorDocument {
    LayeredTerrain {
        #[serde(default)]
        seed: u64,
        #[serde(default = "default_base_height")]
        base_height: i32,
        #[serde(default = "default_height_variation")]
        height_variation: u32,
        #[serde(default = "surface_voxel")]
        surface_voxel: u16,
        #[serde(default = "subsurface_voxel")]
        subsurface_voxel: u16,
        #[serde(default = "deep_voxel")]
        deep_voxel: u16,
        #[serde(default = "default_subsurface_depth")]
        subsurface_depth: u32,
    },
    NaturalTerrain(NaturalTerrainDocument),
}

impl VoxelGeneratorDocument {
    /// Every spelling of `kind`, in the order a picker offers them.
    pub const KINDS: [&'static str; 2] = ["layered_terrain", "natural_terrain"];
}

impl Default for VoxelGeneratorDocument {
    fn default() -> Self {
        Self::LayeredTerrain {
            seed: 0,
            base_height: default_base_height(),
            height_variation: default_height_variation(),
            surface_voxel: surface_voxel(),
            subsurface_voxel: subsurface_voxel(),
            deep_voxel: deep_voxel(),
            subsurface_depth: default_subsurface_depth(),
        }
    }
}

/// Renderer-facing appearance for one semantic voxel identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VoxelMaterialDocument {
    pub voxel: u16,
    pub top: String,
    pub side: String,
    pub bottom: String,
}

/// A persistent, section-streamed voxel world rendered through the block
/// mesher and GPU section cache.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VoxelWorldComponent {
    #[serde(default)]
    pub generator: VoxelGeneratorDocument,
    #[serde(default = "default_materials")]
    pub materials: Vec<VoxelMaterialDocument>,
    /// Section coordinate around which residency is maintained.
    #[serde(default)]
    pub focus: [i32; 3],
    #[serde(default = "default_render_radius")]
    pub render_radius: u32,
    #[serde(default = "default_vertical_radius")]
    pub vertical_radius: u32,
    #[serde(default)]
    pub layer: i32,
}

impl SceneComponent for VoxelWorldComponent {
    const TYPE_NAME: &'static str = "sindri.voxel_world";
}

const fn default_base_height() -> i32 {
    3
}

const fn default_height_variation() -> u32 {
    6
}

const fn default_subsurface_depth() -> u32 {
    3
}

const fn default_render_radius() -> u32 {
    2
}

const fn default_vertical_radius() -> u32 {
    1
}

const fn surface_voxel() -> u16 {
    1
}

const fn subsurface_voxel() -> u16 {
    2
}

const fn deep_voxel() -> u16 {
    3
}

fn default_materials() -> Vec<VoxelMaterialDocument> {
    (1..=3)
        .map(|voxel| VoxelMaterialDocument {
            voxel,
            top: "procedural:checkerboard".to_owned(),
            side: "procedural:checkerboard".to_owned(),
            bottom: "procedural:checkerboard".to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_world_has_bounded_visible_defaults() {
        let component: VoxelWorldComponent = serde_json::from_str("{}").unwrap();
        assert_eq!(component.focus, [0, 0, 0]);
        assert_eq!(component.render_radius, 2);
        assert_eq!(component.vertical_radius, 1);
        assert_eq!(component.generator, VoxelGeneratorDocument::default());
        assert_eq!(component.materials.len(), 3);
    }
}
