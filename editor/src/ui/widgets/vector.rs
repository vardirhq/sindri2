//! The control a position, a rotation, or a scale is edited with.
//!
//! Three bare drags in a row is what egui gives you, and it is the wrong shape
//! for a transform: nothing says which number is which axis except the order,
//! and the order is only knowable by counting. A vector field gives each
//! component its own well with its axis letter attached, tinted the same red,
//! green, and blue the gizmo arms and the view indicator use — so the field an
//! author is dragging is recognisably the arm they would have dragged in the
//! viewport.

use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::ui::theme::{color, metric, radius, text};

/// The letters the three components are called.
pub const AXES: [&str; 3] = ["X", "Y", "Z"];

/// One axis: its letter, and the number beside it.
///
/// Returns whether the number changed.
pub fn axis(
    ui: &mut egui::Ui,
    index: usize,
    value: &mut f64,
    width: f32,
    speed: f64,
    decimals: usize,
) -> bool {
    let enabled = ui.is_enabled();
    let tint = if enabled {
        color::AXES[index.min(2)]
    } else {
        color::AXES_DIM[index.min(2)]
    };
    let mut changed = false;
    egui::Frame::new()
        .fill(color::WELL)
        .stroke(Stroke::new(1.0, color::LINE_SOFT))
        .corner_radius(radius())
        .inner_margin(egui::Margin {
            left: 0,
            right: 4,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.horizontal(|ui| {
                // The letter sits on a tinted spine rather than being a
                // coloured character: a tinted glyph beside a number reads as
                // an alarm, and a spine reads as a label.
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(3.0, metric::CONTROL_HEIGHT - 4.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, crate::ui::theme::radius_tight(), tint);
                ui.label(
                    RichText::new(AXES[index.min(2)])
                        .size(text::NOTE)
                        .color(if enabled { tint } else { color::TEXT_FAINT }),
                );
                // Drawn as a value in a well rather than as a button in a box:
                // the frame around it is the well this closure just painted.
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                widgets.inactive.bg_stroke = Stroke::NONE;
                widgets.hovered.weak_bg_fill = color::RAISED;
                widgets.hovered.bg_stroke = Stroke::NONE;
                widgets.active.weak_bg_fill = color::EMBER;
                widgets.active.bg_stroke = Stroke::NONE;
                changed = ui
                    .add_sized(
                        [ui.available_width(), metric::CONTROL_HEIGHT],
                        egui::DragValue::new(value)
                            .speed(speed)
                            .max_decimals(decimals),
                    )
                    .changed();
            });
        });
    changed
}

/// How wide each of `count` axis fields should be to fill the row.
///
/// Counted through `u8` rather than cast from `usize`: no vector has more
/// components than that, and the conversion is then exact rather than lossy.
pub fn axis_width(ui: &egui::Ui, count: usize) -> f32 {
    let fields = f32::from(u8::try_from(count.max(1)).unwrap_or(u8::MAX));
    let gaps = 4.0 * (fields - 1.0);
    // Each well pays for its own right-hand margin as well as the gap to its
    // neighbour. Left out, three of them overflowed the row by a dozen pixels
    // and the panel silently widened itself to cover it.
    let furniture = gaps + 4.0 * fields;
    ((super::property::value_width(ui) - furniture) / fields).clamp(34.0, 128.0)
}

/// A row of axis fields under one label.
///
/// `locked` names components that are shown but cannot be edited, which is what
/// a transform with its Z pinned looks like: the number is still worth reading,
/// and the command layer would refuse the edit anyway.
pub fn row(
    ui: &mut egui::Ui,
    label: &str,
    values: &mut [f32],
    locked: &[bool],
    speed: f64,
) -> bool {
    let mut changed = false;
    super::property::Property::new(label).show(ui, |ui| {
        let width = axis_width(ui, values.len());
        for (index, value) in values.iter_mut().enumerate() {
            let mut number = f64::from(*value);
            let editable = !locked.get(index).copied().unwrap_or(false);
            let moved = ui
                .add_enabled_ui(editable, |ui| axis(ui, index, &mut number, width, speed, 3))
                .inner;
            if moved {
                #[allow(clippy::cast_possible_truncation)]
                {
                    *value = number as f32;
                }
                changed = true;
            }
        }
    });
    changed
}
