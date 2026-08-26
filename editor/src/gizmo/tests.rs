//! Which handle a click hits, and what dragging it does.

use super::*;
use sindri_core::World;

fn camera() -> Mat4 {
    glam::camera::rh::proj::directx::perspective(60_f32.to_radians(), 1.0, 0.1, 100.0)
        * glam::camera::rh::view::look_at_mat4(Vec3::new(4.0, 3.0, 6.0), Vec3::ZERO, Vec3::Y)
}

fn front_camera() -> Mat4 {
    glam::camera::rh::proj::directx::orthographic(-5.0, 5.0, -5.0, 5.0, 0.1, 100.0)
        * glam::camera::rh::view::look_at_mat4(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y)
}

fn entity() -> EntityId {
    World::default().next_handle()
}

#[test]
fn translate_handles_are_hit_tested_from_the_paths_that_are_drawn() {
    let visual = visual(
        GizmoMode::Translate,
        Transform3D::default(),
        GizmoSpace::World,
        camera(),
        Vec2::splat(400.0),
        3.0,
    )
    .unwrap();
    let x = visual
        .handles
        .iter()
        .find(|handle| handle.axis == Axis::X)
        .unwrap();
    assert_eq!(hit_test(&visual, x.points[1]), Some(Axis::X));
}

#[test]
fn z_lock_removes_the_world_z_translation_handle() {
    let transform = Transform3D {
        z_locked: true,
        ..Transform3D::default()
    };
    let visual = visual(
        GizmoMode::Translate,
        transform,
        GizmoSpace::World,
        camera(),
        Vec2::splat(400.0),
        3.0,
    )
    .unwrap();
    assert!(!visual.handles.iter().any(|handle| handle.axis == Axis::Z));
}

#[test]
fn snapping_quantizes_a_drag_value() {
    let snapping = Snapping {
        enabled: true,
        ..Snapping::default()
    };
    assert!((snapped(0.74, snapping.translation, snapping) - 0.5).abs() < 0.000_1);
    assert!((snapped(0.76, snapping.translation, snapping) - 1.0).abs() < 0.000_1);
}

#[test]
fn local_axes_follow_the_entities_rotation() {
    let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let direction = direction(
        Axis::X,
        rotation,
        GizmoSpace::Local,
        false,
        GizmoMode::Translate,
    )
    .unwrap();
    assert!(direction.abs_diff_eq(Vec3::Y, 0.000_1));
}

#[test]
fn an_axis_drag_moves_one_world_unit_for_one_world_unit_on_screen() {
    let camera = front_camera();
    let viewport = Vec2::splat(400.0);
    let drag = begin_drag(
        entity(),
        GizmoMode::Translate,
        Axis::X,
        Transform3D::default(),
        GizmoSpace::World,
        camera,
        Vec2::new(200.0, 200.0),
        viewport,
    )
    .unwrap();
    let moved = update_drag(
        drag,
        camera,
        Vec2::new(240.0, 200.0),
        viewport,
        Snapping::default(),
    )
    .unwrap();
    assert!((moved.position[0] - 1.0).abs() < 0.000_1);
    assert_eq!(moved.position[1..], [0.0, 0.0]);
}

#[test]
fn a_ring_drag_composes_a_quarter_turn() {
    let camera = front_camera();
    let viewport = Vec2::splat(400.0);
    let drag = begin_drag(
        entity(),
        GizmoMode::Rotate,
        Axis::Z,
        Transform3D::default(),
        GizmoSpace::Local,
        camera,
        Vec2::new(240.0, 200.0),
        viewport,
    )
    .unwrap();
    let turned = update_drag(
        drag,
        camera,
        Vec2::new(200.0, 160.0),
        viewport,
        Snapping::default(),
    )
    .unwrap();
    let rotation = Quat::from_array(turned.rotation);
    assert!(rotation.abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 0.000_1));
}
