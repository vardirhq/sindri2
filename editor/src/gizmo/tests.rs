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
        Anchoring::in_world(Transform3D::default()),
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
        Anchoring::in_world(transform),
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
        Anchoring::in_world(Transform3D::default()),
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
        Anchoring::in_world(Transform3D::default()),
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

/// A UI element's handles are drawn where the overlay puts it, and its third
/// axis is not offered.
///
/// The bug this is the guard for: the gizmo was drawn at the entity's transform
/// in world space while a UI element's transform is an offset from its anchor
/// in overlay space, so selecting Gather's title and choosing Move put a single
/// red arm in the bottom-left corner of the Scene view, mostly off screen,
/// while the text it belonged to was at the top.
#[test]
fn an_overlaid_gizmo_sits_on_the_overlay_and_offers_two_axes() {
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let viewport = Vec2::splat(400.0);
    let transform = Transform3D {
        position: [0.0, -0.2, 0.0],
        ..Transform3D::default()
    };
    let anchoring = Anchoring::on_overlay(placement.origin(transform, sindri_scene::UiAnchor::Top));

    let overlaid = visual(
        GizmoMode::Translate,
        anchoring,
        transform,
        GizmoSpace::World,
        overlay.view_projection,
        viewport,
        overlay.framed_half_height,
    )
    .unwrap();

    // Near the top of the viewport, because that is where an element anchored
    // there is drawn. Read as a world position it would be a hair below the
    // centre — which is the answer that used to be given.
    assert!(
        overlaid.origin.y < viewport.y * 0.2,
        "the handle belongs where the element is, not at {:?}",
        overlaid.origin
    );
    assert_eq!(
        overlaid.handles.len(),
        2,
        "a UI element's Z orders it rather than placing it, so there is no third arm"
    );
}

/// The same call for a world entity is unchanged: the handle is at the
/// transform, and all three axes are offered.
#[test]
fn a_world_gizmo_still_sits_at_its_transform() {
    let camera = front_camera();
    let viewport = Vec2::splat(400.0);
    let transform = Transform3D::default();
    let world = visual(
        GizmoMode::Translate,
        Anchoring::in_world(transform),
        transform,
        GizmoSpace::World,
        camera,
        viewport,
        5.0,
    )
    .unwrap();
    assert_eq!(world.handles.len(), 3);
    assert!((world.origin - viewport * 0.5).length() < 0.001);
}

/// Dragging an overlaid handle writes the authored offset, not the point the
/// handle was drawn at.
///
/// The two are different for anything not anchored to the centre, and the
/// pointer maths has to happen against the drawn origin while the answer lands
/// on the authored value.
#[test]
fn dragging_an_overlaid_handle_moves_the_authored_offset() {
    let (overlay, placement) = sindri_scene::overlay_for_viewport(1.0).unwrap();
    let viewport = Vec2::splat(400.0);
    let transform = Transform3D {
        position: [0.0, -0.2, 0.0],
        ..Transform3D::default()
    };
    let anchoring = Anchoring::on_overlay(placement.origin(transform, sindri_scene::UiAnchor::Top));
    let start = visual(
        GizmoMode::Translate,
        anchoring,
        transform,
        GizmoSpace::World,
        overlay.view_projection,
        viewport,
        overlay.framed_half_height,
    )
    .unwrap()
    .origin;

    let drag = begin_drag(
        entity(),
        GizmoMode::Translate,
        Axis::X,
        anchoring,
        transform,
        GizmoSpace::World,
        overlay.view_projection,
        start,
        viewport,
    )
    .unwrap();
    let moved = update_drag(
        drag,
        overlay.view_projection,
        start + Vec2::new(40.0, 0.0),
        viewport,
        Snapping::default(),
    )
    .unwrap();

    assert!(
        moved.position[0] > 0.0,
        "dragging right must increase the offset, not jump to the drawn origin"
    );
    assert!(
        (moved.position[1] + 0.2).abs() < 0.000_1,
        "and must leave the other axis where it was authored"
    );
}

/// A follower moves by the same offset from wherever it started, which is what
/// keeps a row of five pips a row when one of them is dragged.
#[test]
fn a_follower_moves_by_the_same_offset_from_its_own_start() {
    let from = Transform3D {
        position: [1.0, 0.0, 0.0],
        ..Transform3D::default()
    };
    let to = Transform3D {
        position: [1.0, 2.5, 0.0],
        ..Transform3D::default()
    };
    let follower = Transform3D {
        position: [-4.0, -1.0, 3.0],
        ..Transform3D::default()
    };

    let moved = Change::between(from, to).applied_to(follower);
    assert!(Vec3::from_array(moved.position).abs_diff_eq(Vec3::new(-4.0, 1.5, 3.0), 0.000_1));
    assert!(
        Vec3::from_array(moved.scale).abs_diff_eq(Vec3::from_array(follower.scale), 0.000_1),
        "a move is not a scale"
    );
}

/// Rotation is a quaternion, so the same turn is composed onto whatever the
/// follower already held rather than added to it.
#[test]
fn a_follower_turns_by_the_same_rotation_about_its_own_origin() {
    let quarter_turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let from = Transform3D::default();
    let to = Transform3D {
        rotation: quarter_turn.to_array(),
        ..Transform3D::default()
    };
    let follower = Transform3D {
        rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array(),
        position: [5.0, 0.0, 0.0],
        ..Transform3D::default()
    };

    let turned = Change::between(from, to).applied_to(follower);
    let expected = Quat::from_rotation_y(std::f32::consts::PI);
    assert!(
        Quat::from_array(turned.rotation).abs_diff_eq(expected, 0.000_1),
        "two quarter turns make a half turn, not a sum of components"
    );
    assert!(
        Vec3::from_array(turned.position).abs_diff_eq(Vec3::from_array(follower.position), 0.000_1),
        "and turning is not moving: a follower spins where it stands"
    );
}

/// Scale is additive rather than a ratio, so a follower at scale zero still
/// moves and one at scale ten does not leap.
#[test]
fn a_follower_scales_by_the_same_amount_it_can_survive() {
    let from = Transform3D::default();
    let to = Transform3D {
        scale: [2.0, 2.0, 2.0],
        ..Transform3D::default()
    };
    let flat = Transform3D {
        scale: [0.0, 1.0, 10.0],
        ..Transform3D::default()
    };

    assert!(
        Vec3::from_array(Change::between(from, to).applied_to(flat).scale)
            .abs_diff_eq(Vec3::new(1.0, 2.0, 11.0), 0.000_1)
    );
}

/// Whether an entity stays on its layer is a fact about that entity, so a
/// follower keeps its own lock rather than inheriting the dragged one's.
#[test]
fn a_follower_keeps_its_own_layer_lock() {
    let from = Transform3D::default();
    let to = Transform3D {
        position: [0.0, 1.0, 0.0],
        z_locked: true,
        ..Transform3D::default()
    };
    let free = Transform3D::default();

    assert!(!Change::between(from, to).applied_to(free).z_locked);
}
