//! `sindri.ui.text`: the string, its font, and its size.

use eframe::egui;
use serde_json::Value;

use crate::ui::theme::{color, metric};
use crate::ui::widgets::{panel, property};

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
    // The copy gets the panel's whole width rather than a value column: it is
    // the one field on this component someone writes a sentence into.
    property::Property::new("Text").show(ui, |_| {});
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        let width = (ui.available_width() - metric::GUTTER).max(120.0);
        if ui
            .add_sized(
                [width, 72.0],
                egui::TextEdit::multiline(&mut content)
                    .desired_rows(3)
                    .hint_text("Text shown in the game"),
            )
            .changed()
        {
            payload["text"] = Value::String(content);
        }
    });
    ui.add_space(4.0);

    let current = payload
        .get("font")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    property::Property::new("Font").show(ui, |ui| {
        egui::ComboBox::from_id_salt("text-font-asset")
            .selected_text(if chosen.is_empty() {
                "Choose a font"
            } else {
                chosen.as_str()
            })
            .width(property::picker_width(ui))
            .show_ui(ui, |ui| {
                for font in fonts {
                    ui.selectable_value(&mut chosen, font.clone(), font);
                }
            });
    });
    if chosen != current {
        payload["font"] = Value::String(chosen.clone());
    }

    let missing = fonts.is_empty() || chosen.is_empty() || !fonts.contains(&chosen);
    if missing {
        panel::problem(
            ui,
            if fonts.is_empty() {
                "Add an OpenType font to the project before adding text."
            } else {
                "The selected font is not present in this project."
            },
        );
    }
    let _ = color::TEXT;
}
