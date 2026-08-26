//! The one place the editor decides what it looks like.
//!
//! Every colour, every gap, and every control height the editor draws with is
//! named here. A panel that picks its own grey is how an interface stops
//! matching itself, and the editor had eleven of them: each new panel copied a
//! `Color32::from_rgb` out of the panel beside it and then drifted.
//!
//! The palette is the documentation site's, adapted for dense tooling —
//! graphite ground, forge amber for anything the editor is doing on purpose —
//! and the metrics are chosen for a window someone works in all day rather than
//! a page someone reads once.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2,
};

/// The forge palette: graphite ground, warm metal for intent.
pub mod color {
    use eframe::egui::Color32;

    /// Behind everything, and between the panels.
    pub const INK: Color32 = Color32::from_rgb(8, 10, 13);

    /// The ordinary ground of a panel.
    pub const PANEL: Color32 = Color32::from_rgb(15, 19, 23);

    /// A panel's header strip, and anything else that sits above the panel.
    pub const HEADER: Color32 = Color32::from_rgb(18, 22, 27);

    /// A surface lifted off the panel: a card, a tile, a resting button.
    pub const RAISED: Color32 = Color32::from_rgb(22, 27, 33);

    /// A surface lifted further: a menu, a popup, a modal.
    pub const FLOATING: Color32 = Color32::from_rgb(26, 31, 38);

    /// The well a value is typed into.
    pub const WELL: Color32 = Color32::from_rgb(11, 14, 18);

    /// The line between two regions.
    pub const LINE: Color32 = Color32::from_rgb(37, 43, 50);

    /// The line inside a region, between two rows of one list.
    pub const LINE_SOFT: Color32 = Color32::from_rgb(27, 32, 38);

    /// Warm off-white, as the documentation site sets it.
    pub const TEXT: Color32 = Color32::from_rgb(232, 228, 220);

    /// A label beside a value, and body text that is not the point.
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(152, 160, 168);

    /// A unit, a hint, a count: readable, and not competing.
    pub const TEXT_FAINT: Color32 = Color32::from_rgb(104, 113, 122);

    /// The accent. Anything the editor is doing on purpose wears it.
    pub const FORGE: Color32 = Color32::from_rgb(233, 163, 58);

    /// The accent at full heat, for the one thing in a group that is active.
    pub const FORGE_BRIGHT: Color32 = Color32::from_rgb(255, 190, 85);

    /// The accent as a border on a resting surface.
    pub const FORGE_DIM: Color32 = Color32::from_rgb(138, 98, 32);

    /// The accent as a ground: a selected row, a lit toggle.
    pub const EMBER: Color32 = Color32::from_rgb(46, 34, 17);

    /// The accent as the faintest possible wash, for a hovered row.
    pub const EMBER_FAINT: Color32 = Color32::from_rgb(28, 33, 40);

    /// Something is wrong.
    pub const DANGER: Color32 = Color32::from_rgb(216, 90, 58);

    /// Something is wrong, said in text that still has to be read.
    pub const DANGER_TEXT: Color32 = Color32::from_rgb(255, 138, 148);

    /// Something needs attention but nothing has failed.
    pub const WARNING: Color32 = Color32::from_rgb(233, 193, 58);

    /// Something worked.
    pub const SUCCESS: Color32 = Color32::from_rgb(121, 182, 138);

    /// The three axes, wherever an axis is named — a gizmo arm, a vector
    /// field's letter, the view indicator. One set, so the X in the inspector
    /// is the X in the viewport.
    pub const AXIS_X: Color32 = Color32::from_rgb(214, 96, 92);
    pub const AXIS_Y: Color32 = Color32::from_rgb(127, 176, 105);
    pub const AXIS_Z: Color32 = Color32::from_rgb(91, 144, 214);

    /// The axis colours as they read on a resting control, where a full-strength
    /// letter beside a number would shout.
    pub const AXIS_X_DIM: Color32 = Color32::from_rgb(150, 74, 72);
    pub const AXIS_Y_DIM: Color32 = Color32::from_rgb(92, 124, 78);
    pub const AXIS_Z_DIM: Color32 = Color32::from_rgb(70, 104, 150);

    /// The three axis colours in order, for anything that indexes them.
    pub const AXES: [Color32; 3] = [AXIS_X, AXIS_Y, AXIS_Z];

    /// The dimmed axis colours in order.
    pub const AXES_DIM: [Color32; 3] = [AXIS_X_DIM, AXIS_Y_DIM, AXIS_Z_DIM];
}

/// How much room things take.
///
/// Every number an editor panel would otherwise invent: the height of a row,
/// the gap between a label and its value, how far a child indents. Compact on
/// purpose — this is a tool someone keeps five panels of open at once.
pub mod metric {
    /// The height of a list row: hierarchy, assets, console.
    pub const ROW_HEIGHT: f32 = 21.0;

    /// The height of an editable control.
    pub const CONTROL_HEIGHT: f32 = 21.0;

    /// The side of a square icon button on a toolbar.
    pub const TOOL_SIZE: f32 = 25.0;

    /// The height of a panel's header strip.
    ///
    /// Shared with the workspace tabs so that every strip along the top of a
    /// region lands on one baseline across the window. They were 26 and 30, and
    /// four pixels of disagreement across five panels reads as sloppiness even
    /// when nobody can say why.
    pub const HEADER_HEIGHT: f32 = 28.0;

    /// The height of a toolbar strip.
    pub const TOOLBAR_HEIGHT: f32 = 31.0;

    /// The window's own top bar, which carries the menus and the transport.
    pub const TOP_BAR_HEIGHT: f32 = 40.0;

    /// The window's status strip.
    pub const STATUS_HEIGHT: f32 = 24.0;

    /// The margin from a panel's edge to its content.
    pub const GUTTER: f32 = 8.0;

    /// The gap between two related controls.
    pub const GAP: f32 = 5.0;

    /// The gap between two groups of controls.
    pub const GROUP_GAP: f32 = 10.0;

    /// How far one level of a tree indents.
    pub const INDENT: f32 = 13.0;

    /// The label column in a property row, so every value in the inspector
    /// starts at the same x whichever section drew it.
    pub const LABEL_WIDTH: f32 = 82.0;

    /// The corner radius of a control. Small on purpose: this is a tool, and a
    /// pill-shaped button in a five-panel window reads as a web page.
    pub const RADIUS: u8 = 3;

    /// The corner radius of something small enough that 3 would look round.
    pub const RADIUS_TIGHT: u8 = 2;

    /// The gutter as a frame margin, which egui measures in whole points.
    pub const GUTTER_EDGE: i8 = 8;

    /// The width of the accent rule that marks a selected row.
    pub const SELECT_RULE: f32 = 2.0;
}

/// The sizes text comes in.
///
/// Four, and no more: a title, a body, a label, and a note. Every panel used to
/// pick its own between 9 and 13, which is how "12.0" and "11.0" ended up
/// meaning the same thing in two files.
pub mod text {
    /// A window-level heading: the brand, a modal's title.
    pub const TITLE: f32 = 13.0;

    /// A panel header, a component name, a tab.
    pub const HEADING: f32 = 11.0;

    /// Anything a person reads: a row name, a menu item, a value.
    pub const BODY: f32 = 12.0;

    /// A property label, a kind badge, a status line.
    pub const LABEL: f32 = 11.0;

    /// A unit, a count, a hint under a field.
    pub const NOTE: f32 = 10.0;
}

const INTER_FONT: &[u8] = include_bytes!("../../assets/Inter.ttf");

/// A hairline in the editor's border colour.
pub fn hairline() -> Stroke {
    Stroke::new(1.0, color::LINE)
}

/// A hairline in the colour used inside a region rather than around one.
pub fn hairline_soft() -> Stroke {
    Stroke::new(1.0, color::LINE_SOFT)
}

/// The editor's standard corner.
pub fn radius() -> CornerRadius {
    CornerRadius::same(metric::RADIUS)
}

/// The corner for something too small for [`radius`].
pub fn radius_tight() -> CornerRadius {
    CornerRadius::same(metric::RADIUS_TIGHT)
}

/// Installs the fonts, the palette, and the metrics on a context.
///
/// Called once at startup by the editor, and by any test that draws a widget:
/// a widget that reads `ui.visuals()` answers differently under egui's defaults,
/// so the tests configure the same context the editor runs.
pub fn install(context: &egui::Context) {
    install_fonts(context);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(apply);
}

fn install_fonts(context: &egui::Context) {
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
}

/// The palette and metrics, as egui's own style.
///
/// Stock controls are styled here rather than wrapped: a `ComboBox` inside a
/// component section is drawn by egui, and it should look like the editor
/// without every call site dressing it.
fn apply(style: &mut egui::Style) {
    let spacing = &mut style.spacing;
    spacing.item_spacing = Vec2::new(metric::GAP, 4.0);
    spacing.button_padding = Vec2::new(7.0, 3.0);
    spacing.interact_size.y = metric::CONTROL_HEIGHT;
    spacing.indent = metric::INDENT;
    spacing.menu_margin = Margin::symmetric(4, 4);
    spacing.window_margin = Margin::same(10);
    spacing.combo_height = 320.0;
    spacing.icon_width = 14.0;
    spacing.icon_width_inner = 8.0;
    spacing.scroll.bar_width = 8.0;
    spacing.scroll.floating = true;

    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(text::BODY, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(text::BODY, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(text::NOTE, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(text::TITLE, FontFamily::Proportional),
    );

    let visuals = &mut style.visuals;
    visuals.panel_fill = color::PANEL;
    visuals.window_fill = color::FLOATING;
    visuals.window_stroke = hairline();
    visuals.window_corner_radius = radius();
    visuals.menu_corner_radius = radius();
    visuals.extreme_bg_color = color::WELL;
    visuals.faint_bg_color = color::RAISED;
    visuals.code_bg_color = color::WELL;
    visuals.warn_fg_color = color::WARNING;
    visuals.error_fg_color = color::DANGER_TEXT;
    visuals.hyperlink_color = color::FORGE;
    visuals.weak_text_color = Some(color::TEXT_FAINT);
    visuals.selection.bg_fill = color::EMBER;
    visuals.selection.stroke = Stroke::new(1.0, color::FORGE);
    visuals.text_cursor.stroke = Stroke::new(1.0, color::FORGE);
    // A control that cannot be used should read as unavailable rather than as
    // low-contrast text: egui's default barely dims, so a disabled Save looked
    // like an enabled one someone had chosen a bad colour for.
    visuals.disabled_alpha = 0.42;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.button_frame = true;
    visuals.indent_has_left_vline = false;
    visuals.striped = false;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = color::PANEL;
    widgets.noninteractive.weak_bg_fill = color::PANEL;
    widgets.noninteractive.bg_stroke = hairline_soft();
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, color::TEXT_MUTED);
    widgets.noninteractive.corner_radius = radius();

    widgets.inactive.bg_fill = color::RAISED;
    widgets.inactive.weak_bg_fill = color::RAISED;
    widgets.inactive.bg_stroke = Stroke::new(1.0, color::LINE_SOFT);
    widgets.inactive.fg_stroke = Stroke::new(1.0, color::TEXT_MUTED);
    widgets.inactive.corner_radius = radius();

    widgets.hovered.bg_fill = Color32::from_rgb(31, 37, 45);
    widgets.hovered.weak_bg_fill = Color32::from_rgb(31, 37, 45);
    widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(58, 67, 77));
    widgets.hovered.fg_stroke = Stroke::new(1.0, color::TEXT);
    widgets.hovered.corner_radius = radius();
    // Widgets that grow on hover shift everything beside them by a pixel, which
    // in a dense panel reads as the row twitching.
    widgets.hovered.expansion = 0.0;

    widgets.active.bg_fill = color::EMBER;
    widgets.active.weak_bg_fill = color::EMBER;
    widgets.active.bg_stroke = Stroke::new(1.0, color::FORGE);
    widgets.active.fg_stroke = Stroke::new(1.0, color::FORGE_BRIGHT);
    widgets.active.corner_radius = radius();
    widgets.active.expansion = 0.0;

    widgets.open.bg_fill = color::EMBER;
    widgets.open.weak_bg_fill = color::EMBER;
    widgets.open.bg_stroke = Stroke::new(1.0, color::FORGE_DIM);
    widgets.open.fg_stroke = Stroke::new(1.0, color::TEXT);
    widgets.open.corner_radius = radius();
}
