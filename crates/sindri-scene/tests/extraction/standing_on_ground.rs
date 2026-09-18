//! What the ground does to something standing on it.
//!
//! Depth alone used to decide this, and it put the ground in front of the
//! player standing on it: the column ahead of a walker is nearer than the one
//! under them, so its top face -- flat, at exactly the height of their feet --
//! won on depth and drew over their legs.
//!
//! Terrain is not what changed. Cells still sort against each other by the
//! depth of the column they stand in, because that is right and the tests
//! beside this file say so. What changed is that a walker sorts a cell forward,
//! past ground no higher than its feet, since no face of such a cell -- neither
//! its top nor the walls holding that top up -- can cover it. A step ahead that
//! *is* higher takes that back, because then it is a wall and covering the
//! walker is its job.
//!
//! The cells asked about are the ones a step ahead. A diagonal neighbour is two
//! steps of depth away and its top never rises far enough to reach the sprite,
//! so it is left where it is rather than dragged into the rule.

use sindri_core::{TileSetDocument, World};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{
    CameraView, GridSurfaces, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings,
    resolve_grid_placements,
};

use crate::support::{VIEWPORT, document, world_from};

/// Two tiles that differ only in where their art comes from, so a test can say
/// which cell drew which face.
fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": {
             "grass": { "faces": {
               "top": { "sprite": "blocks.png#0", "size": [1.0, 0.5] },
               "south": { "sprite": "blocks.png#1", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
             } },
             "marked": { "faces": {
               "top": { "sprite": "marked.png#0", "size": [1.0, 0.5] },
               "south": { "sprite": "marked.png#1", "size": [1.0, 0.5], "offset": [0.0, -0.25] }
             } }
           } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("blocks.png", TextureId::new(9));
    bindings.bind("marked.png", TextureId::new(11));
    bindings.bind("walker.png", TextureId::new(10));
    for sheet in ["blocks.png", "marked.png"] {
        bindings
            .bind_sheet(sheet, &sindri_core::SpriteSheetDocument::from_grid(2, 1))
            .unwrap();
    }
    bindings
}

const WALKER: TextureId = TextureId::new(10);
const MARKED: TextureId = TextureId::new(11);

/// A flat three-by-three floor with a walker on the middle of it.
///
/// `marked` names the cells drawn from the second texture, and `extra` adds
/// cells above the floor. Everything else is plain ground at level zero.
fn scene_with(marked: &[[i32; 2]], extra: &str) -> World {
    let floor = (0..3)
        .flat_map(|row| {
            (0..3).map(move |column| {
                let tile = if marked.contains(&[column, row]) {
                    "marked"
                } else {
                    "grass"
                };
                format!(r#"{{ "position": [{column}, {row}, 0], "tile": "{tile}" }}"#)
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    world_from(&document(&format!(
        r#"
        {{ "id": "main-camera", "transform_3d": {{ "position": [0.0, 0.0, 10.0] }},
           "components": {{ "sindri.camera": {{
             "projection": "orthographic", "vertical_size": 8.0,
             "near": 0.1, "far": 100.0 }} }} }},
        {{ "id": "floor", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 3, "rows": 3, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric", "depth_step": 0.01
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "cells": [{floor}{extra}]
          }}
        }} }},
        {{ "id": "walker", "transform_3d": {{}}, "components": {{
          "sindri.grid.placement": {{ "grid": "floor", "cell": [1, 1] }},
          "sindri.sprite": {{ "texture": "walker.png", "tint": [1.0, 1.0, 1.0, 1.0] }}
        }} }}"#
    )))
}

/// Which texture drew each sprite instance, in the order they are drawn.
fn draw_order(marked: &[[i32; 2]], extra: &str) -> Vec<TextureId> {
    let mut world = scene_with(marked, extra);
    let extractor = SceneExtractor::new().expect("the schemas register");
    let sets = tile_sets();
    resolve_grid_placements(
        &mut world,
        extractor.components(),
        Some(&sets),
        &mut GridSurfaces::default(),
    )
    .expect("the walker is placed");
    extractor
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &textures(),
            SceneRuntime::default().with_tile_sets(&sets),
        )
        .expect("the scene extracts")
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

/// Where the walker is drawn, and where the marked cells are.
fn walker_and_marked(order: &[TextureId]) -> (usize, Vec<usize>) {
    let walker = order
        .iter()
        .position(|texture| *texture == WALKER)
        .expect("the walker is drawn");
    let marked = order
        .iter()
        .enumerate()
        .filter(|(_, texture)| **texture == MARKED)
        .map(|(index, _)| index)
        .collect();
    (walker, marked)
}

/// The reported bug, as a test: the tile in front of the player drew over them.
///
/// `(2, 1)` and `(1, 2)` are the two cells one step of depth ahead of the
/// walker on `(1, 1)` -- the ones whose top faces reach its sprite.
#[test]
fn ground_a_step_ahead_never_draws_over_what_stands_on_it() {
    let order = draw_order(&[[2, 1], [1, 2]], "");
    let (walker, marked) = walker_and_marked(&order);
    assert!(!marked.is_empty(), "the marked cells are drawn: {order:?}");
    assert!(
        marked.iter().all(|index| *index < walker),
        "flat ground a step ahead of the walker belongs behind it, and every \
         face of those cells is at or below its feet: walker at {walker}, \
         marked at {marked:?} in {order:?}"
    );
}

/// The other half of the rule, and the reason it is not a layer.
///
/// A block raised a step ahead is a wall rather than ground, so the walker
/// keeps its own depth and the block covers it.
#[test]
fn a_block_raised_a_step_ahead_still_covers_the_walker() {
    let order = draw_order(&[], r#", { "position": [1, 2, 1], "tile": "marked" }"#);
    let (walker, marked) = walker_and_marked(&order);
    assert!(
        marked.iter().any(|index| *index > walker),
        "a block raised a step ahead of the walker draws after it: walker at \
         {walker}, marked at {marked:?} in {order:?}"
    );
}

/// And a block raised behind them does not.
#[test]
fn a_block_raised_behind_does_not_cover_the_walker() {
    let order = draw_order(&[], r#", { "position": [1, 0, 1], "tile": "marked" }"#);
    let (walker, marked) = walker_and_marked(&order);
    assert!(
        marked.iter().all(|index| *index < walker),
        "a block raised behind the walker stays behind it: walker at {walker}, \
         marked at {marked:?} in {order:?}"
    );
}
