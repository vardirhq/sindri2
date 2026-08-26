//! What a panel is made of: its frame, its header, and what it says when it is
//! empty.
//!
//! Every dock in the editor used to open with `panel_title` — four pixels of
//! space, a bold word, a stock separator — and every one then diverged: the
//! project browser grew tabs where the hierarchy had a label, and the inspector
//! had neither. A header is a structural element, so it is drawn rather than
//! spelled out again per panel: one strip, one baseline, one place actions go.

use eframe::egui::{
    self, Align, Align2, FontId, Layout, Pos2, Rect, RichText, Sense, UiBuilder, Vec2,
};
use egui_material_icons::MaterialIcon;

use crate::ui::theme::{color, hairline, metric, text};

/// The frame every dockable panel is drawn in.
pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(color::PANEL)
        .stroke(hairline())
        .inner_margin(0)
}

/// The frame for the region a viewport lives in.
///
/// No border of its own: the rendered image fills it edge to edge and the
/// viewport paints its own outline over the picture.
pub fn viewport_frame() -> egui::Frame {
    egui::Frame::new().fill(color::INK).inner_margin(0)
}

/// A panel's header strip: a rule, a name, and room for the panel's own
/// controls at the far end.
///
/// The name is set in small capitals because a panel header is a label for a
/// region rather than a sentence — it should be findable without being read,
/// and it should not compete with the row names underneath it.
pub fn header<R>(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    title: &str,
    actions: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, metric::HEADER_HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color::HEADER);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, hairline());
    // A short accent rule at the leading edge. It is the one mark that makes a
    // header read as a header at a glance in a window with five of them, and it
    // costs two pixels of width.
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(rect.left(), rect.center().y - 6.0),
            Vec2::new(2.0, 12.0),
        ),
        0.0,
        color::FORGE_DIM,
    );
    let mut cursor = rect.left() + metric::GUTTER + 2.0;
    painter.text(
        Pos2::new(cursor, rect.center().y),
        Align2::LEFT_CENTER,
        icon.outlined().codepoint,
        FontId::new(14.0, icon.outlined().font_family()),
        color::TEXT_FAINT,
    );
    cursor += 18.0;
    painter.text(
        Pos2::new(cursor, rect.center().y),
        Align2::LEFT_CENTER,
        title.to_uppercase(),
        FontId::proportional(text::HEADING - 1.0),
        color::TEXT_MUTED,
    );
    let actions_rect = Rect::from_min_max(
        Pos2::new(cursor + 40.0, rect.top()),
        Pos2::new(rect.right() - 4.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(actions_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 4.0;
    actions(&mut child)
}

/// A panel's contents, inset from its edges by the standard gutter.
pub fn body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: metric::GUTTER_EDGE,
            right: metric::GUTTER_EDGE,
            top: 6,
            bottom: 4,
        })
        .show(ui, add)
        .inner
}

/// A dividing line inside a panel.
///
/// egui's own separator is a light bar with generous margins on both sides,
/// which in a dense panel reads as a gap with something in it. This is the line
/// the rest of the editor's borders are.
pub fn rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 7.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, hairline());
}

/// A dividing line with no space around it, for stacking sections flush.
pub fn rule_tight(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.top(), hairline());
}

/// What a panel says when there is nothing in it.
///
/// A blank panel is the editor failing to say anything: the inspector with no
/// selection was an empty rectangle, which is indistinguishable from a panel
/// that is broken. One icon, one line of what this panel is for, one line of
/// how to fill it.
pub fn empty_state(ui: &mut egui::Ui, icon: MaterialIcon, headline: &str, hint: &str) {
    // Inset before centring: a wrapped hint inside a centred layout is given
    // the whole panel to wrap in, and ran under the panel's own edge.
    let width = (ui.available_width() - 4.0 * metric::GUTTER).max(120.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(width);
        ui.add_space((ui.available_height() * 0.22).clamp(18.0, 96.0));
        ui.label(
            icon.outlined()
                .rich_text()
                .size(26.0)
                .color(color::LINE.gamma_multiply(1.6)),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(headline)
                .size(text::BODY)
                .color(color::TEXT_MUTED),
        );
        ui.add_space(3.0);
        ui.add(
            egui::Label::new(
                RichText::new(hint)
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            )
            .wrap(),
        );
    });
}

/// A one-line note inside a panel, for a state that is not an error.
pub fn note(ui: &mut egui::Ui, message: &str) {
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        ui.add(
            egui::Label::new(
                RichText::new(message)
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            )
            .wrap(),
        );
    });
}

/// A one-line note about something that went wrong.
pub fn problem(ui: &mut egui::Ui, message: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(metric::GUTTER);
        ui.add(
            egui::Label::new(
                RichText::new(message)
                    .size(text::NOTE)
                    .color(color::DANGER_TEXT),
            )
            .wrap(),
        );
    });
}

/// The editor's search box: an icon in the well, and the field beside it.
///
/// Painted into a region it allocates itself rather than assembled out of
/// nested layouts. A `Frame` around a `horizontal` inherits whichever direction
/// its parent happens to be laying out in, so the same call drew the magnifier
/// on the left in the hierarchy and on the right in the project browser's
/// right-aligned toolbar — and took the panel's remaining height with it.
pub fn search(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    let width = ui.available_width().max(64.0);
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, metric::CONTROL_HEIGHT + 4.0),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, crate::ui::theme::radius(), color::WELL);
    painter.rect_stroke(
        rect,
        crate::ui::theme::radius(),
        egui::Stroke::new(1.0, color::LINE_SOFT),
        egui::StrokeKind::Inside,
    );
    let glyph = crate::ui::icons::SEARCH.outlined();
    painter.text(
        Pos2::new(rect.left() + 7.0, rect.center().y),
        Align2::LEFT_CENTER,
        glyph.codepoint,
        FontId::new(13.0, glyph.font_family()),
        color::TEXT_FAINT,
    );
    let field = Rect::from_min_max(
        Pos2::new(rect.left() + 23.0, rect.top() + 2.0),
        Pos2::new(rect.right() - 5.0, rect.bottom() - 2.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(field)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.add_sized(
        field.size(),
        egui::TextEdit::singleline(value)
            .hint_text(RichText::new(hint).size(text::LABEL))
            .frame(egui::Frame::NONE),
    );
}

/// A small painted status dot.
///
/// Painted rather than written: the bundled Inter subset carries 192 glyphs and
/// has no `U+25CF`, so a text bullet renders as a missing-glyph box. Painting it
/// keeps the indicator independent of font coverage.
pub fn status_dot(ui: &mut egui::Ui, tint: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, tint);
}
