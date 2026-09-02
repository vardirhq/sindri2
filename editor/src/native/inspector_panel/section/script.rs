//! `sindri.script`: the exports a script declares, typed.

use eframe::egui::{self, RichText};
use serde_json::Value;
use sindri_decay::ScriptValue;

use crate::ui::theme::{color, text};
use crate::ui::widgets::property;
use crate::{inspector, scripts::SceneScripts};

use super::super::rows::{Authored, text_row, value_row};

/// Which script of the chosen source this entity runs.
///
/// A name typed by hand is a name that compiles to nothing: the source
/// declares the scripts it has, and anything else is a component that loads and
/// then does not run, reported once a frame to a console. So the names come
/// from the compiled source, and the field falls back to a text box only while
/// there is nothing to offer — a source still loading, or one that will not
/// compile, where refusing to show the stored name would hide the thing that
/// needs fixing.
pub(super) fn script_choice_row(ui: &mut egui::Ui, payload: &mut Value, scripts: &SceneScripts) {
    let source = payload
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let declared = scripts.declared(&source);
    let current = payload
        .get("script")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if declared.is_empty() {
        let mut typed = current.clone();
        if text_row(ui, "Script", &mut typed, 10.0) {
            payload["script"] = Value::String(typed);
        }
        return;
    }
    let mut chosen = current.clone();
    property::Property::new("Script")
        .tip("Which of the source's scripts this entity runs")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("script-container")
                .selected_text(
                    RichText::new(&chosen)
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                )
                .width(property::picker_width(ui))
                .show_ui(ui, |ui| {
                    for name in &declared {
                        ui.selectable_value(&mut chosen, name.clone(), name);
                    }
                });
        });
    if chosen != current {
        payload["script"] = Value::String(chosen);
    }
}

/// A script's `@export` fields, drawn from what the script declared.
///
/// This is the capability that justified a statically typed language: the panel
/// knows a field exists, what it is called, what type it is, and what it starts
/// as, without running anything. A field the scene has not set shows its
/// default and says so.
pub(super) fn script_exports_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    scripts: &SceneScripts,
) {
    let source = payload.get("source").and_then(Value::as_str).unwrap_or("");
    let script = payload.get("script").and_then(Value::as_str).unwrap_or("");
    let Some(exports) = scripts.exports(source, script) else {
        // Not the same as having no properties, and saying so matters: a panel
        // that showed nothing would look like a script with nothing to author.
        property::readout(
            ui,
            "Properties",
            "waiting for the script",
            Some("The source has not compiled yet, so what it exports is not known"),
        );
        return;
    };
    if exports.is_empty() {
        property::readout(
            ui,
            "Properties",
            "none declared",
            Some("This script has no @export fields to author"),
        );
        return;
    }

    for export in exports {
        let stored = payload
            .get("properties")
            .and_then(|properties| properties.get(&export.name))
            .cloned();
        let authored = stored.is_some();
        let mut value = stored.unwrap_or_else(|| script_value_json(&export.default));
        let label = inspector::humanize(&export.name);

        let before = value.clone();
        // Marked when the scene set it: a script export showing its default is
        // one the author has not touched, and the dot says so without a line of
        // prose under every row.
        value_row(ui, &export.name, &mut value, 0.0, Authored::of(authored));
        if value != before {
            // Setting a property is what puts it in the scene: a field left
            // alone stays absent, so a scene records the author's choices
            // rather than a copy of every default.
            let properties = payload
                .as_object_mut()
                .expect("a script component is an object")
                .entry("properties")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(properties) = properties.as_object_mut() {
                properties.insert(export.name.clone(), value);
            }
        } else if let Some(type_name) = export.type_name.as_ref() {
            // The declared type, said once under the row rather than repeated
            // in every label: it is what the script guarantees about the field,
            // which is the whole reason the language is typed.
            crate::ui::widgets::section::caption(ui, &format!("default · {type_name}"));
        }
        let _ = label;
    }
}

/// A Decay value as the JSON a scene stores.
///
/// A reference stores as null, because it names a runtime handle and runtime
/// handles are never serialized: writing one to a scene would produce a file
/// that means something different the next time it is opened. An `@export` of
/// an entity is not authorable for that reason, and the inspector shows it as
/// empty rather than as a number nobody can act on.
///
/// A collection stores as null for a sharper reason: there is no literal for
/// one and nothing but the host makes one, so an authored collection is not a
/// thing that can exist. A field declared `Array<T>` has no authorable value
/// and the panel shows it as empty, which is the truth.
pub(super) fn script_value_json(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::Number(number) => Value::from(*number),
        ScriptValue::Bool(flag) => Value::Bool(*flag),
        ScriptValue::String(text) => Value::String(text.clone()),
        ScriptValue::Reference(_)
        | ScriptValue::Array(_)
        | ScriptValue::Null
        | ScriptValue::Unit => Value::Null,
    }
}
