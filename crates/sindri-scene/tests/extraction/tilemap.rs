//! A tilemap, expanded into the sprites that draw it.

use glam::Vec3;
use sindri_core::World;
use sindri_render::FrameCommand;
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, close, scene, world_from};

/// A tilemap draws its filled cells and skips its empty ones.
#[test]
fn a_tilemap_draws_only_the_cells_that_hold_a_tile() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a", "b"],
            "columns": 3, "rows": 2, "space": "world",
            "tiles": [0, 1, null, 1, null, 0] } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the tilemap extracts");

    assert_eq!(frame.passes().len(), 1);
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert_eq!(
        instances.len(),
        4,
        "four of the six cells hold a tile, and the two nulls draw nothing"
    );
}

/// The tilemap's whole reason for existing: the same floor, authored as one
/// entity instead of one per tile, is the same picture.
#[test]
fn a_tilemap_places_its_tiles_where_loose_sprites_were() {
    let loose = world_from(&scene(
        r#",
        { "id": "a", "transform_3d": { "position": [0.5, -0.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "b", "transform_3d": { "position": [1.5, -0.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "c", "transform_3d": { "position": [0.5, -1.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "d", "transform_3d": { "position": [1.5, -1.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } }"#,
    ));
    let mapped = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 2, "rows": 2, "space": "world",
            "tiles": [0, 0, 0, 0] } } }"#,
    ));

    let extractor = SceneExtractor::new().unwrap();
    let positions = |world: &World| -> Vec<Vec3> {
        let frame = extractor
            .extract(
                world,
                VIEWPORT,
                CameraView::default(),
                &TextureBindings::new(),
            )
            .expect("the scene extracts");
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
            panic!("expected a sprite batch");
        };
        let mut found: Vec<Vec3> = instances
            .iter()
            .map(|instance| instance.model().w_axis.truncate())
            .collect();
        found.sort_by(|left, right| {
            (left.x, left.y, left.z)
                .partial_cmp(&(right.x, right.y, right.z))
                .expect("no NaN in a placed tile")
        });
        found
    };

    let from_sprites = positions(&loose);
    let from_tilemap = positions(&mapped);
    assert_eq!(from_sprites.len(), 4);
    for (sprite, tile) in from_sprites.iter().zip(&from_tilemap) {
        assert!(
            close(sprite.x, tile.x) && close(sprite.y, tile.y) && close(sprite.z, tile.z),
            "one tilemap put a tile at {tile:?} where a sprite was at {sprite:?}"
        );
    }
}

/// A map's transform applies to the grid itself, not only to the quads in it.
/// Picking has always inverted the full model matrix, so rendering must compose
/// the same matrix or a rotated floor is painted somewhere other than it draws.
#[test]
fn a_tilemap_transform_moves_rotates_and_scales_its_grid() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {
            "position": [5.0, -2.0, 0.0],
            "rotation": [0.0, 0.0, 0.70710677, 0.70710677],
            "scale": [2.0, 3.0, 1.0] },
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 1, "rows": 1, "space": "world",
            "tiles": [0] } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the transformed map extracts");
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let position = instances[0].model().w_axis.truncate();
    assert!(
        close(position.x, 6.5) && close(position.y, -1.0),
        "the map-local centre is scaled and rotated before translation, got {position:?}"
    );
}

/// A map whose array does not match the size it claims is reported by name
/// rather than drawing part of a floor.
#[test]
fn a_tilemap_of_the_wrong_size_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 4, "rows": 4, "space": "world",
            "tiles": [0, 0] } } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("a map that is not the shape it claims does not extract");
    assert!(
        error.to_string().contains("16 cells"),
        "the error says how many cells the size calls for, got: {error}"
    );
}

/// A tile naming a cell the sheet does not have is reported rather than drawn
/// as whatever the maths happened to produce.
#[test]
fn a_tile_outside_the_sheet_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a", "b"],
            "columns": 1, "rows": 1, "space": "world",
            "tiles": [7] } } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("a tile outside the sheet does not extract");
    assert!(
        error.to_string().contains("tile 7"),
        "the error names the tile, got: {error}"
    );
}

/// A tilemap and a loose sprite sharing a texture and a layer share a batch, so
/// a prop can sit among the floor rather than behind a plane of it.
#[test]
fn a_tilemap_and_a_sprite_share_one_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 2, "rows": 1, "space": "world",
            "tiles": [0, 0] } } },
        { "id": "prop", "transform_3d": { "position": [0.5, -0.5, 0.5] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the scene extracts");

    assert_eq!(
        frame.passes().len(),
        1,
        "one texture and one layer is one batch"
    );
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert_eq!(instances.len(), 3, "two tiles and the prop");
}
