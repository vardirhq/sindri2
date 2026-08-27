//! Showing a file's contents where an entity's inspector would be.
//!
//! Read-only, and saying so. An editor that opens a script in a text box is
//! promising to be a code editor — syntax, errors at the line they are on,
//! find, an undo stack of its own — and half of that is worse than none. What
//! this answers is the question the browser could not: what is in this file.

use eframe::egui::{self, RichText};

use crate::typeface;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{button, button::Intent, panel, property};

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

    /// The inspector, offering to play a clip.
    ///
    /// A button and nothing else. What someone is deciding is whether this is
    /// the right sound, and a waveform they cannot scrub is a picture of an
    /// answer they get faster by listening.
    pub(super) fn audition_panel(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.heard.clone() else {
            return;
        };
        panel::body(ui, |ui| {
            ui.label(
                RichText::new(named(&path))
                    .size(text::BODY)
                    .color(color::TEXT),
            );
        });
        ui.add_space(metric::GAP);
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER);
            if button::labelled(ui, "Play", Intent::Primary, "Hear this clip once").clicked()
                && let Err(error) = self.audition.play(&path)
            {
                self.report(error.to_string());
            }
            if button::labelled(ui, "Stop", Intent::Quiet, "Silence it").clicked() {
                self.audition.stop();
            }
        });
        ui.add_space(metric::GAP);
        panel::note(
            ui,
            "Played by the editor, not by the scene: nothing here reaches a running world.",
        );
    }

    /// The inspector, showing a sample of a font.
    ///
    /// Drawn in the face itself, which is the whole point: a project holding
    /// four typefaces gave four rows that differed only in what they were
    /// called.
    pub(super) fn typeface_panel(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.shown_font.clone() else {
            return;
        };
        self.typeface.show(ui.ctx(), &path);
        panel::body(ui, |ui| {
            ui.label(
                RichText::new(named(&path))
                    .size(text::BODY)
                    .color(color::TEXT),
            );
        });
        let Some(family) = self.typeface.family(ui.ctx()) else {
            if self.typeface.pending() {
                // Registered, and bound at the start of the next frame. Asking
                // for it now is what panicked with "is not bound to any fonts".
                ui.ctx().request_repaint();
                panel::note(ui, "Reading the font…");
            } else {
                // A `.ttf` that is not a font is exactly what a preview exists
                // to reveal, so it says so rather than drawing the sample in
                // the editor's own face and looking fine.
                panel::problem(ui, "This file is not a font the editor can read.");
            }
            return;
        };
        ui.add_space(metric::GAP);
        for size in [26.0_f32, 15.0] {
            ui.horizontal(|ui| {
                ui.add_space(metric::GUTTER);
                ui.add(
                    egui::Label::new(
                        RichText::new(typeface::SAMPLE)
                            .font(egui::FontId::new(size, family.clone()))
                            .color(color::TEXT),
                    )
                    .wrap(),
                );
            });
            ui.add_space(metric::GAP);
        }
        panel::note(
            ui,
            "A component naming this font draws it through the engine's own text renderer.",
        );
    }
}

/// What to call a path in one line.
fn named(path: &std::path::Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}
