//! The world, built from a seed rather than authored.
//!
//! A world worth walking across is a hundred and sixty cells on a side, which
//! is twenty-five thousand columns and rather more cells. Written out as a
//! scene that is tens of megabytes of JSON compiled into the binary, and it
//! would still be one fixed island. So the scene authors the *grid* -- how big
//! a cell is, where the floor stands -- and this fills it in before the first
//! frame.
//!
//! Three fields decide everything: how high the ground is, how wet it is, and
//! how cold. Height alone gives mountains and sea; wetness turns low flat
//! ground into swamp and high ground into forest floor; cold puts snow on the
//! peaks and, towards one end of the map, brings it down to the shore. A biome
//! is not a region drawn on a map here -- it is what those three numbers happen
//! to be at a column, which is why beaches are long and swamps have edges
//! nobody placed.
//!
//! Those edges are frayed rather than sharp. Comparing a smooth field against a
//! fixed number draws a smooth line, and smooth lines through a landscape read
//! as borders on a map: snow stopping at one height the whole way round a
//! mountain, marsh ending along a contour. Near each threshold the answer comes
//! instead from a field sampled at a few cells' scale, so the two terrains
//! interlock in tongues and bays. See `past`.

use sindri_scene::{TileCellDocument, TileVolumeComponent};

/// The level the sea fills to. A column whose ground is below this is under
/// water.
pub const SEA: i32 = 0;

/// How many levels of relief there are between the sea bed and the peaks.
const RELIEF: f32 = 56.0;

/// Above this, ground is bare rock; above it again, snow.
const TREE_LINE: i32 = 9;
const SNOW_LINE: i32 = 13;

/// How far below its own surface a column is filled in.
///
/// Only as far as its lowest neighbour, because the only reason to fill a cell
/// nobody can stand on is to stop daylight showing through the side of a
/// cliff. Filling to a fixed depth everywhere would be a hundred thousand
/// cells that are never seen; filling to none would make every step down a
/// hole in the world.
const MAX_SKIRT: i32 = 6;

/// A value noise field, hashed from its own coordinates.
///
/// Hashed rather than drawn from a random stream, so a column's height is a
/// property of where it is: the same seed gives the same world, and asking
/// about one column does not depend on having asked about another first.
fn hashed(seed: u64, x: i32, y: i32) -> f32 {
    // The sign is kept rather than lost: a column west of the origin is a
    // different column from one east of it, and a cast that folded them
    // together would mirror the world about its own edge.
    let mut value = seed
        ^ i64::from(x)
            .cast_unsigned()
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ i64::from(y)
            .cast_unsigned()
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^= value >> 33;
    // The top 24 bits are plenty, and keep the cast exact.
    #[allow(clippy::cast_precision_loss)]
    let unit = (value >> 40) as f32 / f32::from(u16::MAX) / 256.0;
    unit
}

/// Whether `value` has passed `threshold`, with the crossing frayed.
///
/// A threshold on a smooth field draws a smooth line, and a smooth line
/// through a landscape reads as a border drawn on a map rather than as one
/// ground giving way to another. Marsh does not stop along a contour.
///
/// Within a band either side of the threshold the answer is decided per column
/// instead -- more often the further past it the column is -- by a hash of
/// where that column is. The two terrains then interlock along their edge in a
/// way that is different everywhere, and identical on every machine and every
/// run, which is what the render captures depend on.
///
/// `band` is in whatever `value` is measured in: a fraction for moisture and
/// warmth, levels for a height.
fn past(seed: u64, x: i32, y: i32, value: f32, threshold: f32, band: f32) -> bool {
    // How far through the band this column sits: 0 at its low edge, 1 at its
    // high one. Outside the band there is nothing to decide.
    let lean = (value - threshold) / (band * 2.0) + 0.5;
    if lean <= 0.0 {
        return false;
    }
    if lean >= 1.0 {
        return true;
    }
    // A smooth field at a few cells' scale rather than a hash per column.
    // Deciding each column on its own gives salt and pepper -- single cells of
    // snow scattered through grass, which reads as dirt on the screen rather
    // than as snow lying in the hollows. Sampling a field instead makes the
    // decision agree with its neighbours' for a few cells at a time, so the
    // two terrains meet in tongues and bays.
    #[allow(clippy::cast_precision_loss)]
    let (fx, fy) = (x as f32 / FRAY, y as f32 / FRAY);
    noise(seed, fx, fy) < lean
}

/// How wide the tongues along a frayed boundary are, in cells.
///
/// Small enough that an edge is ragged rather than wandering, large enough
/// that it is a shape rather than a dither.
const FRAY: f32 = 2.6;

/// Smoothly interpolated value noise at one frequency.
fn noise(seed: u64, x: f32, y: f32) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    // Smoothstep, so the grid the noise is built on does not show as a lattice
    // of straight creases running through every hillside.
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let top = hashed(seed, ix, iy) * (1.0 - sx) + hashed(seed, ix + 1, iy) * sx;
    let bottom = hashed(seed, ix, iy + 1) * (1.0 - sx) + hashed(seed, ix + 1, iy + 1) * sx;
    top * (1.0 - sy) + bottom * sy
}

/// Several octaves of it, which is what makes a coastline ragged at every
/// scale rather than smooth with a wobble.
fn fbm(seed: u64, x: f32, y: f32, octaves: u32, scale: f32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut sum = 0.0;
    let mut frequency = 1.0 / scale;
    for octave in 0..octaves {
        total += noise(seed ^ u64::from(octave) << 17, x * frequency, y * frequency) * amplitude;
        sum += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    total / sum
}

/// How big a world is, in cells.
#[derive(Clone, Copy, Debug)]
pub struct WorldShape {
    pub columns: i32,
    pub rows: i32,
    pub seed: u64,
}

impl Default for WorldShape {
    fn default() -> Self {
        Self {
            columns: 160,
            rows: 160,
            seed: 0x5E11_A1D2_0C4F_1907,
        }
    }
}

/// What a column is like, before it is turned into cells.
#[derive(Clone, Copy, Debug)]
struct Column {
    ground: i32,
    moisture: f32,
    warmth: f32,
}

impl WorldShape {
    fn column(self, x: i32, y: i32) -> Column {
        #[allow(clippy::cast_precision_loss)]
        let (fx, fy) = (x as f32, y as f32);
        // Three things make a landscape: where the land is, where it rises
        // into mountains, and the roughness on both.
        let land = fbm(self.seed, fx, fy, 5, 64.0);
        // A ridge field -- folded so its peaks are creases rather than domes,
        // and squared so they are rare. Without it, more octaves of the same
        // noise give rolling hills everywhere and mountains nowhere.
        let folded = 1.0 - (fbm(self.seed ^ 0x51, fx, fy, 4, 46.0) * 2.0 - 1.0).abs();
        let ridges = folded * folded * folded;
        // Where the mountains are allowed to be, so a range runs along part of
        // the map instead of peaks sprouting evenly across all of it.
        let uplift = (fbm(self.seed ^ 0x77, fx, fy, 2, 96.0) - 0.42).max(0.0) * 2.4;
        let detail = fbm(self.seed ^ 0x93, fx, fy, 3, 11.0) * 0.06;
        let height = (land * 0.62 + ridges * uplift * 0.7 + detail).clamp(0.0, 1.0);

        #[allow(clippy::cast_possible_truncation)]
        let ground = ((height - 0.38) * RELIEF).round() as i32;

        let moisture = fbm(self.seed ^ 0xA7, fx, fy, 4, 41.0);
        // Latitude plus weather: one end of the map is cold whatever the
        // ground does, and height takes the rest down.
        #[allow(clippy::cast_precision_loss)]
        let latitude = 1.0 - (fy / self.rows.max(1) as f32);
        let warmth =
            (latitude * 0.7 + fbm(self.seed ^ 0xB3, fx, fy, 3, 63.0) * 0.3).clamp(0.0, 1.0);
        Column {
            ground,
            moisture,
            warmth,
        }
    }

    /// The tile on top of a column, which is the one you see and walk on.
    ///
    /// Every threshold here is frayed rather than sharp, and each is frayed
    /// from its own salt so that two of them meeting do not fray in step and
    /// draw the same ragged line twice.
    fn surface(self, x: i32, y: i32, column: Column) -> &'static str {
        let Column {
            ground,
            moisture,
            warmth,
        } = column;
        // The levels this compares against, as heights. One conversion for all
        // of them, because a level is a small number and an f32 holds it.
        #[allow(clippy::cast_precision_loss)]
        let (level, sea, tree, snow) = (
            ground as f32,
            SEA as f32,
            TREE_LINE as f32,
            SNOW_LINE as f32,
        );
        // A snow line is the clearest case for fraying: real snow lies in
        // tongues down the gullies and bares the ridges, and a line of it at
        // exactly one height is the one thing it never does.
        let high = past(self.seed ^ 0xD1, x, y, level, snow - 0.5, 1.5);
        let cold = past(self.seed ^ 0xD2, x, y, 0.22, warmth, 0.05);
        if high || (cold && ground > SEA) {
            return "snow";
        }
        if past(self.seed ^ 0xD3, x, y, level, tree - 0.5, 1.5) {
            return "rock";
        }
        // The shore, and only the shore. A beach is the strip the sea reaches,
        // so it is one or two courses above the waterline and nothing else --
        // the first try made it every column within two levels of the sea,
        // which on a world this flat was most of the world, and the whole map
        // read as desert.
        if !past(self.seed ^ 0xD4, x, y, level, sea + 1.5, 1.0) {
            if past(self.seed ^ 0xD5, x, y, moisture, 0.58, 0.05) {
                // Low, flat and wet is a swamp rather than a beach.
                return "mud";
            }
            return "sand";
        }
        if past(self.seed ^ 0xD6, x, y, moisture, 0.68, 0.05)
            && !past(self.seed ^ 0xD7, x, y, level, sea + 3.5, 1.0)
        {
            return "moss";
        }
        if past(self.seed ^ 0xD8, x, y, 0.26, moisture, 0.05)
            && past(self.seed ^ 0xD9, x, y, level, sea + 4.5, 1.0)
        {
            return "gravel";
        }
        "ground"
    }

    /// What is under that, which is only seen where the ground is cut.
    fn beneath(column: Column, surface: &'static str) -> &'static str {
        match surface {
            "snow" | "rock" => "rock",
            "sand" => "sand",
            "mud" | "moss" => "mud",
            "gravel" => "stone",
            _ if column.ground > SEA + 4 => "stone",
            _ => "earth",
        }
    }

    /// The bed under water, which is what a shallow shows.
    fn bed(self, x: i32, y: i32, column: Column) -> &'static str {
        #[allow(clippy::cast_precision_loss)]
        let (level, sea) = (column.ground as f32, SEA as f32);
        if past(self.seed ^ 0xE1, x, y, 0.2, column.warmth, 0.04) {
            "ice"
        } else if past(self.seed ^ 0xE2, x, y, column.moisture, 0.66, 0.05) {
            "mud"
        } else if !past(self.seed ^ 0xE3, x, y, level, sea - 2.5, 1.0) {
            "gravel"
        } else {
            "sand"
        }
    }
}

/// Every cell of a generated world, ready to hand to a tile volume.
///
/// A column is filled from its own surface down only as far as its lowest
/// neighbour needs, because the only reason to fill a cell nobody can stand on
/// is to stop daylight showing through the side of a cliff. Filling to a fixed
/// depth everywhere would be a hundred thousand cells nothing ever sees.
#[must_use]
pub fn generate(shape: WorldShape, tileset: &str) -> TileVolumeComponent {
    let mut cells = Vec::new();
    let grounds: Vec<i32> = (0..shape.rows)
        .flat_map(|row| (0..shape.columns).map(move |column| (column, row)))
        .map(|(column, row)| shape.column(column, row).ground)
        .collect();
    let at = |column: i32, row: i32| -> Option<i32> {
        (column >= 0 && row >= 0 && column < shape.columns && row < shape.rows).then(|| {
            let index = row * shape.columns + column;
            grounds[usize::try_from(index).expect("a cell inside the world")]
        })
    };

    for row in 0..shape.rows {
        for column in 0..shape.columns {
            let described = shape.column(column, row);
            let mut push = |level: i32, tile: &str| {
                cells.push(TileCellDocument {
                    position: [column, row, level],
                    tile: tile.to_owned(),
                    visual_override: None,
                });
            };
            if described.ground < SEA {
                // Only the surface of the sea and one shelf beneath it. Water
                // is opaque, so a bed dug to its real depth would be a
                // thousand cells nobody can see -- and the shelf is what a
                // shallow at the shore shows.
                push(SEA - 1, shape.bed(column, row, described));
                push(SEA, "water");
                continue;
            }
            let surface = shape.surface(column, row, described);
            push(described.ground, surface);

            let lowest = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .filter_map(|(dx, dy)| at(column + dx, row + dy))
                .min()
                .unwrap_or(described.ground)
                .max(SEA - 1);
            let skirt = (described.ground - lowest).clamp(0, MAX_SKIRT);
            let beneath = WorldShape::beneath(described, surface);
            for step in 1..=skirt {
                push(described.ground - step, beneath);
            }
        }
    }

    grow_trees(shape, &mut cells);

    TileVolumeComponent {
        tileset: tileset.to_owned(),
        cells,
        variant_seed: shape.seed,
        ..TileVolumeComponent::default()
    }
}

/// Where the game begins, and what it is walking towards.
///
/// Neither can be authored once the world is generated: a cell that was a
/// meadow under one seed is the bottom of a lake under the next, and a scene
/// that named coordinates would start the game underwater as soon as anything
/// about the terrain changed. So the world is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landfall {
    pub start: [i32; 2],
    pub goal: [i32; 2],
}

impl WorldShape {
    /// Whether a walker could stand on this column.
    fn is_footing(self, column: i32, row: i32) -> bool {
        if column < 0 || row < 0 || column >= self.columns || row >= self.rows {
            return false;
        }
        let described = self.column(column, row);
        described.ground >= SEA && !matches!(self.surface(column, row, described), "snow" | "rock")
    }

    /// The nearest standable column to a point, searched outwards.
    fn nearest_footing(self, from: [i32; 2]) -> Option<[i32; 2]> {
        let reach = self.columns.max(self.rows);
        (0..reach).find_map(|ring| {
            (-ring..=ring)
                .flat_map(|offset| {
                    [
                        [from[0] + offset, from[1] - ring],
                        [from[0] + offset, from[1] + ring],
                        [from[0] - ring, from[1] + offset],
                        [from[0] + ring, from[1] + offset],
                    ]
                })
                .find(|at| self.is_footing(at[0], at[1]))
        })
    }

    /// Somewhere to start, and somewhere worth crossing to.
    ///
    /// The goal is put a fair way off across the diagonal and then nudged to
    /// the nearest footing, so what lies between them is whatever the world
    /// put there -- a bay, a ridge, a swamp. That is the game: the obstacle is
    /// not designed, it is found.
    #[must_use]
    pub fn landfall(self) -> Landfall {
        let middle = [self.columns / 2, self.rows / 2];
        let start = self.nearest_footing(middle).unwrap_or(middle);
        let away = [start[0] + self.columns / 5, start[1] - self.rows / 5];
        let goal = self.nearest_footing(away).unwrap_or(away);
        Landfall { start, goal }
    }
}

fn grow_trees(shape: WorldShape, cells: &mut Vec<TileCellDocument>) {
    for row in (3..shape.rows - 3).step_by(6) {
    for column in (3..shape.columns - 3).step_by(6) {
        let x = column
            + if hashed(shape.seed ^ 0xF1, column, row) > 0.5 {
                1
            } else {
                -1
            };
        let y = row
            + if hashed(shape.seed ^ 0xF2, column, row) > 0.5 {
                1
            } else {
                -1
            };
        let described = shape.column(x, y);
        let surface = shape.surface(x, y, described);
        if described.ground <= SEA
            || !matches!(surface, "ground" | "moss")
            || hashed(shape.seed ^ 0xF3, x, y) < 0.58
        {
            continue;
        }
    
        let base = described.ground + 1;
        for level in base..base + 4 {
            cells.push(TileCellDocument {
                position: [x, y, level],
                tile: "log".to_owned(),
                visual_override: None,
            });
        }
        // Chunky broadleaf crown, asymmetric enough not to become a green
        // cube while remaining readable from each 90-degree camera view.
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                cells.push(TileCellDocument {
                    position: [x + dx, y + dy, base + 3],
                    tile: "leaves".to_owned(),
                    visual_override: None,
                });
            }
        }
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            cells.push(TileCellDocument {
                position: [x + dx, y + dy, base + 4],
                tile: "leaves".to_owned(),
                visual_override: None,
            });
        }
        cells.push(TileCellDocument {
            position: [x, y, base + 5],
            tile: "leaves".to_owned(),
            visual_override: None,
        });
    }
    }
    }

#[cfg(test)]
mod tests {
    use super::past;

    const SEED: u64 = 0x5E11_A1D2_0C4F_1907;

    #[test]
    fn a_column_well_clear_of_a_threshold_is_not_in_doubt() {
        // Fraying is a thing that happens at an edge. Away from one the answer
        // has to be the plain comparison, or a snow line becomes snow weather.
        for x in 0..40 {
            for y in 0..40 {
                assert!(past(SEED, x, y, 0.9, 0.5, 0.05), "well past is past");
                assert!(!past(SEED, x, y, 0.1, 0.5, 0.05), "well short is short");
            }
        }
    }

    #[test]
    fn a_column_inside_the_band_is_decided_either_way() {
        // The whole point: on the threshold itself the answer is not one thing
        // everywhere, because that is what draws a line.
        let answers: Vec<bool> = (0..60).map(|x| past(SEED, x, 0, 0.5, 0.5, 0.05)).collect();
        assert!(
            answers.contains(&true) && answers.contains(&false),
            "a run along the threshold goes both ways: {answers:?}"
        );
    }

    #[test]
    fn a_frayed_edge_is_tongues_rather_than_speckle() {
        // Deciding each column on its own gives salt and pepper. The field is
        // sampled at a few cells' scale so that a column mostly agrees with
        // the one beside it, and the edge comes out as tongues and bays.
        let answers: Vec<bool> = (0..200).map(|x| past(SEED, x, 0, 0.5, 0.5, 0.05)).collect();
        let flips = answers.windows(2).filter(|pair| pair[0] != pair[1]).count();
        // White noise on a coin flip would turn over about half the time.
        // Sampling a field a couple of cells wide turns over far less often.
        assert!(
            flips < answers.len() / 4,
            "neighbouring columns mostly agree: {flips} changes in {} columns, \
             which is speckle rather than a boundary",
            answers.len()
        );
    }

    #[test]
    fn generated_world_contains_block_built_trees() {
        let volume = super::generate(super::WorldShape::default(), "causeway.tileset.json");
        let logs = volume
            .cells
            .iter()
            .filter(|cell| cell.tile == "log")
            .count();
        let leaves = volume
            .cells
            .iter()
            .filter(|cell| cell.tile == "leaves")
            .count();
        assert!(logs > 0, "the generated world grows log trunks");
        assert!(
            leaves > logs,
            "tree crowns contain more leaves than trunk blocks"
        );
    }

    #[test]
    fn the_same_column_is_always_decided_the_same_way() {
        // A world that reshuffled its own coastline between two runs would
        // make every render capture a coin toss.
        for x in 0..50 {
            let once = past(SEED, x, 7, 0.5, 0.5, 0.05);
            assert_eq!(once, past(SEED, x, 7, 0.5, 0.5, 0.05));
        }
    }
}
