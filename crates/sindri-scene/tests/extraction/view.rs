//! Moving the viewer, and proving the model does not move with it.

use glam::{Vec2, Vec3};
use sindri_render::FrameCommand;
use sindri_scene::{
    CameraView, SceneExtractError, SceneExtractor, TextureBindings, WorldProjection,
};

use crate::support::{VIEWPORT, close, scene, world_from};

#[test]
fn a_camera_view_moves_the_camera_without_moving_the_model() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let authored = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap();
    let orbited = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                orbit: glam::Vec2::new(0.5, 0.25),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();

    assert_ne!(
        authored.passes()[0].camera.view_projection,
        orbited.passes()[0].camera.view_projection
    );
    let (
        FrameCommand::TexturedCube { model: before, .. },
        FrameCommand::TexturedCube { model: after, .. },
    ) = (&authored.passes()[0].command, &orbited.passes()[0].command)
    else {
        panic!("expected cubes");
    };
    assert_eq!(before, after);
}

/// Where the viewer target lands on screen once the view is panned.
///
/// The target sits at the centre of an unpanned frame, so its projected
/// position is exactly the pan, which is what makes the convention checkable.
fn projected_target(projection: WorldProjection, pan: glam::Vec2) -> glam::Vec2 {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                pan,
                projection,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();
    let clip = frame.passes()[0].camera.view_projection * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
    glam::Vec2::new(clip.x / clip.w, clip.y / clip.w)
}

/// Panning is measured in fractions of the framed half-height, so half a unit
/// moves the picture half a screen — and the same distance under either
/// projection, which is the whole reason it is not measured in world units.
#[test]
fn panning_moves_the_picture_by_the_same_amount_under_both_projections() {
    let unpanned = projected_target(WorldProjection::Perspective, glam::Vec2::ZERO);
    assert!(
        unpanned.abs_diff_eq(glam::Vec2::ZERO, 1.0e-5),
        "an unpanned camera should frame its target at the centre, got {unpanned:?}"
    );

    // A square viewport, so a pan of 0.5 half-heights is 0.5 in clip space.
    let perspective = projected_target(WorldProjection::Perspective, glam::Vec2::new(0.5, 0.0));
    let orthographic = projected_target(WorldProjection::Orthographic, glam::Vec2::new(0.5, 0.0));

    assert!(
        (perspective.x - 0.5).abs() < 1.0e-4,
        "panning right should move the picture right by half a screen, got {perspective:?}"
    );
    assert!(
        perspective.abs_diff_eq(orthographic, 1.0e-4),
        "the projections disagree about a pan: {perspective:?} against {orthographic:?}"
    );
}

#[test]
fn panning_up_moves_the_picture_up() {
    let panned = projected_target(WorldProjection::Perspective, glam::Vec2::new(0.0, 0.5));
    assert!(
        (panned.y - 0.5).abs() < 1.0e-4,
        "panning up should move the picture up, got {panned:?}"
    );
    assert!(panned.x.abs() < 1.0e-5, "panning up moved it sideways");
}

#[test]
fn panning_moves_the_camera_without_moving_the_model() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let authored = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap();
    let panned = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                pan: glam::Vec2::new(0.3, -0.2),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();

    assert_ne!(
        authored.passes()[0].camera.view_projection,
        panned.passes()[0].camera.view_projection
    );
    let (
        FrameCommand::TexturedCube { model: before, .. },
        FrameCommand::TexturedCube { model: after, .. },
    ) = (&authored.passes()[0].command, &panned.passes()[0].command)
    else {
        panic!("expected cubes");
    };
    assert_eq!(before, after, "panning must not move what it is looking at");
}

#[test]
fn a_pan_that_is_not_finite_is_rejected() {
    let world = world_from(&scene(""));
    let extracted = SceneExtractor::new().unwrap().extract(
        &world,
        VIEWPORT,
        CameraView {
            pan: glam::Vec2::new(f32::NAN, 0.0),
            ..CameraView::default()
        },
        &TextureBindings::new(),
    );
    assert!(matches!(
        extracted,
        Err(SceneExtractError::InvalidCameraPan)
    ));
}

#[test]
fn switching_the_world_projection_changes_only_the_world_camera() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let perspective = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();
    let orthographic = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                projection: WorldProjection::Orthographic,
                ..CameraView::default()
            },
            &TextureBindings::new(),
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
fn an_invalid_camera_distance_is_rejected() {
    let world = world_from(&scene(""));
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView {
                distance_scale: 0.0,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        ),
        Err(SceneExtractError::InvalidCameraDistanceScale)
    ));
}

/// What a viewport's own chrome is drawn from. The orbit that turns the picture
/// has to turn the view it is asked for, or an axis indicator drawn from it
/// will disagree with the scene under it — which is exactly what the editor's
/// did while it was painted at fixed angles.
#[test]
fn the_world_camera_view_answers_the_orbit_the_frame_was_drawn_with() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");

    let resting = extractor
        .world_camera(
            &world,
            CameraView {
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
        )
        .expect("asking for the viewer camera succeeds")
        .expect("a perspective viewer camera resolves")
        .view;
    // The resting viewer sits at (3, 2, 4) looking at the origin, so world X
    // is already partly towards the viewer rather than straight across.
    let across = resting.transform_vector3(Vec3::X);
    assert!(across.x > 0.0, "world X points right of the picture");
    assert!(across.z > 0.0, "and towards the viewer");

    let quarter_turn = extractor
        .world_camera(
            &world,
            CameraView {
                orbit: Vec2::new(std::f32::consts::FRAC_PI_2, 0.0),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
        )
        .expect("asking for the viewer camera succeeds")
        .expect("a perspective viewer camera resolves")
        .view;
    let turned = quarter_turn.transform_vector3(Vec3::X);
    assert!(
        (turned - across).length() > 0.5,
        "a quarter turn has to move the axes: {across} became {turned}"
    );
}

/// A pan of one moves the picture by exactly the framed half-height, which is
/// what lets an editor turn a distance on screen back into a pan and centre
/// itself on something.
#[test]
fn a_pan_of_one_moves_the_picture_by_the_framed_half_height() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");
    let camera = |pan| {
        extractor
            .world_camera(
                &world,
                CameraView {
                    pan,
                    projection: WorldProjection::Perspective,
                    ..CameraView::default()
                },
            )
            .expect("asking for the viewer camera succeeds")
            .expect("a perspective viewer camera resolves")
    };

    let resting = camera(Vec2::ZERO);
    let panned = camera(Vec2::new(0.0, 1.0));
    assert!(
        close(panned.framed_half_height, resting.framed_half_height),
        "panning frames the same amount of world"
    );
    // The origin was in the middle; after a pan of one upwards it is exactly a
    // half-height below it, measured in the view's own units.
    let before = resting.view.transform_point3(Vec3::ZERO);
    let after = panned.view.transform_point3(Vec3::ZERO);
    assert!(
        close(after.y - before.y, resting.framed_half_height),
        "a pan of one moved the origin by {} rather than {}",
        after.y - before.y,
        resting.framed_half_height
    );
}

/// The camera must never reach the pole, whatever it is asked for.
///
/// There the offset is parallel to `up` and nothing says which way round the
/// picture goes, so dragging through straight down whips the scene round to
/// face the other way. The guard lives in the orbit maths because that is where
/// the authored elevation is known: a caller clamping its own pitch would be
/// guessing at how far the scene had already tilted.
#[test]
fn an_orbit_stops_short_of_the_pole_however_far_it_is_driven() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");
    let up = Vec3::Y;
    // The resting viewer sits at (3, 2, 4), which is 1.19 radians off the up
    // axis, so this is the pitch that lands exactly on the pole.
    let onto_the_pole = -up.angle_between(Vec3::new(3.0, 2.0, 4.0));

    for pitch in [onto_the_pole, -100.0, -2.0, -1.5, 1.5, 2.0, 100.0] {
        for yaw in [0.0, 1.0, -2.5] {
            let camera = extractor
                .world_camera(
                    &world,
                    CameraView {
                        orbit: Vec2::new(yaw, pitch),
                        projection: WorldProjection::Perspective,
                        ..CameraView::default()
                    },
                )
                .expect("asking for the viewer camera succeeds")
                .expect("a perspective viewer camera resolves");
            assert!(
                camera.view.is_finite(),
                "yaw {yaw} pitch {pitch} produced {:?}",
                camera.view
            );
            // No pan, so the target is the origin and the camera's own position
            // is the offset the orbit produced.
            let eye = camera.view.inverse().w_axis.truncate();
            let polar = up.angle_between(eye);
            assert!(
                polar > 0.005 && polar < std::f32::consts::PI - 0.005,
                "yaw {yaw} pitch {pitch} put the camera {polar} radians from the pole"
            );
        }
    }
}
