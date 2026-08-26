//! Orbit, pan, and framing, and what each leaves alone.
use super::*;

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
        Mat4::IDENTITY,
        1.0,
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
        Mat4::IDENTITY,
        1.0,
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
        Mat4::IDENTITY,
        1.0,
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
