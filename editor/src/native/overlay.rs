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
use crate::occlusion::FaultMark;
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

/// Where the ground draws over what stands on it.
///
/// Three marks per fault, because the fault is a relationship and drawing only
/// the broken cell would leave an author guessing which of its neighbours it is
/// broken against. The ground stood on is filled, the cell covering it is
/// outlined, and a line joins the two so a run of faults along one plot edge
/// reads as one cause rather than as scattered damage.
///
/// A fault nothing accounts for is drawn in danger rather than warning. The
/// others are a case the rule declines and says so; that one is the rule being
/// wrong somewhere nobody has stood yet.
pub(super) fn paint_occlusion_faults(painter: &egui::Painter, rect: Rect, marks: &[FaultMark]) {
    let place = |point: [f32; 2]| {
        Pos2::new(
            rect.min.x + point[0] * rect.width(),
            rect.min.y + point[1] * rect.height(),
        )
    };
    for mark in marks {
        let tint = if mark.blocked_by.is_some() {
            color::WARNING
        } else {
            color::DANGER
        };
        let standing: Vec<Pos2> = mark.standing.iter().copied().map(place).collect();
        let covering: Vec<Pos2> = mark.covering.iter().copied().map(place).collect();
        painter.add(Shape::convex_polygon(
            standing.clone(),
            tint.gamma_multiply(0.22),
            Stroke::NONE,
        ));
        painter.add(Shape::closed_line(covering.clone(), Stroke::new(1.5, tint)));
        if let (Some(from), Some(to)) = (centre(&standing), centre(&covering)) {
            painter.line_segment([from, to], Stroke::new(1.0, tint.gamma_multiply(0.7)));
        }
        if let Some(blocked) = mark.blocked_by {
            let wall: Vec<Pos2> = blocked.iter().copied().map(place).collect();
            painter.add(Shape::closed_line(wall, Stroke::new(1.0, color::FORGE_DIM)));
        }
    }
}

fn centre(points: &[Pos2]) -> Option<Pos2> {
    if points.is_empty() {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let count = points.len() as f32;
    let sum = points
        .iter()
        .fold(Vec2::ZERO, |sum, point| sum + point.to_vec2());
    Some(Pos2::new(sum.x / count, sum.y / count))
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

/// Where the rest of a multiple selection is.
///
/// A ring rather than a second set of arms: only one entity can carry handles,
/// so what the others need to say is "this moves too", and a mark that looked
/// like a gizmo would invite a drag that does nothing.
pub(super) fn paint_selection_marks(painter: &egui::Painter, marks: &[Pos2]) {
    for mark in marks {
        painter.circle_stroke(*mark, 5.0, Stroke::new(1.5, color::FORGE_BRIGHT));
        painter.circle_filled(*mark, 1.5, color::FORGE_BRIGHT);
    }
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
    visible: Rect,
    status: &ViewportStatus<'_>,
    error: Option<&str>,
    axes: Option<Mat4>,
) {
    painter.rect_stroke(rect, 0.0, hairline(), StrokeKind::Inside);
    // Both placed in the part of the view no bar covers. Against the view's
    // own edges, with the furniture floating, the plate sat half under the
    // title bar and the axes under the transport.
    paint_status_plate(painter, visible, status);
    if status.playing {
        paint_play_border(painter, rect);
    }
    paint_error_banner(painter, visible, error);
    if let Some(view) = axes {
        // The bottom-right corner, which no arrangement anchors a panel to;
        // the top-right one is the inspector's.
        paint_axis_gizmo(
            painter,
            Pos2::new(visible.right() - 44.0, visible.bottom() - 44.0),
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
pub(super) fn paint_viewport_border(
    painter: &egui::Painter,
    rect: Rect,
    visible: Rect,
    error: Option<&str>,
) {
    painter.rect_stroke(rect, 0.0, hairline(), StrokeKind::Inside);
    paint_error_banner(painter, visible, error);
}

/// The widest the banner grows, so it stays clear of the panels anchored in
/// the view's bottom corners.
const BANNER_MAX_WIDTH: f32 = 720.0;

/// What went wrong, across the bottom of the part of the view nothing covers.
///
/// `visible` rather than the view: with the furniture floating, the view is the
/// whole window and the status bar sits over its bottom edge, which is where a
/// banner placed against the view's own edge was drawn -- half under the bar.
/// Centred with a capped width, so the panels anchored in the bottom corners do
/// not cover its ends, and wrapped, because the message that matters most names
/// a field and what it accepts and does not fit on one line of a narrow view.
fn paint_error_banner(painter: &egui::Painter, visible: Rect, error: Option<&str>) {
    let Some(error) = error else {
        return;
    };
    let width = (visible.width() - 20.0).clamp(1.0, BANNER_MAX_WIDTH);
    let galley = painter.layout(
        error.to_owned(),
        FontId::proportional(text::NOTE),
        color::DANGER_TEXT,
        (width - 18.0).max(1.0),
    );
    let height = galley.size().y + 12.0;
    let banner = Rect::from_min_size(
        Pos2::new(
            visible.center().x - width / 2.0,
            visible.bottom() - 12.0 - height,
        ),
        Vec2::new(width, height),
    );
    // Backed like the status plate: a translucent red alone was unreadable
    // over bright terrain.
    painter.rect_filled(banner, radius(), Color32::from_black_alpha(200));
    painter.rect_filled(banner, radius(), color::DANGER.gamma_multiply(0.22));
    painter.rect_stroke(
        banner,
        radius(),
        Stroke::new(1.0, color::DANGER),
        StrokeKind::Inside,
    );
    painter.galley(banner.min + Vec2::new(9.0, 6.0), galley, color::DANGER_TEXT);
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
