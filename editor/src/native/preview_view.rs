//! Showing a file's contents where an entity's inspector would be.
//!
//! Read-only, and saying so. An editor that opens a script in a text box is
//! promising to be a code editor — syntax, errors at the line they are on,
//! find, an undo stack of its own — and half of that is worse than none. What
//! this answers is the question the browser could not: what is in this file.

use eframe::egui::{self, RichText};

use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{panel, property};

use super::EditorApp;

impl EditorApp {
    /// The inspector, showing a file rather than an entity.
    pub(super) fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let Some(preview) = &self.preview else {
            return;
        };
        panel::body(ui, |ui| {
            ui.label(
                RichText::new(preview.name())
                    .size(text::BODY)
                    .color(color::TEXT),
            );
        });
        let Ok(body) = preview.body() else {
            let why = preview.body().err().unwrap_or_default();
            panel::problem(ui, why);
            return;
        };
        property::readout(
            ui,
            "Lines",
            &preview.lines().to_string(),
            Some("Shown as it is on disk; the editor does not edit source"),
        );
        if preview.truncated() {
            panel::note(ui, "Longer than the preview reads — shown to its cut");
        }
        ui.add_space(metric::GAP);
        // Monospace and horizontally scrolling, because source is written in
        // columns and wrapping it puts a continuation where a statement was.
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(metric::GUTTER);
                    ui.add(
                        egui::Label::new(
                            RichText::new(body)
                                .font(egui::FontId::monospace(text::LABEL))
                                .color(color::TEXT_MUTED),
                        )
                        .selectable(true)
                        .extend(),
                    );
                });
            });
    }
}
