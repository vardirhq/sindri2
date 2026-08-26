//! Orbit, pan, and framing, and what each leaves alone.
use sindri_render::{look_at, orthographic_projection, perspective_projection};

use super::*;

/// A viewer looking at the origin from `+Z`, with the depth convention the
/// renderer uses. `Mat4::IDENTITY` is not a view-projection: under a 0..1 depth
/// range everything in front of a camera at the origin is behind the near
/// plane, which is exactly what the clipping this file does now says.
fn viewer() -> Mat4 {
    orthographic_projection(-8.0, 8.0, -8.0, 8.0, 0.1, 100.0)
        * look_at(Vec3::new(0.0, 0.0, 12.0), Vec3::ZERO, Vec3::Y)
}

fn moved_camera() -> EditorCamera {
    EditorCamera {
        orbit: GlamVec2::new(0.7, -0.3),
        zoom: 1.4,
        pan: GlamVec2::new(0.25, 0.5),
        projection: CameraProjection::Orthographic,
    }
}

#[test]
fn the_game_view_ignores_wherever_the_editor_has_moved_its_camera() {
    assert_eq!(
        camera_for(WorkspaceTab::Game, moved_camera()),
        CameraView::default(),
        "the game view must render through the authored camera"
    );
}

#[test]
fn the_scene_view_carries_every_editor_adjustment() {
    let camera = camera_for(WorkspaceTab::Scene, moved_camera());
    assert_eq!(camera.orbit, GlamVec2::new(0.7, -0.3));
    assert_eq!(camera.pan, GlamVec2::new(0.25, 0.5));
    assert!((camera.distance_scale - 1.0 / 1.4).abs() < 1.0e-6);
    assert_eq!(camera.projection, WorldProjection::Orthographic);
}

#[test]
fn an_unmoved_scene_view_starts_on_its_independent_camera() {
    let scene = camera_for(WorkspaceTab::Scene, EditorCamera::default());
    let game = camera_for(WorkspaceTab::Game, EditorCamera::default());
    assert_eq!(scene.orbit, GlamVec2::ZERO);
    assert_eq!(scene.pan, GlamVec2::ZERO);
    assert_eq!(scene.distance_scale.to_bits(), 1.0_f32.to_bits());
    assert_eq!(scene.projection, WorldProjection::Perspective);
    assert_eq!(game, CameraView::default());
    assert_eq!(game.projection, WorldProjection::Authored);
    assert_ne!(scene.projection, game.projection);
}

#[test]
fn zooming_is_proportional_rather_than_a_fixed_step() {
    let step = |zoom: f32| (zoom * (50.0_f32 * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
    let near = MIN_ZOOM * 2.0;
    let far = MAX_ZOOM * 0.5;
    let ratio = |zoom: f32| step(zoom) / zoom;
    assert!((ratio(near) - ratio(far)).abs() < 1.0e-5);
    assert_eq!(step(MAX_ZOOM).to_bits(), MAX_ZOOM.to_bits());
}

#[test]
fn perspective_frustum_changes_with_fov() {
    let narrow = perspective_corners(30.0, 0.1, 10.0, 16.0 / 9.0);
    let wide = perspective_corners(90.0, 0.1, 10.0, 16.0 / 9.0);
    assert!(wide[1][1].x.abs() > narrow[1][1].x.abs());
    assert!(wide[1][2].y.abs() > narrow[1][2].y.abs());
}

#[test]
fn camera_rotation_turns_local_forward() {
    let transform = Transform3D {
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array(),
        ..Transform3D::default()
    };
    let forward = safe_rotation(transform) * -Vec3::Z;
    assert!(forward.distance(-Vec3::X) < 1.0e-5);
}

#[test]
fn malformed_camera_rotation_falls_back_to_identity() {
    let transform = Transform3D {
        rotation: [0.0; 4],
        ..Transform3D::default()
    };
    assert_eq!(safe_rotation(transform), Quat::IDENTITY);
}

#[test]
fn moving_perspective_camera_moves_projected_body() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.0));
    let camera = CameraComponent::Perspective {
        vertical_fov_degrees: 60.0,
        near: 0.1,
        far: 10.0,
    };
    let entity = sindri_core::World::default().next_handle();
    let first = camera_visual(
        entity,
        Transform3D::default(),
        camera,
        rect,
        viewer(),
        1.0,
        true,
    )
    .unwrap();
    let moved = camera_visual(
        entity,
        Transform3D {
            position: [0.5, 0.0, 0.0],
            ..Transform3D::default()
        },
        camera,
        rect,
        viewer(),
        1.0,
        true,
    )
    .unwrap();
    assert!(moved.body[0].x > first.body[0].x);
}

#[test]
fn orthographic_visual_uses_transform_position() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.0));
    let entity = sindri_core::World::default().next_handle();
    let camera = CameraComponent::Orthographic {
        vertical_size: 2.0,
        near: 0.1,
        far: 10.0,
    };
    let visual = camera_visual(
        entity,
        Transform3D {
            position: [0.4, 0.0, 0.0],
            ..Transform3D::default()
        },
        camera,
        rect,
        viewer(),
        1.0,
        true,
    )
    .unwrap();
    assert!(visual.body[0].x > rect.center().x);
}

#[test]
fn camera_marker_is_pickable() {
    let entity = sindri_core::World::default().next_handle();
    let visual = AuthoredCameraVisual {
        entity,
        body: vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(4.0, 0.0),
            Pos2::new(2.0, 4.0),
        ],
        lines: Vec::new(),
        hit_lines: Vec::new(),
    };
    assert_eq!(
        pick_authored_camera(&[visual], Pos2::new(2.0, 1.0)),
        Some(entity)
    );
}

/// The frustum belongs to the camera being worked on. Five scenes' worth of
/// frustum crossing the viewport says nothing about any of them.
#[test]
fn only_the_selected_camera_draws_its_frustum() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.0));
    let camera = CameraComponent::Perspective {
        vertical_fov_degrees: 60.0,
        near: 0.1,
        far: 10.0,
    };
    let entity = sindri_core::World::default().next_handle();
    let visual = |selected| {
        camera_visual(
            entity,
            Transform3D::default(),
            camera,
            rect,
            viewer(),
            1.0,
            selected,
        )
        .unwrap()
    };
    assert!(visual(false).lines.len() < visual(true).lines.len());
    assert!(
        !visual(false).lines.is_empty(),
        "an unselected camera still says which way it faces"
    );
    assert_eq!(
        visual(false).hit_lines,
        visual(true).hit_lines,
        "and it is just as clickable either way"
    );
}

/// The Scene view's shape must not change the shape of what an authored camera
/// frames. Resizing the panel used to reshape the frustum, which is a picture
/// claiming the scene changed because a divider moved.
#[test]
fn the_frustum_is_the_shape_of_the_game_viewport_not_the_panel() {
    let camera = CameraComponent::Perspective {
        vertical_fov_degrees: 60.0,
        near: 0.1,
        far: 10.0,
    };
    let width = |rect: Rect| {
        let visual = camera_visual(
            sindri_core::World::default().next_handle(),
            Transform3D::default(),
            camera,
            rect,
            viewer(),
            16.0 / 9.0,
            true,
        )
        .expect("the camera is in front of the viewer");
        let xs: Vec<f32> = visual
            .lines
            .iter()
            .flatten()
            .map(|point| (point.x - rect.center().x) / rect.width())
            .collect();
        xs.iter().fold(0.0_f32, |widest, x| widest.max(x.abs()))
    };

    let square = width(Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.0)));
    let wide = width(Rect::from_min_size(
        Pos2::ZERO,
        egui::Vec2::new(800.0, 400.0),
    ));
    assert!(
        (square - wide).abs() < 1.0e-3,
        "the frustum spanned {square} of a square panel and {wide} of a wide one"
    );
}

/// A corner behind the eye has no projection, and dividing by its negative `w`
/// produces a point on the wrong side of the screen. The segment is cut at the
/// near plane instead, which is what stopped the frustum smearing across the
/// viewport while orbiting.
#[test]
fn a_segment_crossing_the_eye_is_cut_rather_than_mirrored() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::Vec2::splat(400.0));
    let view_projection = perspective_projection(60.0_f32.to_radians(), 1.0, 0.1, 100.0);
    let in_front = Vec3::new(0.0, 0.0, -5.0);
    let behind = Vec3::new(0.0, 0.0, 5.0);

    assert!(
        project_point(rect, view_projection, behind).is_none(),
        "a point behind the eye is not somewhere on screen"
    );
    let segment = project_segment(rect, view_projection, in_front, behind)
        .expect("the half in front of the eye is still drawn");
    assert!(
        rect.expand(rect.width()).contains(segment[1]),
        "the cut end landed at {:?}, which is not near the viewport at all",
        segment[1]
    );
    assert!(
        project_segment(rect, view_projection, behind, behind + Vec3::X).is_none(),
        "a segment entirely behind the eye is not drawn at all"
    );
}
