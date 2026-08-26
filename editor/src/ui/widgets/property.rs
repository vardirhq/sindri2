//! One labelled thing in the inspector.
//!
//! Every property row in the editor used to be its own `ui.horizontal` with its
//! own `add_space`, its own font size, and its value pushed to the right edge by
//! a right-to-left layout. Values of different widths therefore started at
//! different places down a single component, and a label two words long moved
//! the value that followed it. This is one row: a fixed label column, a value
//! area that begins at the same x on every row in the window, and one place to
//! change either.

use eframe::egui::{self, Align, Layout, Response, RichText, Sense, Vec2};

use crate::ui::theme::{color, metric, text};

/// Roughly how many characters the label column fits before it truncates.
const LABEL_CHARS: usize = 15;

/// A labelled row, with whatever control belongs beside the label.
#[derive(Clone, Copy)]
pub struct Property<'a> {
    label: &'a str,
    indent: f32,
    tip: Option<&'a str>,
    modified: bool,
}

impl<'a> Property<'a> {
    pub const fn new(label: &'a str) -> Self {
        Self {
            label,
            indent: 0.0,
            tip: None,
            modified: false,
        }
    }

    /// How far in this row sits, for a value nested inside another.
    #[must_use]
    pub const fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    /// What this row means, said on hover rather than in the layout.
    #[must_use]
    pub const fn tip(mut self, tip: &'a str) -> Self {
        self.tip = Some(tip);
        self
    }

    /// Whether this value is one the author set rather than one the default
    /// supplied.
    ///
    /// Marked with a dot in the margin, because "did I change this?" is the
    /// question an inspector is asked most often and the panel already knows
    /// the answer: it draws the registry's blank filled out with what the
    /// component stored.
    #[must_use]
    pub const fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }

    /// Draws the row, with `add` filling the value column.
    pub fn show<R>(self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(metric::GUTTER + self.indent);
            self.marker(ui);
            self.name(ui);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                add(ui)
            })
            .inner
        })
        .inner
    }

    fn marker(&self, ui: &mut egui::Ui) {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(4.0, metric::CONTROL_HEIGHT), Sense::hover());
        if self.modified {
            ui.painter().circle_filled(rect.center(), 2.0, color::FORGE);
        }
    }

    fn name(&self, ui: &mut egui::Ui) -> Response {
        // The column narrows as rows nest, so a nested value still starts where
        // its siblings do rather than being pushed off the panel.
        let width = (metric::LABEL_WIDTH - self.indent).max(52.0);
        // A long name is cut to keep the value column straight, so it is also
        // readable on hover. A script's `@export turns_per_second` is exactly
        // the case: the field is worth naming in full and the column is not
        // worth widening for it.
        let elided = self.label.chars().count() > LABEL_CHARS;
        let response = ui
            .allocate_ui_with_layout(
                Vec2::new(width, metric::CONTROL_HEIGHT),
                Layout::left_to_right(Align::Center),
                |ui| {
                    // The column is only a column if it holds its width. egui
                    // allocates a child region by what its contents ended up
                    // measuring, not by what was asked for, so without this the
                    // label column was as wide as each label — and "Scale" put
                    // its fields eleven pixels left of "Position"'s.
                    ui.set_min_width(width);
                    ui.add(
                        egui::Label::new(
                            RichText::new(self.label)
                                .size(text::LABEL)
                                .color(color::TEXT_MUTED),
                        )
                        .selectable(false)
                        .truncate(),
                    )
                },
            )
            .inner;
        match self.tip {
            Some(tip) => response.on_hover_text(tip),
            None if elided => response.on_hover_text(self.label),
            None => response,
        }
    }
}

/// Puts a row's own controls at the far end of it.
///
/// A remove button beside its label reads as part of the label; at the end of
/// the row it reads as what the row can do, and lines up with every other row's.
pub fn trailing<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(metric::GUTTER);
        add(ui)
    })
    .inner
}

/// A row whose value is text the editor wrote rather than a control.
pub fn readout(ui: &mut egui::Ui, label: &str, value: &str, why: Option<&str>) {
    let mut row = Property::new(label);
    if let Some(why) = why {
        row = row.tip(why);
    }
    row.show(ui, |ui| {
        let shown = ui.add(
            egui::Label::new(
                RichText::new(value)
                    .size(text::LABEL)
                    .color(color::TEXT_FAINT),
            )
            .truncate(),
        );
        if let Some(why) = why {
            shown.on_hover_text(why);
        }
    });
}

/// A row whose value is one of two words, and pressing it is how it changes.
pub fn toggle(ui: &mut egui::Ui, label: &str, value: &mut bool, on: &str, off: &str) -> bool {
    let mut changed = false;
    Property::new(label).show(ui, |ui| {
        let mut chosen = *value;
        if super::button::Segmented::new(&mut chosen)
            .option(true, on, "")
            .option(false, off, "")
            .show(ui)
        {
            *value = chosen;
            changed = true;
        }
    });
    changed
}

/// The width a control should take to fill the rest of a property row.
///
/// One answer, so a text field and a picker on two consecutive rows end at the
/// same place instead of each guessing a desired width.
pub fn value_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - metric::GUTTER).max(60.0)
}

/// What a `ComboBox` should be told when it is meant to fill the value column.
///
/// egui measures a combo by its *content*, then adds its padding and its arrow
/// on top. Handing it [`value_width`] therefore builds a control wider than the
/// row it sits in, and a panel full of them pushed the inspector's labels off
/// its own left edge.
pub fn picker_width(ui: &egui::Ui) -> f32 {
    (value_width(ui) - PICKER_FURNITURE).max(48.0)
}

/// How much a `ComboBox` adds around whatever width it is given: button padding
/// at both ends, the gap before the arrow, and the arrow itself.
pub const PICKER_FURNITURE: f32 = 30.0;
