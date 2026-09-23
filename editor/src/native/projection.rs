//! Putting world-space points and lines on the Scene view's screen, and asking
//! whether a click landed on what was drawn.
//!
//! Shared by everything the Scene view draws over the world as an editor-only
//! marker: authored cameras and their frusta, and lights and their arrows.

use eframe::egui::{Pos2, Rect};
use glam::{Mat4, Vec3, Vec4};

/// How close to the eye a point may be and still be projected.
///
/// A clip-space `w` at or below this is a point on or behind the eye, and
/// dividing by it is where a frustum turns into the spray of lines that used to
/// appear when the Scene view was orbited past an authored camera.
const NEAR_CLIP: f32 = 1.0e-4;

fn clip_of(view_projection: Mat4, point: Vec3) -> Option<Vec4> {
    let clip = view_projection * point.extend(1.0);
    clip.is_finite().then_some(clip)
}

/// Whether a clip-space point is in front of the near plane, and so has a
/// place on screen at all.
fn in_front(clip: Vec4) -> bool {
    clip.w >= NEAR_CLIP && clip.z >= 0.0
}

/// Where a clip-space point lands in the viewport.
///
/// Only the divide is done here. Whether the point deserves to be projected is
/// [`in_front`] for a point and [`clipped_segment`] for a line, because a line
/// half in front of the viewer is drawn as far as the near plane rather than
/// dropped.
fn project_clip(rect: Rect, clip: Vec4) -> Option<Pos2> {
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() {
        return None;
    }
    Some(Pos2::new(
        rect.min.x + (ndc.x + 1.0) * 0.5 * rect.width(),
        rect.min.y + (1.0 - (ndc.y + 1.0) * 0.5) * rect.height(),
    ))
}

pub(super) fn project_point(rect: Rect, view_projection: Mat4, point: Vec3) -> Option<Pos2> {
    let clip = clip_of(view_projection, point)?;
    in_front(clip).then(|| project_clip(rect, clip)).flatten()
}

/// The part of a segment that is in front of the viewer, in clip space.
///
/// Clipping happens here, before the perspective divide, because after it the
/// question cannot be asked: a point behind the eye divides by a negative `w`
/// and lands somewhere plausible-looking on the opposite side of the screen.
/// Drawing the line to that point is what made a camera's frustum lurch and
/// smear across the viewport while orbiting — the maths was not wrong so much
/// as asked a question that has no answer.
fn clipped_segment(start: Vec4, end: Vec4) -> Option<(Vec4, Vec4)> {
    // Both half-spaces the near plane is made of: in front of the eye, and no
    // nearer than the near plane itself.
    let planes: [fn(Vec4) -> f32; 2] = [|point| point.w - NEAR_CLIP, |point| point.z];
    let (mut start, mut end) = (start, end);
    for distance in planes {
        let (near, far) = (distance(start), distance(end));
        match (near >= 0.0, far >= 0.0) {
            (false, false) => return None,
            (true, true) => {}
            (true, false) => end = start.lerp(end, near / (near - far)),
            (false, true) => start = end.lerp(start, far / (far - near)),
        }
    }
    Some((start, end))
}

/// One world-space segment as a line on screen, clipped to what is visible.
pub(super) fn project_segment(
    rect: Rect,
    view_projection: Mat4,
    start: Vec3,
    end: Vec3,
) -> Option<[Pos2; 2]> {
    let (start, end) = clipped_segment(
        clip_of(view_projection, start)?,
        clip_of(view_projection, end)?,
    )?;
    Some([project_clip(rect, start)?, project_clip(rect, end)?])
}

pub(super) fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_sq();
    if length_squared <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

pub(super) fn polygon_contains(points: &[Pos2], point: Pos2) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = points.len() - 1;
    for current in 0..points.len() {
        let a = points[current];
        let b = points[previous];
        if ((a.y > point.y) != (b.y > point.y))
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}
