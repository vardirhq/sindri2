//! Which batch a world sprite lands in, and where it is drawn.

use glam::Vec2;
use sindri_core::{SpriteAnchor, SpriteSheetDocument};
use sindri_render::{FrameCommand, RenderStage, SpriteDepth, TextureId};
use sindri_scene::{
    CameraView, SceneExtractError, SceneExtractor, TextureBindings, WorldProjection,
};

use crate::support::{VIEWPORT, close, document, scene, world_from};

/// Painter order crosses texture boundaries.
///
/// Sorting into texture batches first used to turn A-behind-B-in-front-of-A
/// into one A draw followed or preceded by one B draw. No ordering of those two
/// calls can represent the requested picture. Correct extraction emits three
/// contiguous runs: A, B, then A again.
#[test]
fn transparent_sprites_interleave_across_textures() {
    let world = world_from(&document(
        r#"
        { "id": "camera", "transform_3d": { "position": [0.0, 0.0, 10.0] },
          "components": { "sindri.camera": {
            "projection": "perspective", "vertical_fov_degrees": 45.0,
            "near": 0.1, "far": 100.0 } } },
        { "id": "far-a", "transform_3d": { "position": [0.0, 0.0, -2.0] },
          "components": { "sindri.sprite": {
            "texture": "a.png", "layer": 5,
            "tint": [1.0, 1.0, 1.0, 0.2] } } },
        { "id": "middle-b", "transform_3d": { "position": [0.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b.png", "layer": 5,
            "tint": [1.0, 1.0, 1.0, 0.5] } } },
        { "id": "near-a", "transform_3d": { "position": [0.0, 0.0, 2.0] },
          "components": { "sindri.sprite": {
            "texture": "a.png", "layer": 5,
            "tint": [1.0, 1.0, 1.0, 0.8] } } }"#,
    ));
    let mut bindings = TextureBindings::new();
    bindings.bind("a.png", TextureId::new(1));
    bindings.bind("b.png", TextureId::new(2));

    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("the scene extracts");

    let textures = frame
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { texture, .. } => *texture,
            _ => panic!("the fixture contains only sprites"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        textures,
        [TextureId::new(1), TextureId::new(2), TextureId::new(1)],
        "texture batching swallowed painter order"
    );

    let alphas = frame
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch { instances, .. } => instances[0].tint()[3],
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(alphas, [0.2, 0.5, 0.8]);
}

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

/// A sheet that anchors at the foot lifts the picture so its bottom edge, not
/// its middle, lands on the entity.
///
/// This is what stops a character being drawn half a body low. The scale is
/// deliberately not 1, because the anchor moves the quad in its own space and
/// so has to scale with it — a sprite drawn twice as tall lifts twice as far.
#[test]
fn an_anchored_sheet_stands_its_sprite_on_the_entity() {
    let world = world_from(&scene(
        r#",
        { "id": "prop",
          "transform_3d": { "position": [1.0, 2.0, 0.0], "scale": [3.0, 5.0, 1.0] },
          "components": { "sindri.sprite": { "texture": "sheet.png#0" } } }"#,
    ));

    let anchored = |anchor| {
        let mut sheet = SpriteSheetDocument::from_grid(1, 1);
        sheet.anchor = anchor;
        let mut bindings = TextureBindings::new();
        bindings.bind("sheet.png", TextureId::new(1));
        bindings.bind_sheet("sheet.png", &sheet).expect("it slices");
        let frame = SceneExtractor::new()
            .unwrap()
            .extract(&world, VIEWPORT, CameraView::default(), &bindings)
            .expect("the scene extracts");
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
            panic!("expected a sprite batch");
        };
        instances[0].model().w_axis.truncate()
    };

    // Saying nothing draws exactly where a quad always drew, so art written
    // before there was an anchor keeps its picture.
    let silent = anchored(None);
    assert!(close(silent.x, 1.0) && close(silent.y, 2.0), "{silent:?}");
    let centred = anchored(Some(SpriteAnchor::Center));
    assert!(
        close(centred.y, silent.y),
        "naming the default changes nothing"
    );

    // Half of the five-unit height, so the foot of the picture sits at 2.0.
    let footed = anchored(Some(SpriteAnchor::Bottom));
    assert!(
        close(footed.y, 4.5) && close(footed.x, 1.0),
        "the anchored sprite centres at {footed:?} rather than standing at 2.0"
    );
}

/// An authored colour transform reaches the instance the shader reads.
///
/// The multiply and offset are per instance rather than per batch so that two
/// sprites recoloured differently still share one draw call when they share a
/// texture and a layer — which is the whole reason the values travel here
/// rather than in the batch key.
#[test]
fn an_authored_colour_transform_reaches_the_instance() {
    let world = world_from(&scene(
        r#",
        { "id": "recoloured", "transform_3d": { "position": [0.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b",
            "color_transform": {
              "multiply": [0.25, 0.5, 0.75, 1.0],
              "offset": [0.1, -0.2, 0.3, 0.0] } } } }"#,
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
    let multiply = instances[0].color_multiply();
    let offset = instances[0].color_offset();
    assert!(
        close(multiply[0], 0.25)
            && close(multiply[1], 0.5)
            && close(multiply[2], 0.75)
            && close(multiply[3], 1.0),
        "the multiply arrived as {multiply:?}"
    );
    assert!(
        close(offset[0], 0.1)
            && close(offset[1], -0.2)
            && close(offset[2], 0.3)
            && close(offset[3], 0.0),
        "the offset arrived as {offset:?}"
    );
}

/// A sprite that says nothing about colour draws exactly as it did before the
/// transform existed.
///
/// This is what makes the feature safe to add to a scene format that is
/// already in use: every sprite authored before it carries the identity, and
/// `sample * tint * 1 + 0` is the old `sample * tint`.
#[test]
fn a_sprite_without_a_colour_transform_carries_the_identity() {
    let world = world_from(&scene(
        r#",
        { "id": "plain", "transform_3d": { "position": [0.0, 0.0, 0.0] },
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

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let multiply = instances[0].color_multiply();
    let offset = instances[0].color_offset();
    assert!(
        multiply.iter().all(|channel| close(*channel, 1.0)),
        "an unauthored multiply should be the identity, not {multiply:?}"
    );
    assert!(
        offset.iter().all(|channel| close(*channel, 0.0)),
        "an unauthored offset should be the identity, not {offset:?}"
    );
}

/// A colour transform that is not a number is refused rather than drawn.
///
/// JSON has no way to spell a NaN and `serde_json` refuses a literal too large
/// for an `f64`, so the way one actually arrives is narrowing: `1e39` is an
/// ordinary `f64` and an infinity once it is an `f32`. It then spreads through
/// `sample * tint * multiply + offset` and leaves the pixel no value at all, so
/// it is refused here rather than drawn. Values merely outside zero to one are
/// a different matter and are left alone: an offset is signed by definition,
/// and the render target clips what it cannot show.
#[test]
fn a_colour_transform_that_is_not_finite_is_refused() {
    let world = world_from(&scene(
        r#",
        { "id": "broken", "transform_3d": { "position": [0.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b",
            "color_transform": { "multiply": [1.0, 1.0, 1.0, 1e39] } } } }"#,
    ));

    let refused = SceneExtractor::new().unwrap().extract(
        &world,
        VIEWPORT,
        CameraView::default(),
        &TextureBindings::new(),
    );
    assert!(
        matches!(refused, Err(SceneExtractError::InvalidColorTransform)),
        "an infinite multiply should be refused, not drawn"
    );

    // A wide but finite transform is somebody's authoring choice, not an
    // error: the render target clips what it cannot show.
    let wide = world_from(&scene(
        r#",
        { "id": "wide", "transform_3d": { "position": [0.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b",
            "color_transform": {
              "multiply": [4.0, 4.0, 4.0, 1.0],
              "offset": [-2.0, 0.0, 2.0, 0.0] } } } }"#,
    ));
    assert!(
        SceneExtractor::new()
            .unwrap()
            .extract(
                &wide,
                VIEWPORT,
                CameraView::default(),
                &TextureBindings::new()
            )
            .is_ok(),
        "a finite transform outside zero to one is authoring, not an error"
    );
}
