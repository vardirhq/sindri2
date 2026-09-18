//! What the generator makes: a world with somewhere to go in it.
//!
//! Stated as proportions rather than as exact cells, because the point of a
//! generated world is that nobody placed any of it. What can be asserted is
//! that it is a *world*: it has sea and land, the land rises far enough to
//! need climbing, and no biome has eaten the map.

use std::collections::BTreeMap;

use sindri_causeway::worldgen::{SEA, WorldShape, generate};

fn surfaces(shape: WorldShape) -> BTreeMap<String, usize> {
    let volume = generate(shape, "causeway.tileset.json");
    // The top cell of each column is the one you see and walk on.
    let mut tops: BTreeMap<(i32, i32), (i32, String)> = BTreeMap::new();
    for cell in &volume.cells {
        let [column, row, level] = cell.position;
        let entry = tops
            .entry((column, row))
            .or_insert((i32::MIN, String::new()));
        if level > entry.0 {
            *entry = (level, cell.tile.clone());
        }
    }
    let mut counts = BTreeMap::new();
    for (_, tile) in tops.into_values() {
        *counts.entry(tile).or_insert(0) += 1;
    }
    counts
}

#[test]
fn the_world_has_sea_and_land_and_neither_has_eaten_the_other() {
    let shape = WorldShape::default();
    let counts = surfaces(shape);
    let total: usize = counts.values().sum();
    // Proportions rather than counts: what matters is that no biome has eaten
    // the map, which is a question about the whole of it.
    #[allow(clippy::cast_precision_loss)]
    let fraction = |name: &str| counts.get(name).copied().unwrap_or(0) as f64 / total as f64;

    assert!(
        (0.05..0.45).contains(&fraction("water")),
        "a world is neither a pond nor a continent: {counts:?}"
    );
    let land = 1.0 - fraction("water");
    assert!(land > 0.5, "most of it should be walkable: {counts:?}");
    assert!(
        fraction("ground") > 0.25,
        "grass is what most of the land is: {counts:?}"
    );
    assert!(
        fraction("sand") < 0.25,
        "a beach is the strip the sea reaches, not the map: {counts:?}"
    );
}

#[test]
fn every_biome_the_palette_names_actually_appears() {
    // A rule nothing satisfies is a rule nobody will notice is broken. Each of
    // these is a place the generator claims to make.
    let counts = surfaces(WorldShape::default());
    for biome in ["water", "sand", "ground", "mud", "snow", "rock", "gravel"] {
        assert!(
            counts.get(biome).copied().unwrap_or(0) > 40,
            "the world should have some {biome} in it: {counts:?}"
        );
    }
}

#[test]
fn the_land_rises_far_enough_to_be_worth_climbing() {
    let volume = generate(WorldShape::default(), "causeway.tileset.json");
    let highest = volume
        .cells
        .iter()
        .map(|cell| cell.position[2])
        .max()
        .expect("the world has cells");
    assert!(
        highest >= SEA + 12,
        "somewhere should be a mountain, not a hill: highest is {highest}"
    );
}

#[test]
fn the_same_seed_builds_the_same_world_twice() {
    // A column's ground is a property of where it is, not of the order columns
    // were asked about. Without that, saving a world means saving every cell.
    let shape = WorldShape::default();
    assert_eq!(
        generate(shape, "t").cells.len(),
        generate(shape, "t").cells.len()
    );
    assert_eq!(generate(shape, "t").cells, generate(shape, "t").cells);
}
