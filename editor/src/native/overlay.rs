//! What is painted over a rendered view rather than into it.
//!
//! The Scene view's chrome is drawn, not laid out: it sits on top of a GPU
//! image, and an egui widget there would either fight the drag handling the
//! viewport needs or take a bite out of the picture. So this file is painter
//! work — a status plate, an axis indicator, and the manipulator arms — all
//! reading their colours from the same tokens the panels use.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use glam::{Mat4, Vec3};

use crate::gizmo::{self, Axis};
use crate::ui::theme::{color, hairline, metric, radius, text};

/// The colour an axis is drawn in, so an arm and the inspector's X field are
/// recognisably the same axis.
const fn axis_colour(axis: Axis) -> Color32 {
    match axis {
        Axis::X => color::AXIS_X,
        Axis::Y => color::AXIS_Y,
        Axis::Z => color::AXIS_Z,
    }
}

pub(super) fn paint_transform_gizmo(
    painter: &egui::Painter,
    rect: Rect,
    visual: &gizmo::GizmoVisual,
    active: Option<Axis>,
) {
    for handle in &visual.handles {
        let colour = if active == Some(handle.axis) {
            Color32::WHITE
        } else {
            axis_colour(handle.axis)
        };
        let points: Vec<Pos2> = handle
            .points
            .iter()
            .map(|point| rect.min + Vec2::new(point.x, point.y))
            .collect();
        painter.add(Shape::line(points.clone(), Stroke::new(2.5, colour)));
        if let Some(end) = points.last().copied()
            && handle.points.len() == 2
        {
            painter.circle_filled(end, 4.0, colour);
        }
    }
    painter.circle_filled(
        rect.min + Vec2::new(visual.origin.x, visual.origin.y),
        3.5,
        Color32::WHITE,
    );
}

/// What the Scene view says about itself, in the corner it can spare.
///
/// One plate rather than two floating labels: the selection, the mode it would
/// be manipulated in, and the pointer bindings are one answer to "what will
/// happen if I drag here", and they were three separate strings before.
pub(super) struct ViewportStatus<'a> {
    pub(super) selection: &'a str,
    pub(super) mode: &'a str,
    pub(super) space: &'a str,
    pub(super) snapping: bool,
    pub(super) playing: bool,
}

pub(super) fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    status: &ViewportStatus<'_>,
    error: Option<&str>,
    axes: Option<Mat4>,
) {
    painter.rect_stroke(rect, 0.0, hairline(), StrokeKind::Inside);
    paint_status_plate(painter, rect, status);
    if status.playing {
        paint_play_border(painter, rect);
    }
    paint_error_banner(painter, rect, error);
    if let Some(view) = axes {
        paint_axis_gizmo(
            painter,
            Pos2::new(rect.right() - 44.0, rect.top() + 46.0),
            view,
        );
    }
}

/// The plate in the top-left corner of the Scene view.
fn paint_status_plate(painter: &egui::Painter, rect: Rect, status: &ViewportStatus<'_>) {
    let plate = Rect::from_min_size(rect.min + Vec2::new(10.0, 10.0), Vec2::new(252.0, 46.0));
    // Dark enough to read over a bright frame, translucent enough not to be a
    // hole punched in the picture.
    painter.rect_filled(plate, radius(), Color32::from_black_alpha(180));
    painter.rect_stroke(
        plate,
        radius(),
        Stroke::new(1.0, color::LINE.gamma_multiply(0.7)),
        StrokeKind::Inside,
    );
    // A short accent rule, the same mark a panel header carries, so the plate
    // reads as part of the editor rather than as part of the render.
    painter.rect_filled(
        Rect::from_min_size(plate.min + Vec2::new(0.0, 6.0), Vec2::new(2.0, 12.0)),
        0.0,
        color::FORGE,
    );
    painter.text(
        plate.min + Vec2::new(9.0, 6.0),
        Align2::LEFT_TOP,
        status.selection,
        FontId::proportional(text::LABEL),
        color::TEXT,
    );
    let detail = if status.snapping {
        format!("{} · {} · snapping", status.mode, status.space)
    } else {
        format!("{} · {}", status.mode, status.space)
    };
    painter.text(
        plate.min + Vec2::new(9.0, 24.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(text::NOTE),
        color::TEXT_FAINT,
    );
    painter.text(
        plate.min + Vec2::new(plate.width() - 9.0, 24.0),
        Align2::RIGHT_TOP,
        "Drag: orbit · Shift: pan",
        FontId::proportional(text::NOTE),
        color::TEXT_FAINT.gamma_multiply(0.85),
    );
}

/// A lit edge while the scene is running.
///
/// Play mode changes what every drag in the viewport means, and the transport
/// chip at the top of the window is a long way from where the pointer is.
fn paint_play_border(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(metric::SELECT_RULE, color::FORGE),
        StrokeKind::Inside,
    );
}

/// The game view's chrome: a frame, and anything that went wrong.
///
/// A render failure is still reported here, because a blank view with no
/// explanation is worse than a view with a message across it.
pub(super) fn paint_viewport_border(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
    painter.rect_stroke(rect, 0.0, hairline(), StrokeKind::Inside);
    paint_error_banner(painter, rect, error);
}

fn paint_error_banner(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
    let Some(error) = error else {
        return;
    };
    let banner = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.bottom() - 40.0),
        Vec2::new((rect.width() - 20.0).max(1.0), 28.0),
    );
    painter.rect_filled(banner, radius(), color::DANGER.gamma_multiply(0.22));
    painter.rect_stroke(
        banner,
        radius(),
        Stroke::new(1.0, color::DANGER),
        StrokeKind::Inside,
    );
    painter.text(
        banner.left_center() + Vec2::new(9.0, 0.0),
        Align2::LEFT_CENTER,
        error,
        FontId::proportional(text::NOTE),
        color::DANGER_TEXT,
    );
}

/// How long an axis arm is when it points straight across the screen.
pub(super) const AXIS_ARM: f32 = 22.0;

/// Where the three world axes point on screen, and in what order to draw them.
///
/// This used to be three hardcoded offsets, so the indicator claimed the same
/// orientation whichever way the camera was facing — the one control in the
/// editor that was wrong rather than merely idle, and the one the first audit
/// walked past because it swept controls instead of pixels.
///
/// Each axis is turned by the camera's view and then flattened: the screen's Y
/// grows downwards, so the view's Y is negated, and an axis pointing at or away
/// from the viewer foreshortens to a stub of its own accord. The order is back
/// to front by how near the viewer each arm ends, so the arm behind is drawn
/// under the ones in front rather than over them.
pub(super) fn axis_arms(view: Mat4, length: f32) -> [(Vec2, Color32, &'static str); 3] {
    let mut arms = [
        (Vec3::X, color::AXIS_X, "X"),
        (Vec3::Y, color::AXIS_Y, "Y"),
        (Vec3::Z, color::AXIS_Z, "Z"),
    ]
    .map(|(axis, colour, label)| {
        let facing = view.transform_vector3(axis);
        (
            facing,
            Vec2::new(facing.x, -facing.y) * length,
            colour,
            label,
        )
    });
    // Ascending depth: in view space the camera looks down -Z, so the largest Z
    // is the arm nearest the viewer and is drawn last.
    arms.sort_by(|left, right| left.0.z.total_cmp(&right.0.z));
    arms.map(|(_, offset, colour, label)| (offset, colour, label))
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2, view: Mat4) {
    // A ground behind the arms, because three thin lines over a bright frame
    // are three thin lines nobody can see.
    painter.circle_filled(origin, AXIS_ARM + 10.0, Color32::from_black_alpha(120));
    for (offset, colour, label) in axis_arms(view, AXIS_ARM) {
        let end = origin + offset;
        painter.line_segment([origin, end], Stroke::new(2.0, colour));
        painter.circle_filled(end, 5.0, colour);
        painter.text(
            end,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(text::NOTE - 1.0),
            Color32::from_rgb(16, 18, 22),
        );
    }
}
