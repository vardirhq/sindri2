//! Bounded pieces of a deterministic Causeway world.

use sindri_scene::{TILE_CHUNK_SIZE, TileCellDocument, TileChunkCoord};

use super::{MAX_SKIRT, SEA, WorldShape, hashed};

/// Generates exactly one engine-sized chunk.
///
/// Heights and biome fields are sampled in world coordinates, including the
/// one-column halo used to form cliff skirts. Generating adjacent chunks in a
/// different order therefore produces the same seam.
#[must_use]
pub fn generate_chunk(shape: WorldShape, chunk: TileChunkCoord) -> Vec<TileCellDocument> {
    let min_x = chunk.min_column().max(0);
    let min_y = chunk.min_row().max(0);
    let max_x = (chunk.min_column() + TILE_CHUNK_SIZE).min(shape.columns);
    let max_y = (chunk.min_row() + TILE_CHUNK_SIZE).min(shape.rows);
    if min_x >= max_x || min_y >= max_y {
        return Vec::new();
    }

    let mut cells = Vec::new();
    for row in min_y..max_y {
        for column in min_x..max_x {
            let described = shape.column(column, row);
            let mut push = |level: i32, tile: &str| {
                cells.push(TileCellDocument {
                    position: [column, row, level],
                    tile: tile.to_owned(),
                    visual_override: None,
                });
            };
            if described.ground < SEA {
                push(SEA - 1, shape.bed(column, row, described));
                push(SEA, "water");
                continue;
            }

            let surface = shape.surface(column, row, described);
            push(described.ground, surface);
            let lowest = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .filter_map(|(dx, dy)| {
                    let (x, y) = (column + dx, row + dy);
                    (x >= 0 && y >= 0 && x < shape.columns && y < shape.rows)
                        .then(|| shape.column(x, y).ground)
                })
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
    grow_trees_in(shape, chunk, &mut cells);
    cells
}

fn first_lattice_at_or_after(value: i32) -> i32 {
    let mut first = 3 + (value - 3).div_euclid(6) * 6;
    if first < value {
        first += 6;
    }
    first
}

fn grow_trees_in(shape: WorldShape, chunk: TileChunkCoord, cells: &mut Vec<TileCellDocument>) {
    // A crown reaches one column beyond its trunk, so anchors are considered
    // through a one-cell halo and each emitted block is clipped back to this
    // chunk. No chunk owns a tree; every block has exactly one owning chunk.
    let min_x = chunk.min_column();
    let min_y = chunk.min_row();
    let max_x = min_x + TILE_CHUNK_SIZE;
    let max_y = min_y + TILE_CHUNK_SIZE;
    let anchor_min_x = first_lattice_at_or_after((min_x - 2).max(3));
    let anchor_min_y = first_lattice_at_or_after((min_y - 2).max(3));
    let anchor_max_x = (max_x + 2).min(shape.columns - 3);
    let anchor_max_y = (max_y + 2).min(shape.rows - 3);

    for row in (anchor_min_y..anchor_max_y).step_by(6) {
        for column in (anchor_min_x..anchor_max_x).step_by(6) {
            let (sample_column, sample_row) = shape.sample(column, row);
            let x = column
                + if hashed(shape.seed ^ 0xF1, sample_column, sample_row) > 0.5 {
                    1
                } else {
                    -1
                };
            let y = row
                + if hashed(shape.seed ^ 0xF2, sample_column, sample_row) > 0.5 {
                    1
                } else {
                    -1
                };
            let described = shape.column(x, y);
            let surface = shape.surface(x, y, described);
            if described.ground <= SEA || !matches!(surface, "ground" | "moss") || {
                let (sample_x, sample_y) = shape.sample(x, y);
                hashed(shape.seed ^ 0xF3, sample_x, sample_y) < 0.58
            } {
                continue;
            }

            let base = described.ground + 1;
            let mut push = |at_x: i32, at_y: i32, level: i32, tile: &str| {
                if chunk.contains(at_x, at_y) {
                    cells.push(TileCellDocument {
                        position: [at_x, at_y, level],
                        tile: tile.to_owned(),
                        visual_override: None,
                    });
                }
            };
            for level in base..base + 4 {
                push(x, y, level, "log");
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        push(x + dx, y + dy, base + 3, "leaves");
                    }
                }
            }
            for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                push(x + dx, y + dy, base + 4, "leaves");
            }
            push(x, y, base + 5, "leaves");
        }
    }
}
