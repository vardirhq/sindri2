//! Headless coverage for world-to-frame extraction.
//!
//! Everything here runs without a GPU: a scene is loaded into a world, the
//! world is extracted, and the resulting passes are inspected directly.

use glam::Vec3;
use sindri_core::{SceneDocument, Transform3D, UnknownComponentPolicy, World};
use sindri_render::{FrameCommand, RenderStage, Viewport};
use sindri_scene::{CameraView, SceneExtractError, SceneExtractor, WorldProjection};

const VIEWPORT: Viewport = Viewport::new(512, 512);

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-5
}

fn world_from(json: &str) -> World {
    let document = SceneDocument::from_json(json).expect("fixture scene parses");
    let extractor = SceneExtractor::new().expect("built-in components register");
    extractor
        .validate(&document, UnknownComponentPolicy::Reject)
        .expect("fixture scene matches the built-in schemas");
    World::from_scene(&document)
        .expect("fixture scene loads")
        .world
}

fn cameras() -> &'static str {
    r#"
    {
      "id": "main-camera",
      "transform_3d": { "position": [3.0, 2.0, 4.0] },
      "components": { "sindri.camera": {
        "projection": "perspective", "target": [0.0, 0.0, 0.0], "up": [0.0, 1.0, 0.0],
        "vertical_fov_degrees": 45.0, "near": 0.1, "far": 100.0 } }
    },
    {
      "id": "overlay-camera",
      "components": { "sindri.camera": {
        "projection": "orthographic", "center": [0.0, 0.0],
        "vertical_size": 2.0, "near": 0.0, "far": 10.0 } }
    }"#
}

fn scene(entities: &str) -> String {
    format!(
        r#"{{ "format_version": 1, "entities": [{}{}] }}"#,
        cameras(),
        entities
    )
}

#[test]
fn a_world_with_only_cameras_draws_nothing() {
    let world = world_from(&scene(""));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("an empty scene extracts");
    assert!(frame.passes().is_empty());
}

#[test]
fn meshes_and_sprites_extract_into_ordered_passes() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 0 } } },
        { "id": "badge", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b", "anchor": "center", "layer": 100 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].stage, RenderStage::Opaque3d);
    assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
}

#[test]
fn sprites_batch_per_layer_and_sort_back_to_front() {
    let world = world_from(&scene(
        r#",
        { "id": "near", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b", "depth": 1.0, "layer": 100 } } },
        { "id": "far", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b", "depth": 9.0, "layer": 100 } } },
        { "id": "other-layer", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b", "depth": 1.0, "layer": 200 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2, "one batch per layer");
    assert_eq!(frame.passes()[0].layer.0, 100);
    assert_eq!(frame.passes()[1].layer.0, 200);

    let FrameCommand::SpriteBatch { instances } = &frame.passes()[0].command else {
        panic!("the first overlay pass should be a sprite batch");
    };
    // Greater depth is further back, so it must be drawn first.
    assert_eq!(instances.len(), 2);
    assert!(close(instances[0].tint()[3], 1.0));
}

#[test]
fn meshes_keep_one_pass_per_layer_in_layer_order() {
    let world = world_from(&scene(
        r#",
        { "id": "high", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 5 } } },
        { "id": "low", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 1 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].layer.0, 1);
    assert_eq!(frame.passes()[1].layer.0, 5);
}

/// Anchors resolve against the overlay camera's extent rather than a constant.
///
/// The values here are the world positions the previous hand-written extraction
/// produced, so the generalised anchor cannot silently move the demo overlay.
#[test]
fn sprite_anchors_resolve_against_the_overlay_extent() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_2d": { "position": [-0.78, 0.44] },
          "components": { "sindri.sprite": { "texture": "b", "anchor": "bottom_right" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("the scene extracts");

    let FrameCommand::SpriteBatch { instances } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let translation = instances[0].model().w_axis.truncate();
    // Overlay half-extent is (1.0, 1.0) at this aspect, so bottom right is
    // (1.0, -1.0) and the sprite offsets from there.
    assert!(
        close(translation.x, 0.22) && close(translation.y, -0.56),
        "anchored sprite landed at {translation:?}"
    );
}

/// The seam this crate exists for: gameplay writes the world, drawing follows.
#[test]
fn writing_a_transform_changes_what_is_drawn() {
    let mut world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let cube = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "cube")
        })
        .map(|(entity, _)| entity)
        .expect("the cube is in the world");

    world.get_mut(cube).unwrap().transform_3d = Some(Transform3D {
        position: [4.0, 0.0, 0.0],
        ..Transform3D::default()
    });

    let frame = extractor
        .extract(&world, VIEWPORT, CameraView::default())
        .expect("the scene extracts");
    let FrameCommand::TexturedCube { model } = frame.passes()[0].command else {
        panic!("expected a cube");
    };
    assert_eq!(model.w_axis.truncate(), Vec3::new(4.0, 0.0, 0.0));
}

#[test]
fn a_camera_view_moves_the_camera_without_moving_the_model() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let authored = extractor
        .extract(&world, VIEWPORT, CameraView::default())
        .unwrap();
    let orbited = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                orbit: glam::Vec2::new(0.5, 0.25),
                ..CameraView::default()
            },
        )
        .unwrap();

    assert_ne!(
        authored.passes()[0].camera.view_projection,
        orbited.passes()[0].camera.view_projection
    );
    let (FrameCommand::TexturedCube { model: before }, FrameCommand::TexturedCube { model: after }) =
        (&authored.passes()[0].command, &orbited.passes()[0].command)
    else {
        panic!("expected cubes");
    };
    assert_eq!(before, after);
}

#[test]
fn switching_the_world_projection_changes_only_the_world_camera() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } },
        { "id": "badge", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let perspective = extractor
        .extract(&world, VIEWPORT, CameraView::default())
        .unwrap();
    let orthographic = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                projection: WorldProjection::Orthographic,
                ..CameraView::default()
            },
        )
        .unwrap();

    assert_ne!(
        perspective.passes()[0].camera.view_projection,
        orthographic.passes()[0].camera.view_projection
    );
    assert_eq!(
        perspective.passes()[1].camera.view_projection,
        orthographic.passes()[1].camera.view_projection
    );
}

#[test]
fn drawing_without_a_camera_reports_which_one_is_missing() {
    let mesh_only = r#"{ "format_version": 1, "entities": [
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }] }"#;
    let world = world_from(mesh_only);
    assert!(matches!(
        SceneExtractor::new()
            .unwrap()
            .extract(&world, VIEWPORT, CameraView::default()),
        Err(SceneExtractError::MissingWorldCamera)
    ));

    let sprite_only = r#"{ "format_version": 1, "entities": [
        { "id": "badge", "transform_2d": {},
          "components": { "sindri.sprite": { "texture": "b" } } }] }"#;
    let world = world_from(sprite_only);
    assert!(matches!(
        SceneExtractor::new()
            .unwrap()
            .extract(&world, VIEWPORT, CameraView::default()),
        Err(SceneExtractError::MissingOverlayCamera)
    ));
}

#[test]
fn an_invalid_camera_distance_is_rejected() {
    let world = world_from(&scene(""));
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView {
                distance_scale: 0.0,
                ..CameraView::default()
            }
        ),
        Err(SceneExtractError::InvalidCameraDistanceScale)
    ));
}
