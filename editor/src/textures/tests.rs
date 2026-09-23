//! Where a manifest is looked for, relative to a scene.

use std::collections::BTreeSet;

use sindri_scene::{PROCEDURAL_TEXTURES, TileSetBindings};

use super::*;

/// The rule that keeps the two kinds of texture reference apart without
/// anybody having to remember it.
#[test]
fn a_procedural_reference_cannot_be_mistaken_for_a_file() {
    for procedural in PROCEDURAL_TEXTURES {
        assert!(
            AssetId::new(procedural.reference).is_err(),
            "{} would be asked of the filesystem",
            procedural.reference
        );
    }
    assert!(AssetId::new("textures/badge.png").is_ok());
}

/// A scene's directory is where its references resolve, which is what makes
/// the same scene file work from wherever it is checked out.
#[test]
fn references_resolve_against_the_scene_s_own_directory() {
    assert_eq!(
        root_of(Some(Path::new("game/levels/one.scene.json"))),
        Some(PathBuf::from("game/levels"))
    );
    assert_eq!(root_of(None), None);
}

/// The island drew as a comb of vertical stripes the moment anything was
/// painted on it.
///
/// A tile volume names a tile *set*, not a texture, so nothing the world says
/// asks for the sheet that cuts the blocks. The tile set's arrival requested
/// it; the next pass over what the scene references — which runs on every edit
/// — found nothing claiming it and released it. Unbinding a sheet does not
/// blank the texture, it unbinds the slicing, so all forty-eight blocks
/// resolved into every cell at once.
#[test]
fn a_tile_volume_keeps_the_sheet_that_cuts_its_blocks() {
    let document = sindri_core::SceneDocument::from_json(
        r#"{ "format_version": 10, "entities": [
                 { "id": "floor", "transform_3d": {}, "components": {
                   "sindri.tile_grid": {
                     "columns": 2, "rows": 2, "cell_size": [1.0, 0.5],
                     "projection": "isometric"
                   },
                   "sindri.tile_volume": {
                     "tileset": "blocks.tileset.json",
                     "cells": [{ "position": [0, 0, 0], "tile": "grass" }]
                   }
                 } }
               ] }"#,
    )
    .expect("the scene parses");
    let extractor = sindri_scene::SceneExtractor::new().expect("the schemas register");
    let world = crate::native::load_world(&extractor, &document).expect("the world builds");

    let mut tile_sets = TileSetBindings::new();
    tile_sets
        .bind(
            "blocks.tileset.json",
            sindri_core::TileSetDocument::from_json(
                r#"{ "format_version": 1, "tiles": { "grass": { "faces": {
                     "top": { "sprite": "textures/blocks.png#grass-top",
                              "size": [1.0, 1.0] }
                   } } } }"#,
            )
            .expect("the tile set parses"),
        )
        .expect("the tile set binds");

    let wanted_tile_sets: BTreeSet<AssetId> = ["blocks.tileset.json"]
        .into_iter()
        .map(|id| AssetId::new(id.to_owned()).expect("a loadable id"))
        .collect();

    let sheets = super::request::wanted_sheets(&world, &tile_sets, &wanted_tile_sets);
    let blocks = AssetId::new("textures/blocks.sheet.json".to_owned()).expect("a loadable id");
    assert!(
        sheets.contains(&blocks),
        "the sheet cutting the volume's blocks was not wanted, so the next \
         pass would release it: {sheets:?}"
    );
}
