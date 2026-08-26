//! The editor's buttons, by what pressing one means.
//!
//! Not a rename of `ui.button`. Each of these centralises something a call site
//! would otherwise get wrong on its own: the size a toolbar icon is, what
//! "selected" looks like, and — the one that matters — that a destructive
//! action is drawn differently from an ordinary one, so nobody has to read the
//! label to know which is which.

use eframe::egui::{self, Color32, Response, RichText, Sense, Stroke, StrokeKind, Vec2};
use egui_material_icons::MaterialIcon;

use crate::ui::theme::{color, metric, radius, text};

/// What pressing a button means, which is what decides how it looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Intent {
    /// The ordinary case: a control on a surface.
    #[default]
    Normal,
    /// The one thing this group is for.
    Primary,
    /// Something that throws work away.
    Danger,
    /// A control with no frame until it is pointed at, for dense strips where
    /// a box per control would be more boxes than controls.
    Quiet,
}

impl Intent {
    fn fill(self, selected: bool) -> Color32 {
        match self {
            _ if selected => color::EMBER,
            Self::Primary => color::FORGE_BRIGHT,
            Self::Quiet => Color32::TRANSPARENT,
            Self::Normal | Self::Danger => color::RAISED,
        }
    }

    fn stroke(self, selected: bool) -> Stroke {
        match self {
            _ if selected => Stroke::new(1.0, color::FORGE),
            Self::Primary => Stroke::new(1.0, color::FORGE),
            Self::Quiet => Stroke::NONE,
            Self::Danger => Stroke::new(1.0, color::DANGER.gamma_multiply(0.6)),
            Self::Normal => Stroke::new(1.0, color::LINE_SOFT),
        }
    }

    fn foreground(self, selected: bool) -> Color32 {
        match self {
            _ if selected => color::FORGE_BRIGHT,
            // Dark ink on hot metal: the one button in the window that inverts.
            Self::Primary => Color32::from_rgb(26, 20, 9),
            Self::Danger => color::DANGER_TEXT,
            Self::Normal | Self::Quiet => color::TEXT_MUTED,
        }
    }
}

/// A square icon button, the size every toolbar in the editor uses.
pub fn icon(ui: &mut egui::Ui, glyph: MaterialIcon, selected: bool, tip: &str) -> Response {
    icon_with_intent(ui, glyph, selected, Intent::Normal, tip)
}

/// A square icon button that says what pressing it means.
pub fn icon_with_intent(
    ui: &mut egui::Ui,
    glyph: MaterialIcon,
    selected: bool,
    intent: Intent,
    tip: &str,
) -> Response {
    let response = ui.add_sized(
        [metric::TOOL_SIZE, metric::TOOL_SIZE],
        egui::Button::new(
            glyph
                .outlined()
                .rich_text()
                .size(16.0)
                .color(intent.foreground(selected)),
        )
        .fill(intent.fill(selected))
        .stroke(intent.stroke(selected))
        .corner_radius(radius()),
    );
    tipped(response, tip)
}

/// A small icon button that lives inside a row rather than on a toolbar.
///
/// Frameless until hovered, because a row already has a shape and a second box
/// inside it is one box too many.
pub fn row_icon(ui: &mut egui::Ui, glyph: MaterialIcon, intent: Intent, tip: &str) -> Response {
    let response = ui.add(
        egui::Button::new(
            glyph
                .outlined()
                .rich_text()
                .size(14.0)
                .color(intent.foreground(false)),
        )
        .frame(false)
        .min_size(Vec2::splat(18.0)),
    );
    tipped(response, tip)
}

/// A button with words on it.
pub fn labelled(ui: &mut egui::Ui, label: &str, intent: Intent, tip: &str) -> Response {
    let response = ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(text::LABEL)
                .color(intent.foreground(false)),
        )
        .fill(intent.fill(false))
        .stroke(intent.stroke(false))
        .corner_radius(radius())
        .min_size(Vec2::new(0.0, metric::CONTROL_HEIGHT)),
    );
    tipped(response, tip)
}

/// One choice in a strip of mutually exclusive ones.
///
/// Drawn as one continuous control rather than as separate buttons: the group
/// paints its own well and each segment fills its share of it, so the reader
/// sees a single switch with a position rather than three buttons that happen
/// to be adjacent.
///
/// Painted into a region it measures for itself. Built out of nested layouts it
/// inherited whichever direction its parent was laying out in — inside the
/// project browser's right-aligned toolbar it stretched to the full width of
/// the panel, and inside the viewport's it pushed its own labels off screen.
pub struct Segmented<'a, T> {
    current: &'a mut T,
    options: Vec<(T, &'a str, &'a str)>,
}

impl<'a, T: Copy + PartialEq> Segmented<'a, T> {
    pub fn new(current: &'a mut T) -> Self {
        Self {
            current,
            options: Vec::new(),
        }
    }

    /// Adds a choice: its value, its label, and what it does on hover.
    #[must_use]
    pub fn option(mut self, value: T, label: &'a str, tip: &'a str) -> Self {
        self.options.push((value, label, tip));
        self
    }

    /// Draws the strip and reports whether the choice changed.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let Self { current, options } = self;
        if options.is_empty() {
            return false;
        }
        let font = egui::FontId::proportional(text::LABEL);
        let galleys: Vec<_> = options
            .iter()
            .map(|(_, label, _)| {
                // Laid out with the placeholder colour so that the tint
                // handed to `Painter::galley` is the one that takes effect: a
                // galley built with a real colour keeps it, and every segment
                // was drawn in the selected segment's white.
                ui.painter()
                    .layout_no_wrap((*label).to_owned(), font.clone(), Color32::PLACEHOLDER)
            })
            .collect();
        let segment_height = metric::CONTROL_HEIGHT - 3.0;
        let widths: Vec<f32> = galleys
            .iter()
            .map(|galley| galley.size().x + 16.0)
            .collect();
        // Counted through `u8`: no switch in the editor has more segments than
        // that, and the conversion is then exact rather than lossy.
        let gaps = f32::from(u8::try_from(widths.len().saturating_sub(1)).unwrap_or(u8::MAX));
        let total = widths.iter().sum::<f32>() + 2.0 * gaps + 6.0;
        // The whole strip is allocated first so that its own response carries
        // an id egui made unique. Deriving the segments from `ui.id()` instead
        // gave every switch in a panel the same ids, and egui painted its
        // duplicate-id warning across the inspector's Transform section.
        let (rect, strip) = ui.allocate_exact_size(
            Vec2::new(total, metric::CONTROL_HEIGHT + 2.0),
            Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, radius(), color::WELL);
        painter.rect_stroke(
            rect,
            radius(),
            Stroke::new(1.0, color::LINE_SOFT),
            StrokeKind::Inside,
        );

        let mut changed = false;
        let mut left = rect.left() + 3.0;
        for (index, ((value, _, tip), galley)) in options.into_iter().zip(galleys).enumerate() {
            let segment = egui::Rect::from_min_size(
                egui::Pos2::new(left, rect.center().y - segment_height / 2.0),
                Vec2::new(widths[index], segment_height),
            );
            left += widths[index] + 2.0;
            let selected = *current == value;
            let response = tipped(
                ui.interact(segment, strip.id.with(index), Sense::click()),
                tip,
            );
            if selected {
                painter.rect_filled(segment, crate::ui::theme::radius_tight(), color::EMBER);
            } else if response.hovered() {
                painter.rect_filled(segment, crate::ui::theme::radius_tight(), color::RAISED);
            }
            painter.galley(
                egui::Pos2::new(
                    segment.center().x - galley.size().x / 2.0,
                    segment.center().y - galley.size().y / 2.0,
                ),
                galley,
                if selected {
                    color::FORGE_BRIGHT
                } else {
                    color::TEXT_FAINT
                },
            );
            if response.clicked() {
                *current = value;
                changed = true;
            }
        }
        changed
    }
}

/// A hairline box drawn around a rect, for grouping without a fill.
pub fn outline(ui: &egui::Ui, rect: egui::Rect, stroke: Stroke) {
    ui.painter()
        .rect_stroke(rect, radius(), stroke, StrokeKind::Inside);
}

/// Attaches a hover explanation, unless there is nothing to explain.
///
/// An empty tip is a real case — a labelled button whose label already says it —
/// and `on_hover_text("")` draws an empty tooltip box rather than none.
fn tipped(response: Response, tip: &str) -> Response {
    if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    }
}

/// A patch of a row that answers the pointer without drawing anything.
///
/// Used by the list widgets, which paint their own background from the
/// interaction state rather than letting a button paint one.
pub fn row_sense(ui: &mut egui::Ui, height: f32) -> (egui::Rect, Response) {
    let width = ui.available_width();
    ui.allocate_exact_size(Vec2::new(width, height), Sense::click())
}
