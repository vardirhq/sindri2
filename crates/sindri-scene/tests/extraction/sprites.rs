//! Which batch a sprite lands in, and where it is drawn.

use glam::Vec2;
use sindri_render::{FrameCommand, RenderStage, SpriteDepth};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings, WorldProjection};

use crate::support::{VIEWPORT, close, document, scene, world_from};

/// Screen sprites sort by their authored Z within the viewport-owned screen
/// projection, so the lower Z is further back in the stack and is drawn first.
#[test]
fn sprites_batch_per_layer_and_sort_back_to_front() {
    let world = world_from(&document(
        r#"
        { "id": "near", "transform_3d": { "position": [0.0, 0.0, -1.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "far", "transform_3d": { "position": [0.0, 0.0, -9.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.75] } } },
        { "id": "other-layer", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 200 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("screen sprites need no authored camera");

    assert_eq!(frame.passes().len(), 2, "one batch per layer");
    assert_eq!(frame.passes()[0].layer.0, 100);
    assert_eq!(frame.passes()[1].layer.0, 200);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the first screen pass should be a sprite batch");
    };
    assert_eq!(instances.len(), 2);
    let alphas: Vec<f32> = instances.iter().map(|sprite| sprite.tint()[3]).collect();
    assert!(
        close(alphas[0], 0.75) && close(alphas[1], 0.25),
        "the further sprite must be drawn first, got {alphas:?}"
    );
}

/// The Z of a screen-space sprite orders it without moving it. That is the one
/// place the sort key and the drawn position deliberately disagree, and it is
/// what keeps a HUD from disappearing when someone pushes it far back.
#[test]
fn a_screen_sprite_is_sorted_by_its_z_but_not_moved_by_it() {
    let world = world_from(&scene(
        r#",
        { "id": "front", "transform_3d": { "position": [0.1, 0.2, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "back", "transform_3d": { "position": [0.1, 0.2, -400.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
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

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert!(
        close(instances[0].tint()[3], 0.75),
        "the sprite pushed back must be drawn first"
    );
    let placements: Vec<_> = instances
        .iter()
        .map(|sprite| sprite.model().w_axis.truncate())
        .collect();
    assert_eq!(
        placements[0], placements[1],
        "a screen sprite's Z must not move it, even four hundred units of it"
    );
    assert!(
        close(placements[0].z, 0.0),
        "screen sprites draw flat in screen space, at {placements:?}"
    );
}

/// A world sprite is sorted by its real distance from the camera, so moving the
/// camera can reverse two sprites without either of them moving.
#[test]
fn world_sprites_sort_by_distance_from_the_camera_that_draws_them() {
    let world = world_from(&scene(
        r#",
        { "id": "east", "transform_3d": { "position": [3.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "space": "world", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "west", "transform_3d": { "position": [-3.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "space": "world", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let alphas = |view| {
        let frame = extractor
            .extract(&world, VIEWPORT, view, &TextureBindings::new())
            .expect("the scene extracts");
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
            panic!("expected a sprite batch");
        };
        instances
            .iter()
            .map(|sprite| sprite.tint()[3])
            .collect::<Vec<f32>>()
    };

    // The resting viewer camera is at (3, 2, 4), so the western sprite is further.
    let resting = alphas(CameraView {
        projection: WorldProjection::Perspective,
        ..CameraView::default()
    });
    assert!(close(resting[0], 0.75) && close(resting[1], 0.25));

    // Half a turn around the target puts the camera on the other side, and the
    // pair must swap without the scene changing at all.
    let orbited = alphas(CameraView {
        orbit: Vec2::new(std::f32::consts::PI, 0.0),
        projection: WorldProjection::Perspective,
        ..CameraView::default()
    });
    assert!(
        close(orbited[0], 0.25) && close(orbited[1], 0.75),
        "orbiting past the sprites must reverse them, got {orbited:?}"
    );
}

/// A world-space sprite is in the world: it is drawn through the world camera,
/// in the transparent stage rather than the screen overlay, and its transform
/// reaches it whole — Z included, which a screen-anchored sprite uses only for
/// ordering.
#[test]
fn world_space_sprites_draw_through_the_world_camera_with_their_full_transform() {
    let world = world_from(&scene(
        r#",
        { "id": "prop", "transform_3d": { "position": [1.0, 2.0, -3.0] },
          "components": { "sindri.sprite": { "texture": "b", "space": "world" } } }"#,
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

    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Transparent2d);
    let FrameCommand::SpriteBatch {
        depth, instances, ..
    } = &frame.passes()[0].command
    else {
        panic!("expected a sprite batch");
    };
    assert_eq!(*depth, SpriteDepth::Test, "world sprites test depth");
    let translation = instances[0].model().w_axis.truncate();
    assert!(
        close(translation.x, 1.0) && close(translation.y, 2.0) && close(translation.z, -3.0),
        "the world sprite landed at {translation:?} rather than where it was authored"
    );

    // The viewer camera moved, so the sprite's picture must move with it. A
    // screen-space sprite is instead resolved directly against the viewport.
    let orbited = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                orbit: Vec2::new(0.4, 0.0),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .expect("the scene extracts");
    assert_ne!(
        frame.passes()[0].camera.view_projection,
        orbited.passes()[0].camera.view_projection
    );
}

/// The default is what every sprite already was, so a scene written before the
/// choice existed keeps drawing exactly where it did.
#[test]
fn a_sprite_that_names_no_space_is_still_screen_anchored() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "anchor": "bottom_right" } } },
        { "id": "explicit", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "space": "screen",
            "anchor": "bottom_right", "layer": 5 } } }"#,
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

    let placements: Vec<_> = frame
        .passes()
        .iter()
        .map(|pass| {
            let FrameCommand::SpriteBatch {
                depth, instances, ..
            } = &pass.command
            else {
                panic!("expected sprite batches");
            };
            (pass.stage, *depth, instances[0].model().w_axis.truncate())
        })
        .collect();
    assert_eq!(
        placements.len(),
        2,
        "different layers are different batches"
    );
    assert_eq!(placements[0].0, RenderStage::Overlay);
    assert_eq!(placements[0].1, SpriteDepth::Ignore);
    assert_eq!(
        placements[0].2, placements[1].2,
        "naming the default must not move the sprite"
    );
}

/// The two spaces use different projections and pipelines, so they cannot share
/// a draw call however much else they have in common.
#[test]
fn sprites_in_different_spaces_do_not_share_a_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "hud", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } },
        { "id": "prop", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "space": "world", "layer": 100 } } }"#,
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

    // Same layer, same texture, and still two passes — and the world one is
    // drawn first, because screen-space content is over the world.
    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].stage, RenderStage::Transparent2d);
    assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
}

/// Anchors resolve against the viewport-owned screen extent rather than a
/// constant. The values here are the world positions the previous extraction
/// produced, so removing the authored overlay camera cannot silently move HUD.
#[test]
fn sprite_anchors_resolve_against_the_screen_extent() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "anchor": "bottom_right" } } }"#,
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

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let translation = instances[0].model().w_axis.truncate();
    // Screen half-extent is (1.0, 1.0) at this aspect, so bottom right is
    // (1.0, -1.0) and the sprite offsets from there.
    assert!(
        close(translation.x, 0.22) && close(translation.y, -0.56),
        "anchored sprite landed at {translation:?}"
    );
}
