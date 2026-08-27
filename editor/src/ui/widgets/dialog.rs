//! The shape a question wears when the editor has to stop and ask one.
//!
//! Two things ask: unsaved work about to be thrown away, and a file about to
//! be removed from disk. They are different questions with different answers —
//! one can offer to save first, the other has nothing to offer — so what is
//! shared here is the shape and not the buttons: a modal of one width, an icon
//! in the colour of the consequence, a title, and the question wrapped under
//! it. The caller adds whatever answering it means.

use eframe::egui::{self, RichText};

use crate::ui::theme::{color, text};

/// How wide a question opens.
///
/// Fixed, so two different questions do not arrive as two different windows.
const DIALOG_WIDTH: f32 = 372.0;

/// Asks something, and hands the caller the row its answers go in.
///
/// `id` is the modal's own, so two questions cannot be mistaken for one.
pub fn ask<R>(
    context: &egui::Context,
    id: &str,
    title: &str,
    question: &str,
    answers: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Modal::new(egui::Id::new(id))
        .show(context, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.horizontal(|ui| {
                ui.label(
                    crate::ui::icons::REMOVE
                        .outlined()
                        .rich_text()
                        .size(17.0)
                        .color(color::DANGER),
                );
                ui.label(
                    RichText::new(title)
                        .strong()
                        .size(text::TITLE)
                        .color(color::TEXT),
                );
            });
            ui.add_space(7.0);
            ui.add(
                egui::Label::new(
                    RichText::new(question)
                        .size(text::BODY)
                        .color(color::TEXT_MUTED),
                )
                .wrap(),
            );
            ui.add_space(14.0);
            ui.horizontal(answers).inner
        })
        .inner
}
