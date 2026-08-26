//! Which batch a world sprite lands in, and where it is drawn.

use glam::Vec2;
use sindri_render::{FrameCommand, RenderStage, SpriteDepth};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings, WorldProjection};

use crate::support::{VIEWPORT, close, scene, world_from};

/// A layer is an explicit override on order, so it splits the batches even
/// when everything else about two sprites is the same.
#[test]
fn sprites_batch_per_layer_and_sort_back_to_front() {
    let world = world_from(&scene(
        r#",
        { "id": "near", "transform_3d": { "position": [0.0, 0.0, 1.0] },
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
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2, "one batch per layer");
    assert_eq!(frame.passes()[0].layer.0, 100);
    assert_eq!(frame.passes()[1].layer.0, 200);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the first pass should be a sprite batch");
    };
    assert_eq!(instances.len(), 2);
    let alphas: Vec<f32> = instances.iter().map(|sprite| sprite.tint()[3]).collect();
    assert!(
        close(alphas[0], 0.75) && close(alphas[1], 0.25),
        "the further sprite must be drawn first, got {alphas:?}"
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
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "west", "transform_3d": { "position": [-3.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
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

/// A sprite is in the world: it is drawn through the world camera, in the
/// transparent stage rather than the screen overlay, and its transform reaches
/// it whole — Z included, which a UI image uses only for ordering.
#[test]
fn sprites_draw_through_the_world_camera_with_their_full_transform() {
    let world = world_from(&scene(
        r#",
        { "id": "prop", "transform_3d": { "position": [1.0, 2.0, -3.0] },
          "components": { "sindri.sprite": { "texture": "b" } } }"#,
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

    // The viewer camera moved, so the sprite's picture must move with it. A UI
    // image is instead resolved directly against the viewport.
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
