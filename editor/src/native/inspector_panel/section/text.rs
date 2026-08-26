//! `sindri.ui.text`: the string, its font, and its size.

use eframe::egui::{self, Align, Layout, RichText};
use serde_json::Value;

use super::super::super::{PROBLEM, TEXT_MUTED};

/// The two text fields whose meaning is richer than their JSON shape.
///
/// Content is multiline gameplay/UI copy, and a font is a project-owned asset
/// reference. Leaving either as an ordinary one-line string technically edits
/// the payload but makes the editor less useful than editing JSON by hand.
pub(super) fn text_section(ui: &mut egui::Ui, payload: &mut Value, fonts: &[String]) {
    let mut content = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Text").size(11.0).color(TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let width = (ui.available_width() - 7.0).max(120.0);
        if ui
            .add_sized(
                [width, 76.0],
                egui::TextEdit::multiline(&mut content)
                    .desired_rows(3)
                    .hint_text("Text shown in the game"),
            )
            .changed()
        {
            payload["text"] = Value::String(content);
        }
    });

    let current = payload
        .get("font")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Font").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("text-font-asset")
                .selected_text(if chosen.is_empty() {
                    "Choose a font"
                } else {
                    chosen.as_str()
                })
                .width(190.0)
                .show_ui(ui, |ui| {
                    for font in fonts {
                        ui.selectable_value(&mut chosen, font.clone(), font);
                    }
                });
        });
    });
    if chosen != current {
        payload["font"] = Value::String(chosen.clone());
    }

    let missing = fonts.is_empty() || chosen.is_empty() || !fonts.contains(&chosen);
    if missing {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            let message = if fonts.is_empty() {
                "Add an OpenType font to the project before adding text."
            } else {
                "The selected font is not present in this project."
            };
            ui.label(RichText::new(message).size(9.0).color(PROBLEM));
        });
    }
}
