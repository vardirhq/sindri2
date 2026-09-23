//! A portable natural-terrain generator: continents and sea, mountain ranges,
//! rivers, biomes, caves, overhangs and trees, all from one seed.
//!
//! Games decide what a coordinate contains, and may keep their own richer
//! sources. This one exists so a scene can ask for a world that is worth
//! walking round without loading game code: the editor constructs it from a
//! document, and every section it produces is a pure function of the settings
//! and the coordinate.
//!
//! The shape of the land is decided first, from fields sampled at a column:
//! where the land is, where it is lifted into ranges, and where water has cut
//! through it. The climate is read next — warmth, which falls with height, and
//! wetness — and a biome is whichever the column's climate is nearest.
//! Biomes blend where their heights meet, so a plain rising into badlands does
//! not do it at a cliff, but their surfaces do not: those fray into each other
//! in tongues and bays, as Causeway's terrains do.
//!
//! Height alone would make every mountain a heap. Two things break that: a
//! band of three-dimensional noise around the surface of the ranges undercuts
//! their faces into overhangs, and tunnels and caverns run through the rock
//! beneath everything.

mod column;
mod features;
mod noise;
mod shape;
#[cfg(test)]
mod tests;

use std::sync::Mutex;

use crate::{SECTION_EDGE, SectionCoord, VoxelCoord, VoxelId, VoxelSection, VoxelSource};

use shape::Shape;

/// A region of the world with its own climate and ground.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainBiome {
    /// Where on the climate map this biome sits, both from zero to one. A
    /// column takes the biome whose point is nearest its own climate.
    pub temperature: f32,
    pub moisture: f32,
    /// What the ground is made of: the top voxel, and the ones under it down
    /// to `subsurface_depth`, below which everything is stone.
    pub surface: VoxelId,
    pub subsurface: VoxelId,
    pub subsurface_depth: u32,
    /// How many of the places a tree could grow have one, from zero to one.
    pub trees: f32,
    /// How rough the land is here, as a multiple of the world's own hills.
    /// Plains are below one, broken country above it. Ranges ignore it.
    pub relief: f32,
    /// The height of the steps the land is cut into, in voxels; zero for
    /// none. Steps of a few voxels make mesas and badlands.
    pub terraces: u32,
}

/// The voxels a world uses for what is not decided by a biome.
///
/// Only stone is required. A role left out is simply not there: no water
/// leaves the sea empty, no snow leaves the peaks bare, and trees need both a
/// trunk and leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPalette {
    pub stone: VoxelId,
    pub water: Option<VoxelId>,
    /// The strip the water reaches, a course or two above it.
    pub beach: Option<VoxelId>,
    /// The ground under water.
    pub sea_bed: Option<VoxelId>,
    /// Bare rock: above the tree line, on steep slopes, and the undercut
    /// faces of the ranges.
    pub cliff: Option<VoxelId>,
    pub snow: Option<VoxelId>,
    /// What cold water freezes into at its surface.
    pub ice: Option<VoxelId>,
    pub trunk: Option<VoxelId>,
    pub leaves: Option<VoxelId>,
}

/// Everything that decides a natural world.
#[derive(Clone, Debug, PartialEq)]
pub struct NaturalTerrainSettings {
    pub seed: u64,
    /// The level water fills to.
    pub sea_level: i32,
    /// How far above the sea the highest ranges reach, in voxels.
    pub relief: u32,
    /// The width of the largest landforms, in voxels. Continents are a few
    /// times this; ranges and rivers about this.
    pub feature_size: u32,
    /// Heights above the sea at which the ground turns to bare rock and then
    /// to snow. Both edges are frayed rather than level.
    pub tree_line: i32,
    pub snow_line: i32,
    pub caves: bool,
    pub rivers: bool,
    pub palette: TerrainPalette,
    /// At least one. A world given none is stone throughout.
    pub biomes: Vec<TerrainBiome>,
}

/// The generator itself: the settings, and what it has already worked out.
pub struct NaturalTerrain {
    settings: NaturalTerrainSettings,
    memo: Memo,
}

impl NaturalTerrain {
    #[must_use]
    pub fn new(mut settings: NaturalTerrainSettings) -> Self {
        if settings.biomes.is_empty() {
            let stone = settings.palette.stone;
            settings.biomes.push(TerrainBiome {
                temperature: 0.5,
                moisture: 0.5,
                surface: stone,
                subsurface: stone,
                subsurface_depth: 0,
                trees: 0.0,
                relief: 1.0,
                terraces: 0,
            });
        }
        Self {
            settings,
            memo: Memo::default(),
        }
    }

    #[must_use]
    pub const fn settings(&self) -> &NaturalTerrainSettings {
        &self.settings
    }

    /// The height of the ground at a column, before caves and overhangs.
    #[must_use]
    pub fn ground(&self, x: i32, z: i32) -> i32 {
        self.shape(x, z).ground
    }

    /// Which of the settings' biomes a column belongs to.
    #[must_use]
    pub fn biome(&self, x: i32, z: i32) -> usize {
        self.shape(x, z).biome
    }

    fn shape(&self, x: i32, z: i32) -> Shape {
        self.memo
            .get(x, z)
            .unwrap_or_else(|| self.memo.put(x, z, self.settings.shape(x, z)))
    }
}

impl Clone for NaturalTerrain {
    fn clone(&self) -> Self {
        // What was worked out is derived state; a copy works it out again.
        Self::new(self.settings.clone())
    }
}

impl PartialEq for NaturalTerrain {
    fn eq(&self, other: &Self) -> bool {
        self.settings == other.settings
    }
}

impl std::fmt::Debug for NaturalTerrain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NaturalTerrain")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl VoxelSource for NaturalTerrain {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        let column = self.column(coord.x, coord.z);
        let trees = if self.may_hold_a_tree(&column, coord.y) {
            self.trees_near(coord.x, coord.z, coord.x, coord.z)
        } else {
            Vec::new()
        };
        self.voxel_in(&column, coord, &trees)
    }

    /// A section a column at a time: each column's shape, surface and trees
    /// are worked out once for its sixteen voxels rather than sixteen times.
    fn generate_section(&self, section: SectionCoord) -> VoxelSection {
        let mut result = VoxelSection::default();
        let min = section.min_voxel();
        let last = SECTION_EDGE - 1;
        let trees = self.trees_near(min.x, min.z, min.x + last, min.z + last);
        for z in 0..SECTION_EDGE {
            for x in 0..SECTION_EDGE {
                let column = self.column(min.x + x, min.z + z);
                if min.y > column.highest(&self.settings) {
                    continue;
                }
                for y in 0..SECTION_EDGE {
                    let coord = VoxelCoord::new(min.x + x, min.y + y, min.z + z);
                    let voxel = self.voxel_in(&column, coord, &trees);
                    if !voxel.is_air() {
                        result.set(coord.local(), voxel);
                    }
                }
            }
        }
        result
    }
}

/// Shapes already worked out, by column.
///
/// The mesher asks about single voxels just outside what is resident, and
/// asks about the same columns over and over as it looks at each face and
/// corner. A shape is a few dozen noise samples; remembering the recent ones
/// is what keeps that affordable. Direct-mapped, so it never grows: a column
/// that collides with another is simply worked out again.
struct Memo(Mutex<Vec<Option<(i32, i32, Shape)>>>);

const MEMO_SLOTS: usize = 4_096;

impl Default for Memo {
    fn default() -> Self {
        Self(Mutex::new(vec![None; MEMO_SLOTS]))
    }
}

impl Memo {
    fn slot(x: i32, z: i32) -> usize {
        let mixed = x.cast_unsigned().wrapping_mul(0x9E37_79B1)
            ^ z.cast_unsigned().wrapping_mul(0x85EB_CA77);
        usize::try_from(mixed >> 20).unwrap_or(0) % MEMO_SLOTS
    }

    fn get(&self, x: i32, z: i32) -> Option<Shape> {
        let slots = self.0.lock().ok()?;
        match slots[Self::slot(x, z)] {
            Some((held_x, held_z, shape)) if held_x == x && held_z == z => Some(shape),
            _ => None,
        }
    }

    fn put(&self, x: i32, z: i32, shape: Shape) -> Shape {
        if let Ok(mut slots) = self.0.lock() {
            slots[Self::slot(x, z)] = Some((x, z, shape));
        }
        shape
    }
}
