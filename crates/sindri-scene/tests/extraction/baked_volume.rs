//! What a volume draws is resolved once and kept until something changes it.
//!
//! An island's faces do not move while the player walks around it, so they are
//! baked and replayed. That is only correct while every input to them is still
//! the one they were baked from, and each test here is one of those inputs
//! moving on: the cells, the grid, the transform, the tile set, the textures,
//! and whether the entity takes part in the scene at all. A stale cache draws
//! last frame's island, which is the kind of wrong nothing else would catch.

use sindri_core::{EntityId, SpriteSheetDocument, TileSetDocument, World};
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{CameraView, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings};

use crate::support::{VIEWPORT, document, world_camera};

/// A tile set whose faces are `size` across, so rebinding it visibly changes
/// every face the volume draws.
fn tile_sets(size: f32) -> TileSetBindings {
    let document = TileSetDocument::from_json(&format!(
        r#"{{
          "format_version": 1,
          "tiles": {{ "grass": {{ "faces": {{
            "top": {{ "sprite": "blocks.png#0", "size": [{size}, {size}] }}
          }} }} }}
        }}"#
    ))
    .expect("the tile set parses");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

/// Textures cutting `blocks.png` into `columns` across, so rebinding the sheet
/// changes the rect every face reads from.
fn textures(columns: u32) -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("blocks.png", TextureId::new(9));
    bindings
        .bind_sheet("blocks.png", &SpriteSheetDocument::from_grid(columns, 1))
        .unwrap();
    bindings
}

/// Two cells a clear distance apart, so a camera on the other side of them
/// draws them in the other order.
fn volume_world(cells: &str) -> World {
    crate::support::world_from(&document(&format!(
        r#"{}, {{ "id": "blocks", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "cells": [{cells}]
          }}
        }} }}"#,
        world_camera()
    )))
}

/// The same volume, hung under a parent that can be switched off.
fn parented_volume_world() -> World {
    crate::support::world_from(&document(&format!(
        r#"{}, {{ "id": "island", "transform_3d": {{}} }},
        {{ "id": "blocks", "parent": "island", "transform_3d": {{}}, "components": {{
          "sindri.tile_grid": {{
            "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric"
          }},
          "sindri.tile_volume": {{
            "tileset": "world.tileset.json",
            "cells": [{}]
          }}
        }} }}"#,
        world_camera(),
        TWO_CELLS
    )))
}

const TWO_CELLS: &str = r#"
    { "position": [0, 0, 0], "tile": "grass" },
    { "position": [3, 3, 0], "tile": "grass" }"#;

/// Everything the volume draws, described exactly, in the order drawn.
///
/// The debug form rather than the instances, because what matters is that two
/// frames agree down to the matrix -- and that two frames that should *not*
/// agree do not.
fn drawn(
    extractor: &SceneExtractor,
    world: &World,
    textures: &TextureBindings,
    tile_sets: &TileSetBindings,
) -> String {
    let frame = extractor
        .extract_animated(
            world,
            VIEWPORT,
            CameraView::default(),
            textures,
            SceneRuntime::default().with_tile_sets(tile_sets),
        )
        .expect("the volume extracts");
    let instances = frame
        .passes()
        .iter()
        .flat_map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances.clone(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    format!("{instances:?}")
}

fn entity_with(world: &World, component: &str) -> EntityId {
    world
        .entities()
        .find(|(_, data)| data.components.contains_key(component))
        .map(|(entity, _)| entity)
        .expect("the scene holds that component")
}

#[test]
fn a_replayed_volume_draws_what_it_drew_the_first_time() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let world = volume_world(TWO_CELLS);
    let first = drawn(&extractor, &world, &textures, &tile_sets);
    let second = drawn(&extractor, &world, &textures, &tile_sets);
    assert_eq!(first, second);
    // And what an extractor that has never seen it draws, so replaying is not
    // quietly its own answer.
    let fresh = SceneExtractor::new().unwrap();
    assert_eq!(first, drawn(&fresh, &world, &textures, &tile_sets));
}

#[test]
fn an_edited_volume_is_resolved_again() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let mut world = volume_world(TWO_CELLS);
    let before = drawn(&extractor, &world, &textures, &tile_sets);

    let entity = entity_with(&world, "sindri.tile_volume");
    let volume = world
        .get_mut(entity)
        .and_then(|data| data.components.get_mut("sindri.tile_volume"))
        .expect("the volume is there to edit");
    volume["cells"] = serde_json::json!([{ "position": [0, 0, 0], "tile": "grass" }]);

    let after = drawn(&extractor, &world, &textures, &tile_sets);
    assert_ne!(before, after, "a cell was removed and nothing changed");
    assert_eq!(
        after,
        drawn(
            &SceneExtractor::new().unwrap(),
            &world,
            &textures,
            &tile_sets
        )
    );
}

#[test]
fn a_moved_volume_is_resolved_again() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let mut world = volume_world(TWO_CELLS);
    let before = drawn(&extractor, &world, &textures, &tile_sets);

    let entity = entity_with(&world, "sindri.tile_volume");
    let data = world.get_mut(entity).expect("the volume is there to move");
    let mut transform = data.transform_3d.unwrap_or_default();
    transform.position[0] += 5.0;
    data.transform_3d = Some(transform);

    assert_ne!(
        before,
        drawn(&extractor, &world, &textures, &tile_sets),
        "the volume moved and its faces did not"
    );
}

#[test]
fn a_rebound_tile_set_is_resolved_again() {
    let extractor = SceneExtractor::new().unwrap();
    let world = volume_world(TWO_CELLS);
    let textures = textures(2);
    let before = drawn(&extractor, &world, &textures, &tile_sets(1.0));
    assert_ne!(
        before,
        drawn(&extractor, &world, &textures, &tile_sets(2.0)),
        "the tile set's faces changed size and the volume kept the old ones"
    );
}

#[test]
fn a_rebound_sheet_is_resolved_again() {
    let extractor = SceneExtractor::new().unwrap();
    let world = volume_world(TWO_CELLS);
    let tile_sets = tile_sets(1.0);
    let before = drawn(&extractor, &world, &textures(2), &tile_sets);
    assert_ne!(
        before,
        drawn(&extractor, &world, &textures(4), &tile_sets),
        "the sheet was re-cut and the volume kept the old rects"
    );
}

#[test]
fn a_switched_off_volume_stops_drawing() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let mut world = volume_world(TWO_CELLS);
    let before = drawn(&extractor, &world, &textures, &tile_sets);
    assert_ne!(before, "[]");

    let entity = entity_with(&world, "sindri.tile_volume");
    world
        .get_mut(entity)
        .expect("the volume is there to switch off")
        .disabled = true;

    assert_eq!(
        drawn(&extractor, &world, &textures, &tile_sets),
        "[]",
        "a switched-off volume went on drawing"
    );
}

#[test]
fn a_moved_camera_re_measures_a_kept_volume() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let mut world = volume_world(TWO_CELLS);
    let before = drawn(&extractor, &world, &textures, &tile_sets);

    // The volume itself does not change here, so it is replayed rather than
    // resolved again -- and its depths still have to be measured against where
    // the camera now looks, or the far cell is drawn in front of the near one.
    // Turned around rather than moved. Depth is measured along where the
    // camera looks, so sliding it sideways shifts every cell by the same amount
    // and reorders nothing -- only facing the other way does.
    let camera = entity_with(&world, "sindri.camera");
    let data = world.get_mut(camera).expect("the camera is there to turn");
    let mut transform = data.transform_3d.unwrap_or_default();
    transform.rotation = [0.0, 0.0, 1.0, 0.0];
    data.transform_3d = Some(transform);

    assert_ne!(
        before,
        drawn(&extractor, &world, &textures, &tile_sets),
        "the camera turned around and the draw order did not change"
    );
}

#[test]
fn a_volume_under_a_switched_off_parent_stops_drawing() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let mut world = parented_volume_world();
    assert_ne!(drawn(&extractor, &world, &textures, &tile_sets), "[]");

    // The case nothing else catches. Switching off the parent does not touch
    // the volume, so the volume is still exactly what it was baked from and is
    // replayed rather than resolved again -- and it must still stop drawing.
    // Whether an entity takes part in the scene is not a property of the
    // entity, and so is not something that can be baked into it.
    let island = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "island")
        })
        .map(|(entity, _)| entity)
        .expect("the parent is in the scene");
    world
        .get_mut(island)
        .expect("the parent is there to switch off")
        .disabled = true;

    assert_eq!(
        drawn(&extractor, &world, &textures, &tile_sets),
        "[]",
        "a volume under a switched-off parent went on drawing"
    );
}

#[test]
fn two_worlds_are_not_mistaken_for_each_other() {
    // One extractor, two scenes -- what the editor does when a second file is
    // opened, and what Gather does every frame, since presenting a world
    // clones it. Both worlds lay their entities out identically, so an entity
    // in one has the same handle as a different entity in the other, and a
    // revision that counted only within a world would match across them.
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));

    let one = volume_world(TWO_CELLS);
    let other = volume_world(r#"{ "position": [1, 1, 0], "tile": "grass" }"#);
    let drew_one = drawn(&extractor, &one, &textures, &tile_sets);
    let drew_other = drawn(&extractor, &other, &textures, &tile_sets);

    assert_ne!(
        drew_one, drew_other,
        "a second world was drawn as the first one"
    );
    assert_eq!(
        drew_other,
        drawn(
            &SceneExtractor::new().unwrap(),
            &other,
            &textures,
            &tile_sets
        ),
        "the second world was not drawn as itself"
    );
}

#[test]
fn a_clone_that_moves_on_is_not_drawn_as_its_original() {
    let extractor = SceneExtractor::new().unwrap();
    let (textures, tile_sets) = (textures(2), tile_sets(1.0));
    let world = volume_world(TWO_CELLS);
    let before = drawn(&extractor, &world, &textures, &tile_sets);

    // Gather presents its world by cloning it, so the clone and the original
    // hand out revisions from the same place.
    let mut clone = world.clone();
    let entity = entity_with(&clone, "sindri.tile_volume");
    let volume = clone
        .get_mut(entity)
        .and_then(|data| data.components.get_mut("sindri.tile_volume"))
        .expect("the clone has a volume to edit");
    volume["cells"] = serde_json::json!([{ "position": [2, 2, 0], "tile": "grass" }]);

    assert_ne!(
        before,
        drawn(&extractor, &clone, &textures, &tile_sets),
        "an edited clone was drawn as its original"
    );
}
