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
    fn surface(column: Column) -> &'static str {
        let Column {
            ground,
            moisture,
            warmth,
        } = column;
        // Cold enough and high enough for the snow to lie.
        let snow_here = ground >= SNOW_LINE || (warmth < 0.22 && ground > SEA);
        if snow_here {
            return "snow";
        }
        if ground >= TREE_LINE {
            return "rock";
        }
        // The shore, and only the shore. A beach is the strip the sea reaches,
        // so it is one or two courses above the waterline and nothing else --
        // the first try made it every column within two levels of the sea,
        // which on a world this flat was most of the world, and the whole map
        // read as desert.
        if ground <= SEA + 1 {
            if moisture > 0.58 {
                // Low, flat and wet is a swamp rather than a beach.
                return "mud";
            }
            return "sand";
        }
        if moisture > 0.68 && ground <= SEA + 3 {
            return "moss";
        }
        if moisture < 0.26 && ground > SEA + 4 {
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
    fn bed(column: Column) -> &'static str {
        if column.warmth < 0.2 {
            "ice"
        } else if column.moisture > 0.66 {
            "mud"
        } else if column.ground <= SEA - 3 {
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
                push(SEA - 1, WorldShape::bed(described));
                push(SEA, "water");
                continue;
            }
            let surface = WorldShape::surface(described);
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
        described.ground >= SEA && !matches!(Self::surface(described), "snow" | "rock")
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
