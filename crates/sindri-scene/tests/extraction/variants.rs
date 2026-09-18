//! One tile ID with several looks, chosen by where the cell is.
//!
//! What this replaces is an author scattering `grass`, `grass-b` and
//! `grass-c` by hand: five hundred cells that all mean the same thing, stored
//! as five hundred decisions somebody had to make and can never change
//! without making them again.
//!
//! The choice is a hash of the cell, the tile's name and the volume's seed, so
//! it is the same on every machine and after every reload. That matters more
//! than it sounds: a render capture in CI compares pixels, and variation that
//! moved between runs would fail it for no reason anybody could act on.

use sindri_core::{SpriteSheetDocument, TileSetDocument};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings};

use crate::support::{VIEWPORT, document, world_from};

/// One tile, three looks, each from its own texture so a test can tell them
/// apart by which draw call they landed in.
fn tile_sets(weights: [u32; 3]) -> TileSetBindings {
    let document = TileSetDocument::from_json(&format!(
        r#"{{ "format_version": 1, "tiles": {{ "grass": {{
             "weight": {},
             "faces": {{ "top": {{ "sprite": "plain.png#0", "size": [1.0, 0.5] }} }},
             "variants": [
               {{ "weight": {}, "faces": {{ "top": {{ "sprite": "tuft.png#0", "size": [1.0, 0.5] }} }} }},
               {{ "weight": {}, "faces": {{ "top": {{ "sprite": "bloom.png#0", "size": [1.0, 0.5] }} }} }}
             ]
           }} }} }}"#,
        weights[0], weights[1], weights[2]
    ))
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

const PLAIN: TextureId = TextureId::new(1);
const TUFT: TextureId = TextureId::new(2);
const BLOOM: TextureId = TextureId::new(3);

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    for (name, id) in [
        ("plain.png", PLAIN),
        ("tuft.png", TUFT),
        ("bloom.png", BLOOM),
    ] {
        bindings.bind(name, id);
        bindings
            .bind_sheet(name, &SpriteSheetDocument::from_grid(1, 1))
            .unwrap();
    }
    bindings
}

/// An eight-by-eight field of one tile, so there is enough of it to vary.
fn field(seed: u64) -> sindri_core::World {
    let cells = (0..8)
        .flat_map(|row| {
            (0..8).map(move |column| {
                format!(r#"{{ "position": [{column}, {row}, 0], "tile": "grass" }}"#)
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    world_from(&document(&format!(
        r#"
        {{ "id": "main-camera", "transform_3d": {{ "position": [0.0, 0.0, 10.0] }},
           "components": {{ "sindri.camera": {{
             "projection": "orthographic", "vertical_size": 40.0,
             "near": 0.1, "far": 100.0 }} }} }},
        {{ "id": "floor", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 8, "rows": 8, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "variant_seed": {seed},
            "cells": [{cells}]
          }}
        }} }}"#
    )))
}

/// Which texture drew each face, in draw order.
fn drawn(seed: u64, weights: [u32; 3]) -> Vec<TextureId> {
    let sets = tile_sets(weights);
    SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &field(seed),
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(&sets),
        )
        .expect("the field extracts")
        .passes()
        .iter()
        .flat_map(|pass| match &pass.command {
            FrameCommand::SpriteBatch {
                texture, instances, ..
            } => vec![*texture; instances.len()],
            _ => Vec::new(),
        })
        .collect()
}

fn counts(drawn: &[TextureId]) -> [usize; 3] {
    [PLAIN, TUFT, BLOOM].map(|id| drawn.iter().filter(|drawn| **drawn == id).count())
}

#[test]
fn one_tile_id_draws_several_looks() {
    let counts = counts(&drawn(0, [1, 1, 1]));
    assert_eq!(counts.iter().sum::<usize>(), 64, "every cell drew its top");
    assert!(
        counts.iter().all(|count| *count > 0),
        "all three looks should appear across sixty-four cells: {counts:?}"
    );
}

/// The property the whole thing rests on.
#[test]
fn the_same_field_looks_the_same_twice() {
    assert_eq!(drawn(0, [1, 1, 1]), drawn(0, [1, 1, 1]));
}

/// A seed is how an author rerolls a whole volume at once.
#[test]
fn a_different_seed_rearranges_the_field() {
    assert_ne!(
        drawn(0, [1, 1, 1]),
        drawn(9, [1, 1, 1]),
        "a reroll should change which cell got which look"
    );
}

/// Weights are proportions, and a heavy tile should dominate.
#[test]
fn weights_decide_how_often_a_look_appears() {
    let counts = counts(&drawn(0, [14, 1, 1]));
    assert!(
        counts[0] > counts[1] + counts[2],
        "fourteen to one to one should be mostly plain: {counts:?}"
    );
}

/// A tile that declares no variants is every tile written before this existed.
#[test]
fn a_tile_without_variants_draws_its_own_faces() {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": { "grass": {
             "faces": { "top": { "sprite": "plain.png#0", "size": [1.0, 0.5] } }
           } } }"#,
    )
    .expect("the tile set decodes");
    let definition = document.tile("grass").expect("the tile is there");
    for coord in [[0, 0, 0], [3, 5, 1], [-2, 7, 0]] {
        assert!(
            definition
                .faces_at(0, coord, "grass")
                .get(sindri_core::TileFace::Top)
                == definition.faces.get(sindri_core::TileFace::Top),
            "no variants means its own faces, wherever it is"
        );
    }
}

/// A variant's sprites have to be loaded whether or not this scene chose them.
#[test]
fn every_looks_texture_is_collected() {
    let sets = tile_sets([1, 1, 1]);
    let document = sets.get("world.tileset.json").expect("it is bound");
    let textures = sindri_scene::tile_set_textures(document);
    assert!(
        textures.contains("plain.png")
            && textures.contains("tuft.png")
            && textures.contains("bloom.png"),
        "a variant nobody happened to choose still needs its texture: {textures:?}"
    );
}
