//! The authored form of the engine's natural terrain generator.

use serde::{Deserialize, Serialize};

use super::voxel_world::VoxelBlock;

/// A world of continents, ranges, rivers and biomes, decided by one seed.
///
/// Each field has a default, so switching a world to this generator gives a
/// usable one that still only needs the three materials every new world
/// starts with. The optional voxels are `null` until chosen, and a world
/// without them is simply without water, beaches, bare rock, snow, ice or
/// trees.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NaturalTerrainDocument {
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub sea_level: i32,
    /// How far above the sea the highest ranges reach, in voxels.
    #[serde(default = "default_relief")]
    pub relief: u32,
    /// The width of the largest landforms, in voxels.
    #[serde(default = "default_feature_size")]
    pub feature_size: u32,
    /// Heights above the sea where the ground turns to bare rock, then snow.
    #[serde(default = "default_tree_line")]
    pub tree_line: i32,
    #[serde(default = "default_snow_line")]
    pub snow_line: i32,
    #[serde(default = "yes")]
    pub caves: bool,
    #[serde(default = "yes")]
    pub rivers: bool,
    #[serde(default = "default_stone")]
    pub stone_voxel: VoxelBlock,
    #[serde(default)]
    pub water_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub beach_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub sea_bed_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub cliff_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub snow_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub ice_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub trunk_voxel: Option<VoxelBlock>,
    #[serde(default)]
    pub leaves_voxel: Option<VoxelBlock>,
    #[serde(default = "default_biomes")]
    pub biomes: Vec<BiomeDocument>,
}

impl Default for NaturalTerrainDocument {
    fn default() -> Self {
        Self {
            seed: 0,
            sea_level: 0,
            relief: default_relief(),
            feature_size: default_feature_size(),
            tree_line: default_tree_line(),
            snow_line: default_snow_line(),
            caves: true,
            rivers: true,
            stone_voxel: default_stone(),
            water_voxel: None,
            beach_voxel: None,
            sea_bed_voxel: None,
            cliff_voxel: None,
            snow_voxel: None,
            ice_voxel: None,
            trunk_voxel: None,
            leaves_voxel: None,
            biomes: default_biomes(),
        }
    }
}

impl NaturalTerrainDocument {
    /// A whole world built from the engine's own blocks: seven biomes from
    /// swamp to tundra, water, beaches, bare cliffs, snow, ice and trees.
    ///
    /// What a new voxel world is, so the first thing an author sees is land
    /// rather than three checkered layers.
    #[must_use]
    pub fn with_builtin_blocks() -> Self {
        let biome = |name: &str, climate: (f32, f32), ground: (&str, &str, u32), trees, relief| {
            BiomeDocument {
                name: name.to_owned(),
                temperature: climate.0,
                moisture: climate.1,
                surface_voxel: ground.0.into(),
                subsurface_voxel: ground.1.into(),
                subsurface_depth: ground.2,
                trees,
                relief,
                terraces: 0,
            }
        };
        let mut biomes = vec![
            biome("Grassland", (0.55, 0.4), ("grass", "dirt", 3), 0.06, 0.8),
            biome("Forest", (0.55, 0.72), ("grass", "dirt", 3), 0.55, 1.0),
            biome("Swamp", (0.75, 0.92), ("mud", "mud", 3), 0.2, 0.3),
            biome("Desert", (0.92, 0.12), ("sand", "sand", 5), 0.0, 0.6),
            biome("Badlands", (0.85, 0.35), ("clay", "clay", 6), 0.0, 1.4),
            biome("Taiga", (0.28, 0.65), ("moss", "dirt", 3), 0.45, 1.1),
            biome("Tundra", (0.1, 0.3), ("snow", "dirt", 2), 0.03, 0.7),
        ];
        biomes[4].terraces = 3;
        Self {
            stone_voxel: "stone".into(),
            water_voxel: Some("water".into()),
            beach_voxel: Some("sand".into()),
            sea_bed_voxel: Some("gravel".into()),
            cliff_voxel: Some("rock".into()),
            snow_voxel: Some("snow".into()),
            ice_voxel: Some("ice".into()),
            trunk_voxel: Some("log".into()),
            leaves_voxel: Some("leaves".into()),
            biomes,
            ..Self::default()
        }
    }
}

/// One biome: where it sits on the climate map, and what its ground is.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BiomeDocument {
    /// For people; the engine does not read it.
    #[serde(default)]
    pub name: String,
    /// Where on the climate map the biome sits, both from zero to one. A
    /// column takes the biome whose point is nearest its own climate.
    #[serde(default = "half")]
    pub temperature: f32,
    #[serde(default = "half")]
    pub moisture: f32,
    #[serde(default = "default_surface")]
    pub surface_voxel: VoxelBlock,
    #[serde(default = "default_subsurface")]
    pub subsurface_voxel: VoxelBlock,
    #[serde(default = "default_subsurface_depth")]
    pub subsurface_depth: u32,
    /// How many of the places a tree could stand have one, from zero to one.
    #[serde(default)]
    pub trees: f32,
    /// How rough the land is, as a multiple of the world's hills.
    #[serde(default = "one")]
    pub relief: f32,
    /// The height of the steps the land is cut into; zero for none.
    #[serde(default)]
    pub terraces: u32,
}

impl Default for BiomeDocument {
    fn default() -> Self {
        Self {
            name: "Grassland".to_owned(),
            temperature: half(),
            moisture: half(),
            surface_voxel: default_surface(),
            subsurface_voxel: default_subsurface(),
            subsurface_depth: default_subsurface_depth(),
            trees: 0.0,
            relief: one(),
            terraces: 0,
        }
    }
}

const fn default_relief() -> u32 {
    40
}

const fn default_feature_size() -> u32 {
    64
}

const fn default_tree_line() -> i32 {
    22
}

const fn default_snow_line() -> i32 {
    30
}

const fn yes() -> bool {
    true
}

const fn default_stone() -> VoxelBlock {
    VoxelBlock::Material(3)
}

const fn default_surface() -> VoxelBlock {
    VoxelBlock::Material(1)
}

const fn default_subsurface() -> VoxelBlock {
    VoxelBlock::Material(2)
}

const fn default_subsurface_depth() -> u32 {
    3
}

const fn half() -> f32 {
    0.5
}

const fn one() -> f32 {
    1.0
}

fn default_biomes() -> Vec<BiomeDocument> {
    vec![BiomeDocument::default()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_natural_terrain_uses_only_the_three_starting_materials() {
        let document: NaturalTerrainDocument = serde_json::from_str("{}").unwrap();
        assert_eq!(document, NaturalTerrainDocument::default());
        let mut used = vec![document.stone_voxel.clone()];
        for biome in &document.biomes {
            used.extend([biome.surface_voxel.clone(), biome.subsurface_voxel.clone()]);
        }
        assert!(
            used.iter()
                .all(|voxel| matches!(voxel, VoxelBlock::Material(1..=3))),
            "{used:?}"
        );
        assert!(document.water_voxel.is_none() && document.trunk_voxel.is_none());
    }

    #[test]
    fn an_unchosen_voxel_is_written_as_null_so_it_can_be_chosen() {
        let written = serde_json::to_value(NaturalTerrainDocument::default()).unwrap();
        assert_eq!(written["water_voxel"], serde_json::Value::Null);
    }
}
