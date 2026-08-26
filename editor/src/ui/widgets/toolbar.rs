//! Strips of controls: the window's top bar, the viewport's tools, a panel's
//! own row of buttons.
//!
//! The scene tools used to be nine identically framed buttons with equal gaps,
//! which says all nine are the same kind of thing. They are not: four choose a
//! manipulator, two are about the scene camera, one is a snapping state. A
//! toolbar is grouped or it is a wall of icons, so grouping is what this
//! provides — and the group draws the box, not each control inside it.

use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke, UiBuilder, Vec2};

use crate::ui::theme::{color, hairline, metric, radius, text};

/// A full-width strip with the editor's toolbar ground and a rule beneath it.
pub fn strip<R>(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color::HEADER);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, hairline());
    let mut content = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(metric::GUTTER, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    content.spacing_mut().item_spacing.x = 4.0;
    add(&mut content)
}

/// A set of controls that belong together, in one well.
///
/// The controls inside lose their own frames: the group is the frame, and a box
/// inside a box is the look this exists to stop.
pub fn group<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(color::WELL)
        .stroke(Stroke::new(1.0, color::LINE_SOFT))
        .corner_radius(radius())
        .inner_margin(egui::Margin::symmetric(3, 2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui)
        })
        .inner
}

/// The gap between two groups, with a hairline in it.
pub fn divider(ui: &mut egui::Ui) {
    let height = (ui.available_height() - 12.0).clamp(10.0, 20.0);
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, color::LINE),
    );
    ui.add_space(3.0);
}

/// A named readout on a strip: what something currently is, in two words.
///
/// A control would be a lie — nothing here is chosen by pressing it — so this
/// is deliberately not shaped like one.
pub fn readout(ui: &mut egui::Ui, name: &str, value: &str, lit: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            RichText::new(name.to_uppercase())
                .size(text::NOTE)
                .color(color::TEXT_FAINT),
        );
        ui.label(RichText::new(value).size(text::LABEL).color(if lit {
            color::FORGE
        } else {
            color::TEXT_MUTED
        }));
    });
}

/// A small filled pill, for a state worth spotting from across the window.
pub fn chip(ui: &mut egui::Ui, label: &str, tint: egui::Color32) -> egui::Response {
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.16))
        .stroke(Stroke::new(1.0, tint.gamma_multiply(0.55)))
        .corner_radius(crate::ui::theme::radius_tight())
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(text::NOTE).color(tint));
        })
        .response
}
