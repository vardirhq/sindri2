//! How the editor looks: its palette, its typeface, and the small pieces of
//! chrome every panel is built from.
//!
//! The colours are here rather than beside the panels that use them because a
//! panel choosing its own greys is how an interface stops matching itself. A
//! widget belongs here when it carries no editor state — a title, a search box,
//! a label — and beside its panel when it does.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, FontData, FontFamily, FontId, Layout, Response, RichText, Sense, Stroke,
    TextStyle, Vec2,
};
use egui_material_icons::{MaterialIcon, icons::ICON_SEARCH};

pub(super) const INTER_FONT: &[u8] = include_bytes!("../../assets/Inter.ttf");

pub(super) const ACCENT: Color32 = Color32::from_rgb(246, 169, 35);

/// What a panel says something is wrong in, matching the console's errors.
pub(super) const PROBLEM: Color32 = Color32::from_rgb(255, 138, 148);

pub(super) const ACCENT_BRIGHT: Color32 = Color32::from_rgb(255, 187, 54);

pub(super) const ACCENT_SOFT: Color32 = Color32::from_rgb(59, 45, 20);

pub(super) const APP_BG: Color32 = Color32::from_rgb(9, 12, 16);

pub(super) const TOP_BG: Color32 = Color32::from_rgb(12, 15, 19);

pub(super) const PANEL_BG: Color32 = Color32::from_rgb(15, 19, 23);

pub(super) const PANEL_RAISED: Color32 = Color32::from_rgb(19, 24, 29);

pub(super) const FIELD_BG: Color32 = Color32::from_rgb(12, 16, 20);

pub(super) const BORDER: Color32 = Color32::from_rgb(39, 46, 53);

pub(super) const BORDER_SUBTLE: Color32 = Color32::from_rgb(29, 35, 41);

pub(super) const TEXT: Color32 = Color32::from_rgb(224, 228, 231);

pub(super) const TEXT_MUTED: Color32 = Color32::from_rgb(143, 151, 159);

pub(super) const TEXT_FAINT: Color32 = Color32::from_rgb(92, 101, 110);

pub(super) const SUCCESS: Color32 = Color32::from_rgb(98, 202, 122);

pub(super) fn configure_theme(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(INTER_FONT)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    context.set_fonts(fonts);
    egui_material_icons::initialize(context);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(7.0, 6.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.interact_size.y = 26.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.visuals.panel_fill = PANEL_BG;
        style.visuals.window_fill = PANEL_RAISED;
        style.visuals.extreme_bg_color = FIELD_BG;
        style.visuals.faint_bg_color = PANEL_RAISED;
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(25, 31, 37);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(55, 65, 74));
        style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

/// A panel's heading.
///
/// Actions live beneath the heading rather than being smuggled into this
/// shared decoration. The hierarchy's create menu, for example, now has both
/// an undoable spawn command and stable authored IDs behind it.
pub(super) fn panel_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
    });
    ui.add_space(3.0);
    ui.separator();
}

pub(super) fn search_field(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            ICON_SEARCH
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.add_sized(
            [ui.available_width() - 10.0, 28.0],
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .frame(egui::Frame::NONE),
        );
    });
}

pub(super) fn section_header(ui: &mut egui::Ui, icon: MaterialIcon, title: &str) {
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
    });
}

/// A property row whose value is a choice rather than a readout.
///
/// Shaped like [`property_label`] because it sits among those rows, and reading
/// as a label until you notice it responds is the point: what it says is the
/// state, and pressing it is how the state changes.
pub(super) fn property_toggle(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut bool,
    on: &str,
    off: &str,
) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            let text = if *value { on } else { off };
            let color = if *value { ACCENT } else { TEXT_MUTED };
            if ui
                .selectable_label(*value, RichText::new(text).size(11.0).color(color))
                .clicked()
            {
                *value = !*value;
            }
        });
    });
}

pub(super) fn property_label(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            ui.label(RichText::new(value).size(11.0).color(TEXT));
        });
    });
}

/// The name above a view when both are on screen at once.
///
/// A label rather than a tab: in this layout the view is already visible, so a
/// control that selects it would do nothing.
pub(super) fn view_title(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(12.0).color(TEXT));
    });
}

pub(super) fn icon_button(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    selected: bool,
    tip: &str,
) -> Response {
    ui.add_sized(
        [28.0, 28.0],
        egui::Button::new(icon.outlined().rich_text().size(17.0).color(if selected {
            ACCENT_BRIGHT
        } else {
            TEXT_MUTED
        }))
        .selected(selected)
        .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
        .stroke(Stroke::new(
            1.0,
            if selected { ACCENT } else { BORDER_SUBTLE },
        )),
    )
    .on_hover_text(tip)
}

/// Draws a small status dot.
///
/// The bundled Inter subset carries 192 glyphs and has no `U+25CF`, so a text
/// bullet renders as a missing-glyph box rather than a dot. Painting it keeps
/// the indicator independent of font coverage.
pub(super) fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (response, painter) = ui.allocate_painter(Vec2::splat(9.0), Sense::hover());
    painter.circle_filled(response.rect.center(), 3.0, color);
}
