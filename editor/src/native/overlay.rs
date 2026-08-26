//! What is painted over a rendered view rather than into it.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use glam::{Mat4, Vec3};

use crate::gizmo::{self, Axis};

use super::theme::{BORDER, TEXT, TEXT_FAINT};

pub(super) fn paint_transform_gizmo(
    painter: &egui::Painter,
    rect: Rect,
    visual: &gizmo::GizmoVisual,
    active: Option<Axis>,
) {
    for handle in &visual.handles {
        let color = if active == Some(handle.axis) {
            Color32::WHITE
        } else {
            match handle.axis {
                Axis::X => Color32::from_rgb(239, 92, 101),
                Axis::Y => Color32::from_rgb(89, 201, 135),
                Axis::Z => Color32::from_rgb(91, 151, 239),
            }
        };
        let points: Vec<Pos2> = handle
            .points
            .iter()
            .map(|point| rect.min + Vec2::new(point.x, point.y))
            .collect();
        painter.add(Shape::line(points.clone(), Stroke::new(2.5, color)));
        if let Some(end) = points.last().copied()
            && handle.points.len() == 2
        {
            painter.circle_filled(end, 4.0, color);
        }
    }
    painter.circle_filled(
        rect.min + Vec2::new(visual.origin.x, visual.origin.y),
        3.5,
        Color32::WHITE,
    );
}

pub(super) fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    selected_name: &str,
    error: Option<&str>,
    axes: Option<Mat4>,
) {
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let label_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(218.0, 42.0));
    painter.rect_filled(label_rect, 3.0, Color32::from_black_alpha(165));
    painter.text(
        label_rect.min + Vec2::new(9.0, 7.0),
        egui::Align2::LEFT_TOP,
        selected_name,
        FontId::proportional(12.0),
        TEXT,
    );
    painter.text(
        label_rect.min + Vec2::new(9.0, 24.0),
        egui::Align2::LEFT_TOP,
        "Primary: orbit or tool  ·  Secondary: orbit  ·  Shift-drag: pan",
        FontId::proportional(10.0),
        TEXT_FAINT,
    );
    paint_error_banner(painter, rect, error);
    if let Some(view) = axes {
        paint_axis_gizmo(
            painter,
            Pos2::new(rect.right() - 42.0, rect.top() + 48.0),
            view,
        );
    }
}

/// The game view's chrome: a frame, and anything that went wrong.
///
/// A render failure is still reported here, because a blank view with no
/// explanation is worse than a view with a message across it.
pub(super) fn paint_viewport_border(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    paint_error_banner(painter, rect, error);
}

fn paint_error_banner(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
    if let Some(error) = error {
        let error_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 42.0),
            Vec2::new((rect.width() - 24.0).max(1.0), 30.0),
        );
        painter.rect_filled(error_rect, 3.0, Color32::from_rgb(72, 28, 32));
        painter.text(
            error_rect.left_center() + Vec2::new(9.0, 0.0),
            egui::Align2::LEFT_CENTER,
            error,
            FontId::proportional(10.0),
            Color32::from_rgb(255, 184, 191),
        );
    }
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
        (Vec3::X, Color32::from_rgb(239, 92, 101), "X"),
        (Vec3::Y, Color32::from_rgb(89, 201, 135), "Y"),
        (Vec3::Z, Color32::from_rgb(91, 151, 239), "Z"),
    ]
    .map(|(axis, color, label)| {
        let facing = view.transform_vector3(axis);
        (
            facing,
            Vec2::new(facing.x, -facing.y) * length,
            color,
            label,
        )
    });
    // Ascending depth: in view space the camera looks down -Z, so the largest Z
    // is the arm nearest the viewer and is drawn last.
    arms.sort_by(|left, right| left.0.z.total_cmp(&right.0.z));
    arms.map(|(_, offset, color, label)| (offset, color, label))
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2, view: Mat4) {
    for (offset, color, label) in axis_arms(view, AXIS_ARM) {
        let end = origin + offset;
        painter.line_segment([origin, end], Stroke::new(2.0, color));
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.0),
            color,
        );
    }
}
