use std::collections::BTreeSet;

use super::*;
use crate::{LocalVoxelCoord, SECTION_EDGE};

const GRASS: VoxelId = VoxelId::new(1);
const EARTH: VoxelId = VoxelId::new(2);
const STONE: VoxelId = VoxelId::new(3);
const SAND: VoxelId = VoxelId::new(4);
const ROCK: VoxelId = VoxelId::new(5);
const SNOW: VoxelId = VoxelId::new(6);
const WATER: VoxelId = VoxelId::new(12);
const LOG: VoxelId = VoxelId::new(13);
const LEAVES: VoxelId = VoxelId::new(14);

fn biome(temperature: f32, moisture: f32, surface: VoxelId, trees: f32) -> TerrainBiome {
    TerrainBiome {
        temperature,
        moisture,
        surface,
        subsurface: EARTH,
        subsurface_depth: 3,
        trees,
        relief: 1.0,
        terraces: 0,
    }
}

fn settings() -> NaturalTerrainSettings {
    NaturalTerrainSettings {
        seed: 0x00C0_FFEE,
        sea_level: 0,
        relief: 40,
        feature_size: 64,
        tree_line: 22,
        snow_line: 30,
        caves: true,
        rivers: true,
        palette: TerrainPalette {
            stone: STONE,
            water: Some(WATER),
            beach: Some(SAND),
            sea_bed: Some(SAND),
            cliff: Some(ROCK),
            snow: Some(SNOW),
            ice: None,
            trunk: Some(LOG),
            leaves: Some(LEAVES),
        },
        biomes: vec![
            biome(0.5, 0.3, GRASS, 0.05),
            biome(0.5, 0.8, GRASS, 0.6),
            biome(0.9, 0.1, SAND, 0.0),
            biome(0.1, 0.4, SNOW, 0.1),
        ],
    }
}

fn terrain() -> NaturalTerrain {
    NaturalTerrain::new(settings())
}

/// Columns across a square this many voxels wide, every fourth one.
const SPAN: i32 = 512;

fn columns() -> impl Iterator<Item = (i32, i32)> {
    (-SPAN / 2..SPAN / 2)
        .step_by(4)
        .flat_map(|z| (-SPAN / 2..SPAN / 2).step_by(4).map(move |x| (x, z)))
}

#[test]
fn a_section_is_the_same_whether_generated_whole_or_asked_about_a_voxel_at_a_time() {
    // The mesher mixes the two: resident sections come whole, and the voxels
    // just outside them one at a time. Any disagreement is a seam.
    let terrain = terrain();
    for section in [
        SectionCoord::new(0, 0, 0),
        SectionCoord::new(-3, -1, 2),
        SectionCoord::new(5, 1, -4),
        SectionCoord::new(-7, 0, -7),
    ] {
        let whole = terrain.generate_section(section);
        let min = section.min_voxel();
        for y in 0..SECTION_EDGE {
            for z in 0..SECTION_EDGE {
                for x in 0..SECTION_EDGE {
                    let coord = VoxelCoord::new(min.x + x, min.y + y, min.z + z);
                    assert_eq!(
                        whole.get(LocalVoxelCoord::new(
                            u8::try_from(x).unwrap(),
                            u8::try_from(y).unwrap(),
                            u8::try_from(z).unwrap()
                        )),
                        terrain.voxel(coord),
                        "{coord:?} in {section:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_same_settings_always_make_the_same_world() {
    let (once, again) = (terrain(), terrain());
    let section = SectionCoord::new(2, 0, -1);
    assert_eq!(
        once.generate_section(section),
        again.generate_section(section)
    );
    assert_eq!(once.clone(), once);
    let mut reseeded = settings();
    reseeded.seed ^= 1;
    let other = NaturalTerrain::new(reseeded);
    let differs = columns().any(|(x, z)| once.ground(x, z) != other.ground(x, z));
    assert!(differs, "another seed is another world");
}

#[test]
fn the_world_has_sea_lowland_and_mountains() {
    let terrain = terrain();
    let grounds: Vec<i32> = columns().map(|(x, z)| terrain.ground(x, z)).collect();
    let (lowest, highest) = (
        *grounds.iter().min().unwrap(),
        *grounds.iter().max().unwrap(),
    );
    let sea = grounds.iter().filter(|ground| **ground < 0).count();
    assert!(sea > grounds.len() / 20, "some of it is under water: {sea}");
    assert!(sea < grounds.len() * 3 / 4, "most of it is not: {sea}");
    assert!(
        highest >= 24,
        "ranges rise well above the plains: {highest}"
    );
    assert!(lowest <= -6, "the sea has depth: {lowest}");
}

#[test]
fn every_biome_is_somewhere() {
    let terrain = terrain();
    let seen: BTreeSet<usize> = columns().map(|(x, z)| terrain.biome(x, z)).collect();
    assert_eq!(seen.len(), settings().biomes.len(), "{seen:?}");
}

#[test]
fn biome_edges_fray_rather_than_run_along_a_line() {
    // Two neighbouring columns mostly share a biome, but not always along a
    // run: an edge is tongues and bays, not a speckle and not a ruler line.
    let terrain = terrain();
    let run: Vec<usize> = (0..SPAN).map(|x| terrain.biome(x, 17)).collect();
    let changes = run.windows(2).filter(|pair| pair[0] != pair[1]).count();
    assert!(changes > 0, "a long run crosses at least one edge");
    assert!(changes < run.len() / 8, "{changes} changes is speckle");
}

#[test]
fn water_fills_the_sea_and_nothing_above_it() {
    let terrain = terrain();
    let (x, z) = columns()
        .find(|(x, z)| terrain.ground(*x, *z) < -3)
        .expect("somewhere is under water");
    assert_eq!(terrain.voxel(VoxelCoord::new(x, 0, z)), WATER);
    assert_eq!(terrain.voxel(VoxelCoord::new(x, -1, z)), WATER);
    assert_eq!(terrain.voxel(VoxelCoord::new(x, 1, z)), VoxelId::AIR);
}

#[test]
fn caves_run_through_the_rock() {
    let terrain = terrain();
    let mut hollow = 0;
    let mut solid = 0;
    for (x, z) in columns().step_by(3) {
        let ground = terrain.ground(x, z);
        for y in (ground - 40)..(ground - 6) {
            if terrain.voxel(VoxelCoord::new(x, y, z)).is_air() {
                hollow += 1;
            } else {
                solid += 1;
            }
        }
    }
    let share = f64::from(hollow) / f64::from(hollow + solid);
    assert!(share > 0.002, "caves are there to find: {share}");
    assert!(
        share < 0.2,
        "and the ground is still mostly ground: {share}"
    );
}

#[test]
fn trees_stand_on_their_biome_and_their_crowns_are_above_their_trunks() {
    let terrain = terrain();
    let mut trunks = 0;
    for (x, z) in columns() {
        let ground = terrain.ground(x, z);
        let above = terrain.voxel(VoxelCoord::new(x, ground + 1, z));
        if above == LOG {
            trunks += 1;
            let top = (ground + 1..ground + 12)
                .take_while(|y| terrain.voxel(VoxelCoord::new(x, *y, z)) == LOG)
                .last()
                .expect("a trunk has a top");
            assert_eq!(
                terrain.voxel(VoxelCoord::new(x, top + 1, z)),
                LEAVES,
                "a trunk at ({x}, {z}) ends in leaves"
            );
        }
    }
    assert!(trunks > 3, "the forests have trees: {trunks}");
}

#[test]
fn a_world_without_optional_voxels_has_no_water_and_no_trees() {
    let mut bare = settings();
    bare.palette = TerrainPalette {
        stone: STONE,
        water: None,
        beach: None,
        sea_bed: None,
        cliff: None,
        snow: None,
        ice: None,
        trunk: None,
        leaves: None,
    };
    let terrain = NaturalTerrain::new(bare);
    let used: BTreeSet<u16> = [
        SectionCoord::new(0, 0, 0),
        SectionCoord::new(0, -1, 0),
        SectionCoord::new(4, 0, 4),
        SectionCoord::new(-6, -1, 3),
    ]
    .into_iter()
    .flat_map(|section| {
        let generated = terrain.generate_section(section);
        let mut values = Vec::new();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    values.push(generated.get(LocalVoxelCoord::new(x, y, z)).value());
                }
            }
        }
        values
    })
    .collect();
    // Sand is left out: it is the desert biome's own ground.
    for voxel in [WATER, LOG, LEAVES, ROCK] {
        assert!(
            !used.contains(&voxel.value()),
            "{voxel:?} was not asked for"
        );
    }
}

#[test]
fn a_world_without_biomes_is_stone() {
    let mut plain = settings();
    plain.biomes.clear();
    plain.palette.beach = None;
    plain.palette.snow = None;
    plain.palette.cliff = None;
    let terrain = NaturalTerrain::new(plain);
    let (x, z) = columns()
        .find(|(x, z)| terrain.ground(*x, *z) > 3)
        .expect("somewhere is land");
    assert_eq!(
        terrain.voxel(VoxelCoord::new(x, terrain.ground(x, z), z)),
        STONE
    );
}

/// Whether a voxel is ground: not air, and not the sea.
fn is_ground(terrain: &NaturalTerrain, x: i32, y: i32, z: i32) -> bool {
    let voxel = terrain.voxel(VoxelCoord::new(x, y, z));
    !voxel.is_air() && voxel != WATER
}

/// A mountain undercut from both sides at once, or a tunnel through a ridge,
/// is a window you can see the sky through: a row of voxels straight across
/// the rock with every one of them empty.
#[test]
fn mountains_are_undercut_but_not_cut_through() {
    let terrain = terrain();
    let (mut rows, mut through) = (0, 0);
    for z in (-SPAN / 2..SPAN / 2).step_by(4) {
        for y in (10..60).step_by(2) {
            let mut x = -SPAN / 2;
            while x < SPAN / 2 {
                let start = x;
                let mut open = true;
                while x < SPAN / 2 && terrain.ground(x, z) >= y {
                    open &= !is_ground(&terrain, x, y, z);
                    x += 1;
                }
                if x > start && start > -SPAN / 2 && x < SPAN / 2 {
                    rows += 1;
                    through += i32::from(open);
                }
                x += 1;
            }
        }
    }
    let share = f64::from(through) / f64::from(rows);
    assert!(rows > 1_000, "the rows cross mountains: {rows}");
    assert!(share < 0.005, "{through} of {rows} rows see through");

    // Caves hollow the rock too; without them, what is left is the overhangs.
    let terrain = NaturalTerrain::new(NaturalTerrainSettings {
        caves: false,
        ..settings()
    });
    let undercut = columns()
        .filter(|&(x, z)| {
            let ground = terrain.ground(x, z);
            (ground - 8..ground)
                .any(|y| !is_ground(&terrain, x, y, z) && is_ground(&terrain, x, y + 1, z))
        })
        .count();
    assert!(undercut > 10, "ranges still overhang: {undercut}");
}
