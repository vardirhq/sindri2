//! Overlays: panels over the scene, in a corner or wherever they were put.
//!
//! An overlay starts anchored to its corner. Dragging the empty part of its tab
//! strip lifts it out and it floats where it is dropped; the tabs themselves
//! still drag panels between places, as they always did. The grip in its
//! bottom-right corner resizes it. Double-clicking the strip, or dropping it
//! back into its corner, anchors it again.
//!
//! Free panels were refused once, on the grounds that panels placed by hand
//! pile on each other by accident. The rules here are what answer that: an
//! edge within a few points of the canvas's edge or another overlay's snaps to
//! it, a panel cannot be dropped even partly off the canvas, and the corner it
//! came from is always one drop away.

use eframe::egui::{self, Rect, Sense, Stroke, Vec2, pos2, vec2};

use crate::dock::{Corner, Place};
use crate::ui::theme::{color, metric};
use crate::ui::widgets::panel;

use super::super::EditorApp;
use super::{AXES_CLEARANCE, OVERLAY_GUTTER};

/// How close an edge has to come to another before it snaps to it.
const SNAP: f32 = 10.0;

/// The side of the square in an overlay's corner that resizes it.
const GRIP: f32 = 14.0;

/// The shortest an expanded overlay may be made: its strip and a few rows.
const MIN_HEIGHT: f32 = 120.0;

impl EditorApp {
    /// One corner's overlay, anchored or floating, and the gestures that move
    /// and size it.
    pub(super) fn draw_overlays(&mut self, ui: &mut egui::Ui, corner: Corner) {
        let place = Place::Overlay(corner);
        let Some(group) = self.preferences.workspace.group(place) else {
            return;
        };
        let field = self.overlay_field();
        let shape = Shape {
            corner,
            size: vec2(group.size, group.height),
            collapsed: group.collapsed,
            position: group.position,
        };
        let rect = overlay_rect(field, shape, place.min_size());
        let others: Vec<Rect> = self
            .dock
            .overlays
            .iter()
            .filter(|(other, _)| *other != place)
            .map(|(_, rect)| *rect)
            .collect();
        let mut gesture = Gesture::None;
        let mut drawn = rect;
        egui::Area::new(egui::Id::new(place.id()))
            .fixed_pos(rect.min)
            // Not kept on screen by egui. A row even slightly wider than the
            // overlay grew the area past the window edge, egui moved the
            // whole area left to bring it back, and the clip below stayed
            // where the overlay is: every row lost the start of its label. An
            // overflow now costs only the end of the row that overflows.
            .constrain(false)
            .order(egui::Order::Middle)
            // Interactable so a click on an overlay is a click on the overlay
            // rather than a camera drag through it onto the scene behind.
            .interactable(true)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                // Registered before the tabs, which are drawn on top of it and
                // so take a press on themselves: only the strip's empty space
                // moves the panel.
                let strip =
                    Rect::from_min_size(rect.min, vec2(rect.width(), metric::HEADER_HEIGHT));
                let handle = ui
                    .interact(
                        strip,
                        egui::Id::new((place.id(), "move")),
                        Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                // Measured from where the drag began, not frame by frame: a
                // slow drag moves a point or two a frame, and snapping each of
                // those steps would pull it back to the edge it started on, so
                // a panel in its corner could never be lifted out.
                let start = handle.id.with("start");
                if handle.drag_started() {
                    ui.data_mut(|data| data.insert_temp(start, rect.min));
                }
                if handle.double_clicked() {
                    gesture = Gesture::Anchor;
                } else if handle.dragged() {
                    let from = ui.data(|data| data.get_temp(start)).unwrap_or(rect.min);
                    let travelled = handle.total_drag_delta().unwrap_or_default();
                    gesture = Gesture::Move(from + travelled);
                }
                let card = panel::overlay_frame()
                    .show(ui, |ui| {
                        // Clipped just inside the frame, or a list longer than
                        // the overlay draws straight out of the bottom of it and
                        // over the world with no edge to say where the panel
                        // stopped. Inset by a point so the clip does not eat
                        // the card's own outline along with the overflow.
                        ui.set_clip_rect(rect.shrink(1.0));
                        panel::fill_slot(ui, true);
                        self.draw_group(ui, place);
                    })
                    .response
                    .rect;
                // A card is as tall as its contents, up to its height: an
                // inspector with little to show is a short card in a tall
                // slot. What is seen is what the grip, snapping and dropping
                // work on, or the grip would hang in empty space below it.
                drawn = card.intersect(rect);
                if !shape.collapsed {
                    let grip =
                        Rect::from_min_size(drawn.max - Vec2::splat(GRIP), Vec2::splat(GRIP));
                    // Clicks as well as drags: a scroll bar runs down the
                    // right of a long inspector, right beside the grip, and
                    // egui gives a press to what can be clicked before what can
                    // only be dragged. Sensing both, the grip is the top hit.
                    let sizing = ui
                        .interact(
                            grip,
                            egui::Id::new((place.id(), "size")),
                            Sense::click_and_drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                    paint_grip(ui.painter(), grip, sizing.hovered() || sizing.dragged());
                    if sizing.dragged() {
                        gesture = Gesture::Resize(drawn.size() + sizing.drag_delta());
                    }
                }
            });
        self.dock.overlays.retain(|(other, _)| *other != place);
        self.dock.overlays.push((place, drawn));
        self.apply(place, field, drawn, &others, gesture);
    }

    fn apply(&mut self, place: Place, field: Rect, rect: Rect, others: &[Rect], gesture: Gesture) {
        let workspace = &mut self.preferences.workspace;
        match gesture {
            Gesture::None => {}
            Gesture::Anchor => workspace.anchor(place),
            Gesture::Move(to) => {
                let moved = snapped(Rect::from_min_size(to, rect.size()), field, others);
                let moved = inside(moved, field);
                let Place::Overlay(corner) = place else {
                    return;
                };
                // Dropped back where its corner would put it: it is anchored
                // again, rather than floating at the corner's position.
                if moved.min.distance(anchored_min(field, corner, rect.size())) < 0.5 {
                    workspace.anchor(place);
                } else {
                    let offset = moved.min - field.min;
                    workspace.float(place, [offset.x, offset.y]);
                }
            }
            Gesture::Resize(size) => {
                // Sizing from the bottom-right corner keeps the top-left where
                // it is, which an anchored right-hand panel cannot do: it
                // would grow away from the pointer. So it floats first.
                let offset = rect.min - field.min;
                workspace.float(place, [offset.x, offset.y]);
                let width = size.x.clamp(place.min_size(), field.right() - rect.left());
                let height = size.y.clamp(MIN_HEIGHT, field.bottom() - rect.top());
                workspace.resize(place, width);
                workspace.resize_height(place, height);
            }
        }
    }
}

/// What the pointer asked of an overlay this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    None,
    Anchor,
    Move(egui::Pos2),
    Resize(Vec2),
}

/// What decides where an overlay is drawn.
#[derive(Clone, Copy, Debug)]
struct Shape {
    corner: Corner,
    size: Vec2,
    collapsed: bool,
    position: Option<[f32; 2]>,
}

/// Where an overlay is drawn: at its position if it has one, in its corner if
/// not, and inside the field either way.
fn overlay_rect(field: Rect, shape: Shape, min_width: f32) -> Rect {
    // A top-right overlay in its corner stops short of the corner below it,
    // where the view draws its axes; reaching the bottom would cover them.
    let reserved = if shape.position.is_none() && shape.corner == Corner::TopRight {
        AXES_CLEARANCE
    } else {
        0.0
    };
    let width = shape.size.x.clamp(
        min_width,
        (field.width() - OVERLAY_GUTTER * 2.0).max(min_width),
    );
    let height = if shape.collapsed {
        metric::HEADER_HEIGHT
    } else {
        shape.size.y.clamp(
            metric::HEADER_HEIGHT,
            (field.height() - OVERLAY_GUTTER * 2.0 - reserved).max(metric::HEADER_HEIGHT),
        )
    };
    let size = vec2(width, height);
    if let Some([x, y]) = shape.position {
        placed(field, field.min + vec2(x, y), size)
    } else {
        Rect::from_min_size(anchored_min(field, shape.corner, size), size)
    }
}

/// A floating overlay at `min`, kept on the canvas. Its height is how tall it
/// may grow, not how tall it is, so one lower down is shortened from the
/// bottom rather than pushed back up: a tall inspector can still be put
/// anywhere, and keeps the room it has left.
fn placed(field: Rect, min: egui::Pos2, size: Vec2) -> Rect {
    let room = field.shrink(OVERLAY_GUTTER);
    let lowest = (room.bottom() - size.y.min(MIN_HEIGHT)).max(room.top());
    let top = min.y.clamp(room.top(), lowest);
    let height = size.y.min(room.bottom() - top).max(metric::HEADER_HEIGHT);
    let x = min
        .x
        .clamp(room.left(), (room.right() - size.x).max(room.left()));
    Rect::from_min_size(pos2(x, top), vec2(size.x, height))
}

/// Where a corner puts an overlay of this size.
fn anchored_min(field: Rect, corner: Corner, size: Vec2) -> egui::Pos2 {
    pos2(
        if corner.is_left() {
            field.left() + OVERLAY_GUTTER
        } else {
            field.right() - OVERLAY_GUTTER - size.x
        },
        if corner.is_bottom() {
            field.bottom() - OVERLAY_GUTTER - size.y
        } else {
            field.top() + OVERLAY_GUTTER
        },
    )
}

/// Moves `rect` the least distance that puts it wholly inside `field`, inset
/// by the gutter, so no drop leaves a panel partly off the canvas.
fn inside(rect: Rect, field: Rect) -> Rect {
    let room = field.shrink(OVERLAY_GUTTER);
    let x = rect
        .left()
        .clamp(room.left(), (room.right() - rect.width()).max(room.left()));
    let y = rect
        .top()
        .clamp(room.top(), (room.bottom() - rect.height()).max(room.top()));
    Rect::from_min_size(pos2(x, y), rect.size())
}

/// `rect`, with each axis moved to the nearest line within `SNAP` of one of
/// its edges: the canvas's edges inset by the gutter, and the edges of the
/// other overlays, both flush and a gutter apart.
fn snapped(rect: Rect, field: Rect, others: &[Rect]) -> Rect {
    let room = field.shrink(OVERLAY_GUTTER);
    let mut xs = vec![room.left(), room.right()];
    let mut ys = vec![room.top(), room.bottom()];
    for other in others {
        xs.extend([
            other.left(),
            other.right(),
            other.left() - OVERLAY_GUTTER,
            other.right() + OVERLAY_GUTTER,
        ]);
        ys.extend([
            other.top(),
            other.bottom(),
            other.top() - OVERLAY_GUTTER,
            other.bottom() + OVERLAY_GUTTER,
        ]);
    }
    let x = snap_axis(rect.left(), rect.width(), &xs);
    let y = snap_axis(rect.top(), rect.height(), &ys);
    Rect::from_min_size(pos2(x, y), rect.size())
}

/// The start of a span of `length` beginning at `start`, moved so that its
/// start or its end sits on the nearest line within `SNAP`, or left alone.
fn snap_axis(start: f32, length: f32, lines: &[f32]) -> f32 {
    let mut best = start;
    let mut nearest = SNAP;
    for line in lines {
        for (distance, moved) in [
            ((start - line).abs(), *line),
            ((start + length - line).abs(), line - length),
        ] {
            if distance < nearest {
                nearest = distance;
                best = moved;
            }
        }
    }
    best
}

/// Three short diagonals in the corner, the mark every desktop uses for "drag
/// here to resize".
fn paint_grip(painter: &egui::Painter, grip: Rect, lit: bool) {
    let stroke = Stroke::new(
        1.0,
        if lit {
            color::TEXT_MUTED
        } else {
            color::TEXT_FAINT
        },
    );
    let corner = grip.max - vec2(3.0, 3.0);
    for step in [4.0, 7.5, 11.0] {
        painter.line_segment([corner - vec2(step, 0.0), corner - vec2(0.0, step)], stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> Rect {
        Rect::from_min_size(pos2(0.0, 40.0), vec2(1000.0, 700.0))
    }

    #[test]
    fn an_edge_near_a_line_snaps_to_it() {
        // The start is 6 from a line at 12: it snaps. Its end, at 312, is
        // nowhere near 500.
        assert!((snap_axis(18.0, 300.0, &[12.0, 500.0]) - 12.0).abs() < f32::EPSILON);
        // The end, 318, is 7 from a line at 325: the span moves so it ends
        // there.
        assert!((snap_axis(18.0, 300.0, &[325.0]) - 25.0).abs() < f32::EPSILON);
        // Nothing near: left where it is.
        assert!((snap_axis(100.0, 50.0, &[0.0, 400.0]) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_panel_cannot_be_dropped_off_the_canvas() {
        let dropped = Rect::from_min_size(pos2(900.0, 700.0), vec2(300.0, 200.0));
        let kept = inside(dropped, field());
        let room = field().shrink(OVERLAY_GUTTER);
        assert!(room.contains_rect(kept), "{kept:?} is not inside {room:?}");
    }

    #[test]
    fn a_floating_overlay_is_drawn_where_it_was_put() {
        let shape = Shape {
            corner: Corner::TopRight,
            size: vec2(300.0, 400.0),
            collapsed: false,
            position: Some([100.0, 50.0]),
        };
        let rect = overlay_rect(field(), shape, 180.0);
        assert_eq!(rect.min, pos2(100.0, 90.0));
        assert_eq!(rect.size(), vec2(300.0, 400.0));
    }

    #[test]
    fn a_tall_overlay_put_low_down_is_shortened_rather_than_pushed_up() {
        let shape = Shape {
            corner: Corner::TopRight,
            size: vec2(320.0, 2_000.0),
            collapsed: false,
            position: Some([400.0, 300.0]),
        };
        let rect = overlay_rect(field(), shape, 180.0);
        assert!((rect.top() - 340.0).abs() < f32::EPSILON, "{rect:?}");
        let floor = field().bottom() - OVERLAY_GUTTER;
        assert!((rect.bottom() - floor).abs() < f32::EPSILON, "{rect:?}");
    }

    #[test]
    fn an_anchored_overlay_sits_in_its_corner() {
        let shape = Shape {
            corner: Corner::BottomLeft,
            size: vec2(260.0, 240.0),
            collapsed: false,
            position: None,
        };
        let rect = overlay_rect(field(), shape, 180.0);
        assert!((rect.left() - OVERLAY_GUTTER).abs() < f32::EPSILON);
        assert!((rect.bottom() - (field().bottom() - OVERLAY_GUTTER)).abs() < f32::EPSILON);
    }
}
