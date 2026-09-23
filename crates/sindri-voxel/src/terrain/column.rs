//! What a column is made of: which voxel is on top, what is under it, and
//! whether anything may grow there.

use crate::VoxelId;

use super::features::TREE_REACH;
use super::noise::noise2;
use super::shape::Shape;
use super::{NaturalTerrain, NaturalTerrainSettings};

const FRAY_SHORE: u64 = 0xD1;
const FRAY_SNOW: u64 = 0xD2;
const FRAY_ROCK: u64 = 0xD3;

/// Below this warmth the sea's surface freezes.
const FREEZING: f32 = 0.18;

/// A difference in height to a neighbour at which a slope is a face rather
/// than a hillside, and is bare rock.
const STEEP: i32 = 3;

/// The deepest an overhang reaches below or above the ground, in voxels, in
/// the heart of a range.
pub(super) const OVERHANG: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Column {
    pub shape: Shape,
    pub top: VoxelId,
    pub under: VoxelId,
    /// How far below the top `under` reaches before stone.
    pub under_depth: i32,
    /// Whether the sea's surface over this column is ice.
    pub frozen: bool,
    /// Whether a tree could stand here: the biome's own ground, not too steep.
    pub open: bool,
}

impl Column {
    /// The highest a solid voxel of this column could be, not counting trees.
    #[allow(clippy::cast_possible_truncation)]
    pub fn highest(&self, settings: &NaturalTerrainSettings) -> i32 {
        let overhang = (self.shape.mountain * OVERHANG).ceil() as i32;
        let crown = if settings.palette.trunk.is_some() {
            TREE_REACH
        } else {
            0
        };
        self.shape
            .ground
            .max(settings.sea_level)
            .saturating_add(overhang.max(crown))
    }
}

impl NaturalTerrain {
    pub(super) fn column(&self, x: i32, z: i32) -> Column {
        let shape = self.shape(x, z);
        let slope = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .map(|(dx, dz)| (self.shape(x + dx, z + dz).ground - shape.ground).abs())
            .max()
            .unwrap_or(0);
        self.settings.surface(x, z, shape, slope)
    }
}

impl NaturalTerrainSettings {
    /// Every threshold here is frayed rather than level, each from its own
    /// salt so two that meet do not fray in step and draw one ragged line
    /// twice.
    #[allow(clippy::cast_precision_loss)]
    fn surface(&self, x: i32, z: i32, shape: Shape, slope: i32) -> Column {
        let palette = self.palette;
        let biome = &self.biomes[shape.biome];
        let stone = palette.stone;
        let (level, sea) = (shape.ground as f32, self.sea_level as f32);
        let column = |top, under, under_depth, open| Column {
            shape,
            top,
            under,
            under_depth,
            frozen: palette.ice.is_some() && shape.temperature < FREEZING,
            open,
        };

        if shape.ground < self.sea_level {
            let bed = palette.sea_bed.unwrap_or(biome.subsurface);
            return column(bed, bed, 2, false);
        }
        // A beach is the strip the water reaches, a course or two above it,
        // and nothing more: any wider and a flat world reads as a desert.
        if let Some(beach) = palette.beach
            && !self.past(FRAY_SHORE, x, z, level, sea + 1.5, 1.0)
        {
            return column(beach, beach, 3, false);
        }
        // Snow lies in tongues down the gullies and bares the ridges; a line
        // of it at exactly one height is the one thing it never does.
        if let Some(snow) = palette.snow
            && self.past(
                FRAY_SNOW,
                x,
                z,
                level,
                sea + self.snow_line as f32 - 0.5,
                2.0,
            )
        {
            return column(snow, palette.cliff.unwrap_or(stone), 2, false);
        }
        if let Some(cliff) = palette.cliff
            && (slope >= STEEP
                || self.past(
                    FRAY_ROCK,
                    x,
                    z,
                    level,
                    sea + self.tree_line as f32 - 0.5,
                    2.0,
                ))
        {
            return column(cliff, cliff, 3, false);
        }
        let depth = i32::try_from(biome.subsurface_depth).unwrap_or(i32::MAX);
        column(biome.surface, biome.subsurface, depth, slope <= 1)
    }

    /// Whether `value` has passed `threshold`, with the crossing frayed.
    ///
    /// Within `band` either side of the threshold the answer comes from a
    /// field sampled at a few voxels' scale, more often yes the further past
    /// it the column is. Deciding each column on its own would give salt and
    /// pepper; a field makes neighbours agree, so two terrains meet in tongues
    /// and bays.
    #[allow(clippy::cast_precision_loss)]
    pub(super) fn past(
        &self,
        salt: u64,
        x: i32,
        z: i32,
        value: f32,
        threshold: f32,
        band: f32,
    ) -> bool {
        const FRAY: f32 = 2.6;
        let lean = (value - threshold) / (band * 2.0) + 0.5;
        if lean <= 0.0 {
            return false;
        }
        if lean >= 1.0 {
            return true;
        }
        noise2(self.seed ^ salt, x as f32 / FRAY, z as f32 / FRAY) < lean
    }
}
