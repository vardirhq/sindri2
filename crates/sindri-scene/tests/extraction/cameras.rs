//! Which camera a scene authored, and what happens when it authored
//! none.

use sindri_render::RenderStage;
use sindri_scene::{CameraView, SceneExtractError, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, document, world_from};

#[test]
fn the_ui_needs_no_authored_camera_but_the_world_does() {
    let world_sprite_without_a_camera = document(
        r#"
        { "id": "prop", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b" } } }"#,
    );
    let world = world_from(&world_sprite_without_a_camera);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingWorldCamera)
    ));

    let ui_image_without_a_camera = document(
        r#"
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.ui.image": { "texture": "b" } } }"#,
    );
    let world = world_from(&ui_image_without_a_camera);
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the UI is viewport-owned");
    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Overlay);
}

#[test]
fn an_authored_orthographic_camera_is_a_world_camera() {
    let world = world_from(&document(
        r#"
        { "id": "ortho", "transform_3d": { "position": [0.0, 0.0, 5.0] },
          "components": { "sindri.camera": {
            "projection": "orthographic", "vertical_size": 4.0,
            "near": 0.1, "far": 100.0 } } },
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("an orthographic authored camera draws the world");
    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Opaque3d);
    assert!(frame.passes()[0].camera.view_projection.is_finite());
}

#[test]
fn multiple_authored_world_cameras_are_rejected_explicitly() {
    let world = world_from(&document(
        r#"
        { "id": "perspective", "transform_3d": {},
          "components": { "sindri.camera": {
            "projection": "perspective", "vertical_fov_degrees": 45.0,
            "near": 0.1, "far": 100.0 } } },
        { "id": "orthographic", "transform_3d": {},
          "components": { "sindri.camera": {
            "projection": "orthographic", "vertical_size": 4.0,
            "near": 0.1, "far": 100.0 } } }"#,
    ));
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MultipleWorldCameras)
    ));
}

#[test]
fn world_content_without_a_camera_reports_missing_world_camera_but_screen_content_draws() {
    let mesh_only = document(
        r#"
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    );
    let world = world_from(&mesh_only);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingWorldCamera)
    ));

    let ui_only = document(
        r#"
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.ui.image": { "texture": "b" } } }"#,
    );
    let world = world_from(&ui_only);
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("UI-only content needs no authored camera");
    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Overlay);
}

/// A world with nothing to look through says so rather than inventing an
/// authored view.
#[test]
fn a_world_with_no_authored_camera_has_no_authored_view_to_offer() {
    let world = world_from(&document(
        r#"{ "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t.png" } } }"#,
    ));
    let extractor = SceneExtractor::new().expect("built-in components register");
    assert_eq!(
        extractor
            .world_camera(&world, CameraView::default())
            .expect("asking is not an error"),
        None
    );
}
