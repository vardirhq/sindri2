//! The terrain a voxel world is generated from, built from its document.
//!
//! Each generator is checked here before a world is built from it: an ID no
//! material defines, or a value outside what the generator can use, is an
//! error naming the field rather than a world that draws wrongly.

use std::collections::{BTreeMap, BTreeSet};

use sindri_voxel::{
    NaturalTerrain, NaturalTerrainSettings, SectionCoord, TerrainBiome, TerrainPalette, VoxelCoord,
    VoxelId, VoxelSection, VoxelSource,
};

use crate::{NaturalTerrainDocument, VoxelBlock, VoxelGeneratorDocument};

use super::SceneExtractError;

pub(super) const MAX_HEIGHT_VARIATION: u32 = 4_096;
/// The tallest a natural world's ranges may be, and the widest its landforms.
pub(super) const MAX_RELIEF: u32 = 1_024;
pub(super) const FEATURE_SIZES: (u32, u32) = (8, 4_096);
/// Beyond these a biome's ground and steps stop meaning anything a section can
/// show.
pub(super) const MAX_SUBSURFACE_DEPTH: u32 = 256;
pub(super) const MAX_TERRACE: u32 = 64;
pub(super) const MAX_BIOME_RELIEF: f64 = 4.0;
const TERRAIN_SAMPLE_SCALE: i32 = 8;

/// Either built-in generator, behind one source.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum SceneTerrain {
    Layered(LayeredTerrain),
    Natural(NaturalTerrain),
}

impl VoxelSource for SceneTerrain {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        match self {
            Self::Layered(terrain) => terrain.voxel(coord),
            Self::Natural(terrain) => terrain.voxel(coord),
        }
    }

    fn generate_section(&self, section: SectionCoord) -> VoxelSection {
        match self {
            Self::Layered(terrain) => terrain.generate_section(section),
            Self::Natural(terrain) => terrain.generate_section(section),
        }
    }
}

/// What a world's generator may name: its materials by number, or the
/// blocks of its block set by name, each with the ID the engine stores it as.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Palette {
    pub(super) materials: BTreeSet<u16>,
    pub(super) blocks: BTreeMap<String, u16>,
}

impl Palette {
    pub(super) fn of_materials(materials: impl IntoIterator<Item = u16>) -> Self {
        Self {
            materials: materials.into_iter().collect(),
            blocks: BTreeMap::new(),
        }
    }

    fn resolve(&self, block: &VoxelBlock) -> Result<VoxelId, SceneExtractError> {
        match block {
            VoxelBlock::Material(voxel)
                if *voxel != VoxelId::AIR.value() && self.materials.contains(voxel) =>
            {
                Ok(VoxelId::new(*voxel))
            }
            VoxelBlock::Material(voxel) => Err(SceneExtractError::MissingVoxelMaterial(*voxel)),
            VoxelBlock::Named(name) => self
                .blocks
                .get(name)
                .map(|voxel| VoxelId::new(*voxel))
                .ok_or_else(|| SceneExtractError::UnknownVoxelBlock(name.clone())),
        }
    }
}

/// The source a generator document describes, if every block it names is in
/// `palette` and every value is one it can use.
pub(super) fn terrain_source(
    generator: &VoxelGeneratorDocument,
    palette: &Palette,
) -> Result<SceneTerrain, SceneExtractError> {
    let defined = |block: &VoxelBlock| palette.resolve(block);
    match generator {
        VoxelGeneratorDocument::LayeredTerrain {
            seed,
            base_height,
            height_variation,
            surface_voxel,
            subsurface_voxel,
            deep_voxel,
            subsurface_depth,
        } => {
            if *height_variation > MAX_HEIGHT_VARIATION {
                return Err(SceneExtractError::VoxelHeightVariationTooLarge {
                    variation: *height_variation,
                    maximum: MAX_HEIGHT_VARIATION,
                });
            }
            Ok(SceneTerrain::Layered(LayeredTerrain {
                seed: *seed,
                base_height: *base_height,
                height_variation: *height_variation,
                surface_voxel: defined(surface_voxel)?,
                subsurface_voxel: defined(subsurface_voxel)?,
                deep_voxel: defined(deep_voxel)?,
                subsurface_depth: *subsurface_depth,
            }))
        }
        VoxelGeneratorDocument::NaturalTerrain(document) => natural(document, &defined)
            .map(|settings| SceneTerrain::Natural(NaturalTerrain::new(settings))),
    }
}

fn natural(
    document: &NaturalTerrainDocument,
    defined: &dyn Fn(&VoxelBlock) -> Result<VoxelId, SceneExtractError>,
) -> Result<NaturalTerrainSettings, SceneExtractError> {
    let optional = |voxel: &Option<VoxelBlock>| voxel.as_ref().map(defined).transpose();
    within(
        "relief",
        f64::from(document.relief),
        1.0..=f64::from(MAX_RELIEF),
        "from 1 to 1024",
    )?;
    within(
        "feature_size",
        f64::from(document.feature_size),
        f64::from(FEATURE_SIZES.0)..=f64::from(FEATURE_SIZES.1),
        "from 8 to 4096",
    )?;
    if document.biomes.is_empty() {
        return Err(SceneExtractError::VoxelGeneratorField {
            field: "biomes".to_owned(),
            requirement: "at least one biome",
        });
    }
    let biomes = document
        .biomes
        .iter()
        .enumerate()
        .map(|(index, biome)| {
            let field = |name: &str| format!("biomes.{index}.{name}");
            for (name, value) in [
                ("temperature", biome.temperature),
                ("moisture", biome.moisture),
                ("trees", biome.trees),
            ] {
                within(&field(name), f64::from(value), 0.0..=1.0, "from 0 to 1")?;
            }
            within(
                &field("relief"),
                f64::from(biome.relief),
                0.0..=MAX_BIOME_RELIEF,
                "from 0 to 4",
            )?;
            within(
                &field("subsurface_depth"),
                f64::from(biome.subsurface_depth),
                0.0..=f64::from(MAX_SUBSURFACE_DEPTH),
                "from 0 to 256",
            )?;
            within(
                &field("terraces"),
                f64::from(biome.terraces),
                0.0..=f64::from(MAX_TERRACE),
                "from 0 to 64",
            )?;
            Ok(TerrainBiome {
                temperature: biome.temperature,
                moisture: biome.moisture,
                surface: defined(&biome.surface_voxel)?,
                subsurface: defined(&biome.subsurface_voxel)?,
                subsurface_depth: biome.subsurface_depth,
                trees: biome.trees,
                relief: biome.relief,
                terraces: biome.terraces,
            })
        })
        .collect::<Result<Vec<_>, SceneExtractError>>()?;
    Ok(NaturalTerrainSettings {
        seed: document.seed,
        sea_level: document.sea_level,
        relief: document.relief,
        feature_size: document.feature_size,
        tree_line: document.tree_line,
        snow_line: document.snow_line,
        caves: document.caves,
        rivers: document.rivers,
        palette: TerrainPalette {
            stone: defined(&document.stone_voxel)?,
            water: optional(&document.water_voxel)?,
            beach: optional(&document.beach_voxel)?,
            sea_bed: optional(&document.sea_bed_voxel)?,
            cliff: optional(&document.cliff_voxel)?,
            snow: optional(&document.snow_voxel)?,
            ice: optional(&document.ice_voxel)?,
            trunk: optional(&document.trunk_voxel)?,
            leaves: optional(&document.leaves_voxel)?,
        },
        biomes,
    })
}

/// A value inside `range`, or an error naming the field and saying what it
/// must be.
fn within(
    field: &str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    requirement: &'static str,
) -> Result<(), SceneExtractError> {
    if value.is_finite() && range.contains(&value) {
        return Ok(());
    }
    Err(SceneExtractError::VoxelGeneratorField {
        field: field.to_owned(),
        requirement,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LayeredTerrain {
    seed: u64,
    base_height: i32,
    height_variation: u32,
    surface_voxel: VoxelId,
    subsurface_voxel: VoxelId,
    deep_voxel: VoxelId,
    subsurface_depth: u32,
}

impl LayeredTerrain {
    fn height(&self, x: i32, z: i32) -> i32 {
        if self.height_variation == 0 {
            return self.base_height;
        }
        let grid_x = x.div_euclid(TERRAIN_SAMPLE_SCALE);
        let grid_z = z.div_euclid(TERRAIN_SAMPLE_SCALE);
        let offset_x = i64::from(x.rem_euclid(TERRAIN_SAMPLE_SCALE));
        let offset_z = i64::from(z.rem_euclid(TERRAIN_SAMPLE_SCALE));
        let sample = |sample_x, sample_z| {
            i64::from(
                u32::try_from(
                    terrain_hash(self.seed, sample_x, sample_z)
                        % u64::from(self.height_variation.saturating_add(1)),
                )
                .expect("terrain variation fits in u32"),
            )
        };
        let lerp = |from: i64, to: i64, offset: i64| {
            (from * (i64::from(TERRAIN_SAMPLE_SCALE) - offset) + to * offset)
                / i64::from(TERRAIN_SAMPLE_SCALE)
        };
        let near = lerp(
            sample(grid_x, grid_z),
            sample(grid_x.saturating_add(1), grid_z),
            offset_x,
        );
        let far = lerp(
            sample(grid_x, grid_z.saturating_add(1)),
            sample(grid_x.saturating_add(1), grid_z.saturating_add(1)),
            offset_x,
        );
        self.base_height.saturating_add(
            i32::try_from(lerp(near, far, offset_z)).expect("terrain variation fits in i32"),
        )
    }
}

fn terrain_hash(seed: u64, x: i32, z: i32) -> u64 {
    let x = u64::from(u32::from_ne_bytes(x.to_ne_bytes()));
    let z = u64::from(u32::from_ne_bytes(z.to_ne_bytes()));
    let mut value = seed ^ x.wrapping_mul(0x9e37_79b1) ^ z.wrapping_mul(0x85eb_ca77);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

impl VoxelSource for LayeredTerrain {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        let height = self.height(coord.x, coord.z);
        if coord.y > height {
            VoxelId::AIR
        } else if coord.y == height {
            self.surface_voxel
        } else if u32::try_from(height.saturating_sub(coord.y))
            .is_ok_and(|depth| depth <= self.subsurface_depth)
        {
            self.subsurface_voxel
        } else {
            self.deep_voxel
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[u16]) -> Palette {
        Palette::of_materials(values.iter().copied())
    }

    #[test]
    fn layered_terrain_is_solid_below_its_surface() {
        let Ok(SceneTerrain::Layered(source)) =
            terrain_source(&VoxelGeneratorDocument::default(), &ids(&[1, 2, 3]))
        else {
            panic!("the default generator is layered and valid");
        };
        let height = source.height(12, -7);
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height + 1, -7)),
            VoxelId::AIR
        );
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height, -7)),
            VoxelId::new(1)
        );
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height - 20, -7)),
            VoxelId::new(3)
        );
    }

    #[test]
    fn a_generator_names_blocks_by_name() {
        let palette = Palette {
            materials: BTreeSet::new(),
            blocks: [("dirt", 1), ("grass", 2), ("stone", 3)]
                .into_iter()
                .map(|(name, voxel)| (name.to_owned(), voxel))
                .collect(),
        };
        let generator = VoxelGeneratorDocument::LayeredTerrain {
            seed: 0,
            base_height: 3,
            height_variation: 6,
            surface_voxel: "grass".into(),
            subsurface_voxel: "dirt".into(),
            deep_voxel: "stone".into(),
            subsurface_depth: 3,
        };
        let Ok(SceneTerrain::Layered(source)) = terrain_source(&generator, &palette) else {
            panic!("every block is in the set");
        };
        let height = source.height(0, 0);
        assert_eq!(source.voxel(VoxelCoord::new(0, height, 0)), VoxelId::new(2));
        // A number names a material, and a world built from blocks has none.
        let numbered = VoxelGeneratorDocument::default();
        assert!(terrain_source(&numbered, &palette).is_err());
    }

    #[test]
    fn a_default_natural_terrain_needs_only_the_starting_materials() {
        let generator = VoxelGeneratorDocument::NaturalTerrain(NaturalTerrainDocument::default());
        assert!(matches!(
            terrain_source(&generator, &ids(&[1, 2, 3])),
            Ok(SceneTerrain::Natural(_))
        ));
    }

    #[test]
    fn a_natural_terrain_names_the_field_it_cannot_use() {
        let mut document = NaturalTerrainDocument {
            water_voxel: Some(VoxelBlock::Material(9)),
            ..NaturalTerrainDocument::default()
        };
        let generator = VoxelGeneratorDocument::NaturalTerrain(document.clone());
        let error = terrain_source(&generator, &ids(&[1, 2, 3])).unwrap_err();
        assert!(error.to_string().contains("material ID 9"), "{error}");

        document.water_voxel = Some(VoxelBlock::from("lava"));
        let generator = VoxelGeneratorDocument::NaturalTerrain(document.clone());
        let error = terrain_source(&generator, &ids(&[1, 2, 3])).unwrap_err();
        assert_eq!(
            error.to_string(),
            "no block called `lava` in the world's block set"
        );

        document.water_voxel = None;
        document.biomes[0].trees = 1.5;
        let generator = VoxelGeneratorDocument::NaturalTerrain(document);
        let error = terrain_source(&generator, &ids(&[1, 2, 3])).unwrap_err();
        assert_eq!(
            error.to_string(),
            "voxel generator `biomes.0.trees` must be from 0 to 1"
        );
    }
}
