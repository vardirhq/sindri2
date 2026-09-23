//! The shape of the land at a column, and its climate.

use super::NaturalTerrainSettings;
use super::noise::{fbm2, spread};

// Each field is sampled from its own salt of the seed, so no two of them
// rise and fall together.
const WARP_X: u64 = 0x11;
const WARP_Z: u64 = 0x12;
const RIDGE: u64 = 0x51;
const UPLIFT: u64 = 0x77;
const DETAIL: u64 = 0x93;
const RIVER: u64 = 0x5A;
const WARMTH: u64 = 0xB3;
const WETNESS: u64 = 0xA7;
const FRAY_WARMTH: u64 = 0xC1;
const FRAY_WETNESS: u64 = 0xC2;

/// How far a biome's influence on height reaches across the climate map.
/// Wide enough that heights blend over tens of voxels, not at a step.
const BLEND: f32 = 0.015;

/// What a column is like before it is turned into voxels.
#[derive(Clone, Copy, Debug)]
pub(super) struct Shape {
    /// The top of the ground, before caves and overhangs.
    pub ground: i32,
    /// How much of a mountain range this column is, from zero to one.
    pub mountain: f32,
    pub temperature: f32,
    /// The biome whose surface this column wears.
    pub biome: usize,
}

impl NaturalTerrainSettings {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub(super) fn shape(&self, x: i32, z: i32) -> Shape {
        let seed = self.seed;
        let (fx, fz) = (x as f32, z as f32);
        let size = self.feature_size.max(1) as f32;
        let relief = self.relief.max(1) as f32;
        let sea = self.sea_level as f32;

        // Sampled through a warp, so coastlines and ranges curl rather than
        // running in the straight-ish lines value noise draws on its own.
        let warp = size * 0.5;
        let wx = fx + (fbm2(seed ^ WARP_X, fx, fz, 2, size) - 0.5) * warp;
        let wz = fz + (fbm2(seed ^ WARP_Z, fx, fz, 2, size) - 0.5) * warp;

        // Where the land is: above zero is land, below it sea.
        // Biased a little towards land: an even split puts half the world
        // under water, and a world is mostly for walking on.
        let land = spread(fbm2(seed, wx, wz, 5, size * 2.0), 2.2) * 2.0 - 0.9;
        // A ridge field, folded so its peaks are creases rather than domes and
        // cubed so they are rare. More octaves of the same noise give rolling
        // hills everywhere and mountains nowhere.
        let folded =
            1.0 - (spread(fbm2(seed ^ RIDGE, wx, wz, 4, size * 0.75), 1.8) * 2.0 - 1.0).abs();
        let ridges = folded * folded * folded;
        // Where ranges are allowed to be, so one runs along part of the world
        // instead of peaks sprouting evenly across all of it.
        let uplift =
            ((spread(fbm2(seed ^ UPLIFT, wx, wz, 2, size * 1.5), 2.0) - 0.5) * 2.5).clamp(0.0, 1.0);
        let inland = ((land + 0.15) * 4.0).clamp(0.0, 1.0);
        let mountain = ridges * uplift * inland;
        let detail = fbm2(seed ^ DETAIL, fx, fz, 3, 14.0) - 0.5;

        let hills = if land > 0.0 { land * 0.3 } else { land * 0.45 };
        let height_for = |roughness: f32| {
            let lowland = if land > 0.0 { hills * roughness } else { hills };
            sea + relief * (lowland + mountain * 0.85) + detail * 6.0 * roughness
        };

        // Warmth falls with height, so the ranges are colder than the plains
        // around them whatever the weather says. The estimate uses plain
        // roughness: the biome is what is being decided.
        let lapse = ((height_for(1.0) - sea) / relief).max(0.0) * 0.35;
        let temperature =
            (spread(fbm2(seed ^ WARMTH, fx, fz, 3, size * 5.0), 2.6) - lapse).clamp(0.0, 1.0);
        let moisture = spread(fbm2(seed ^ WETNESS, fx, fz, 3, size * 4.0), 2.6);

        let biome = self.nearest_biome(
            temperature + (fbm2(seed ^ FRAY_WARMTH, fx, fz, 2, 5.0) - 0.5) * 0.25,
            moisture + (fbm2(seed ^ FRAY_WETNESS, fx, fz, 2, 5.0) - 0.5) * 0.25,
        );
        let weights = self.biome_weights(temperature, moisture);
        let roughness: f32 = weights
            .iter()
            .zip(&self.biomes)
            .map(|(weight, biome)| weight * biome.relief.max(0.0))
            .sum();
        let mut height = height_for(roughness);

        // Terraces are blended by weight too: a badland's steps flatten out
        // into the plain beside it instead of ending at a wall.
        if height > sea {
            height = weights
                .iter()
                .zip(&self.biomes)
                .map(|(weight, biome)| weight * terraced(height, sea, biome.terraces))
                .sum();
        }

        if self.rivers && land > -0.05 {
            height = self.carve_river(height, sea, relief, size, (wx, wz));
        }

        Shape {
            ground: height.round() as i32,
            mountain,
            temperature,
            biome,
        }
    }

    /// A river is where a slow field crosses its middle: a line that wanders,
    /// never ends, and never crosses itself. It cuts the lowlands down to just
    /// below the sea, so the sea fills it, and fades out as the ground rises,
    /// which is where rivers rise.
    fn carve_river(&self, height: f32, sea: f32, relief: f32, size: f32, at: (f32, f32)) -> f32 {
        const WIDTH: f32 = 0.04;
        let channel =
            (spread(fbm2(self.seed ^ RIVER, at.0, at.1, 3, size * 1.5), 1.6) * 2.0 - 1.0).abs();
        if channel >= WIDTH {
            return height;
        }
        let depth = 1.0 - channel / WIDTH;
        let depth = depth * depth * (3.0 - 2.0 * depth);
        let lowland = 1.0 - ((height - sea) / (relief * 0.5)).clamp(0.0, 1.0);
        let bed = sea - 1.0 - depth * 2.0;
        if height <= bed {
            return height;
        }
        height + (bed - height) * depth * lowland
    }

    fn nearest_biome(&self, temperature: f32, moisture: f32) -> usize {
        self.biomes
            .iter()
            .map(|biome| distance(biome.temperature, biome.moisture, temperature, moisture))
            .enumerate()
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map_or(0, |(index, _)| index)
    }

    /// How much each biome has to say about a column's height, summing to one.
    fn biome_weights(&self, temperature: f32, moisture: f32) -> Vec<f32> {
        let mut weights: Vec<f32> = self
            .biomes
            .iter()
            .map(|biome| {
                (-distance(biome.temperature, biome.moisture, temperature, moisture) / BLEND).exp()
            })
            .collect();
        let total: f32 = weights.iter().sum();
        if total > f32::MIN_POSITIVE {
            for weight in &mut weights {
                *weight /= total;
            }
        } else {
            // Further from every biome than the exponent can say: the nearest
            // one decides alone.
            weights.fill(0.0);
            if let Some(nearest) = weights.get_mut(self.nearest_biome(temperature, moisture)) {
                *nearest = 1.0;
            }
        }
        weights
    }
}

/// Squared distance on the climate map.
fn distance(from_t: f32, from_m: f32, to_t: f32, to_m: f32) -> f32 {
    (from_t - to_t).powi(2) + (from_m - to_m).powi(2)
}

/// A height cut down to the step below it.
#[allow(clippy::cast_precision_loss)]
fn terraced(height: f32, sea: f32, step: u32) -> f32 {
    if step == 0 {
        return height;
    }
    let step = step as f32;
    sea + ((height - sea) / step).floor() * step
}
