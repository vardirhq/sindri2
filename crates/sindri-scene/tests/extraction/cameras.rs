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

/// Panning has to be exact, so this checks it against the projection itself.
///
/// A point is projected to the screen, the camera is panned by what a drag
/// asks for, and the point is projected again. If panning is right the point
/// has moved by exactly the drag -- which is the whole of "the ground stays
/// under your finger", stated in a way that cannot be satisfied by a plausible
/// basis with a sign wrong.
#[test]
fn a_drag_moves_the_ground_exactly_as_far_as_the_finger() {
    use glam::{Vec3, Vec4};

    let scene = document(
        r#"
        { "id": "eye",
          "transform_3d": { "position": [18.0, 16.0, 18.0],
                            "rotation": [-0.1913417, 0.3535534, 0.0732233, 0.9238795] },
          "components": { "sindri.camera": {
            "projection": "orthographic", "vertical_size": 20.0,
            "near": 0.1, "far": 200.0 } } }"#,
    );
    let mut world = world_from(&scene);
    // The fixture viewport is 512 square, which every float here holds exactly.
    let viewport = (512.0_f32, 512.0_f32);
    let aspect = viewport.0 / viewport.1;
    let extractor = SceneExtractor::new().unwrap();

    let camera = sindri_scene::world_camera_of(&world, extractor.components(), aspect)
        .expect("the camera resolves")
        .expect("the scene authored a world camera");

    // Somewhere the camera can see, with no particular relationship to its axes.
    let ground = Vec3::new(3.0, 1.0, -2.0);
    let to_screen = |camera: &sindri_scene::ViewCamera, point: Vec3| {
        let clip: Vec4 = camera.view_projection * point.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        [
            (ndc.x * 0.5 + 0.5) * viewport.0,
            (0.5 - ndc.y * 0.5) * viewport.1,
        ]
    };

    let before = to_screen(&camera, ground);
    let drag = [37.0, -21.0];
    let shift = sindri_scene::pan_for_drag(&camera, viewport, drag);

    // Move the camera by what the pan asked for, and look again.
    let eye = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "eye")
        })
        .map(|(entity, _)| entity)
        .expect("the camera is in the world");
    let moved = {
        let data = world.get_mut(eye).expect("still there");
        let mut transform = data.transform_3d.expect("it has one");
        transform.position = (Vec3::from(transform.position) + shift).into();
        data.transform_3d = Some(transform);
        sindri_scene::world_camera_of(&world, extractor.components(), aspect)
            .expect("the camera still resolves")
            .expect("it is still the world camera")
    };
    let after = to_screen(&moved, ground);

    let travelled = [after[0] - before[0], after[1] - before[1]];
    assert!(
        (travelled[0] - drag[0]).abs() < 0.05 && (travelled[1] - drag[1]).abs() < 0.05,
        "the ground follows the finger: dragged {drag:?} and the ground moved {travelled:?}"
    );
}
