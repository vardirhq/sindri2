//! The axis indicator and framing a selection.

use eframe::egui::Vec2;
use glam::{Mat4, Vec3};
use sindri_render::look_at;
use sindri_scene::CameraView;

use super::super::camera::pan_to_centre;
use super::super::editing::find_by_source_id;
use super::super::overlay::{AXIS_ARM, axis_arms};
use super::super::*;
use super::support::*;

/// Where one axis ends up on screen, by name.
fn arm(view: Mat4, axis: &str) -> Vec2 {
    axis_arms(view, 1.0)
        .into_iter()
        .find(|(_, _, label)| *label == axis)
        .map(|(offset, _, _)| offset)
        .expect("every axis is drawn")
}

/// The indicator has to answer the camera. It was painted at three fixed
/// offsets, so it claimed the same orientation from every angle — the one
/// control in the editor that was wrong rather than merely idle.
#[test]
fn the_axis_indicator_turns_with_the_camera() {
    let front = look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
    assert!(arm(front, "X").x > 0.9, "X points across the picture");
    assert!(
        arm(front, "Y").y < -0.9,
        "Y points up it, and the screen's Y grows downwards"
    );
    assert!(
        arm(front, "Z").length() < 0.01,
        "Z points at the viewer, so it has nowhere to go on screen"
    );

    // A quarter turn to the side and the two swap: Z now lies across the
    // picture and X points at the viewer.
    let side = look_at(Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, Vec3::Y);
    // Standing on +X and facing the origin puts world +Z on the left,
    // which is where X's arm no longer is.
    assert!(
        arm(side, "Z").x < -0.9,
        "Z has taken the across-screen axis"
    );
    assert!(arm(side, "X").length() < 0.01, "and X points at the viewer");
    assert!(arm(side, "Y").y < -0.9, "up is still up");
}

/// An arm behind the origin is drawn under the ones in front of it, so the
/// indicator reads as three arms in space rather than three flat lines.
#[test]
fn the_axis_indicator_draws_back_to_front() {
    // Looking from above and to one side, so no two arms share a depth.
    let view = look_at(Vec3::new(4.0, 3.0, 5.0), Vec3::ZERO, Vec3::Y);
    let order: Vec<&str> = axis_arms(view, AXIS_ARM)
        .iter()
        .map(|(_, _, label)| *label)
        .collect();
    let depth = |axis: Vec3| view.transform_vector3(axis).z;
    let mut expected = [
        (depth(Vec3::X), "X"),
        (depth(Vec3::Y), "Y"),
        (depth(Vec3::Z), "Z"),
    ];
    expected.sort_by(|left, right| left.0.total_cmp(&right.0));
    let expected: Vec<&str> = expected.iter().map(|(_, label)| *label).collect();
    assert_eq!(order, expected, "the nearest arm is drawn last");
}

/// Framing a subject puts it in the middle, which is the whole claim.
///
/// Checked against the extractor rather than against the number the editor
/// computed: the pan is worked out by reading the pan's own definition
/// backwards, and the way that goes wrong is a sign, which only shows up by
/// asking where the subject ended up.
#[test]
fn focusing_a_selection_puts_it_in_the_middle_of_the_view() {
    let extractor = extractor();
    let world = demo_world();
    let entity = find_by_source_id(&world, "checker-cube").unwrap();
    let position = Vec3::from_array(world.get(entity).unwrap().transform_3d.unwrap().position);

    // Somewhere the subject is well off centre to begin with.
    let mut pan = GlamVec2::new(0.8, -0.5);
    let view = |pan| CameraView {
        orbit: GlamVec2::new(0.4, -0.2),
        distance_scale: 1.0,
        pan,
        projection: sindri_scene::WorldProjection::Perspective,
    };
    let camera = extractor
        .world_camera(&world, view(pan))
        .unwrap()
        .expect("the demo scene has a perspective camera");
    let before = camera.view.transform_point3(position);
    assert!(
        before.x.abs() + before.y.abs() > 0.5,
        "the subject has to start off centre for this to prove anything"
    );

    pan = pan_to_centre(camera, pan, position);

    let after = extractor
        .world_camera(&world, view(pan))
        .unwrap()
        .unwrap()
        .view
        .transform_point3(position);
    assert!(
        after.x.abs() < 1.0e-4 && after.y.abs() < 1.0e-4,
        "the subject should be in the middle and is at ({}, {})",
        after.x,
        after.y
    );
}

/// The Scene view has to sense clicks, not only drags.
///
/// egui sets a response's clicked flag only for a widget whose sense includes
/// clicks, and the viewport sensed drags alone — so `clicked_by` was always
/// false and nothing in the Scene view could be selected by clicking it, however
/// correct the picking underneath. The coupling is invisible from either side,
/// which is why it is stated here.
#[test]
fn the_viewport_answers_a_click_as_well_as_a_drag() {
    let sense = super::super::viewport::viewport_sense();
    assert!(
        sense.senses_click(),
        "select_viewport_click asks the response whether it was clicked"
    );
    assert!(
        sense.senses_drag(),
        "and the camera and the tile brush both read drags"
    );
}
