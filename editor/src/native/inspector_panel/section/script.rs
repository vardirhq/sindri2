//! `sindri.script`: the exports a script declares, typed.

use eframe::egui::{self, RichText};
use serde_json::Value;
use sindri_decay::ScriptValue;

use crate::{inspector, scripts::SceneScripts};

use super::super::super::{TEXT_MUTED, theme::property_label};
use super::super::rows::value_row;

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
        property_label(ui, "Properties", "waiting for the script");
        return;
    };
    if exports.is_empty() {
        property_label(ui, "Properties", "none declared");
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
        value_row(ui, &export.name, &mut value, 10.0);
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
        } else if !authored {
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.label(
                    RichText::new(format!(
                        "default{}",
                        export
                            .type_name
                            .as_ref()
                            .map_or_else(String::new, |name| format!(" · {name}"))
                    ))
                    .size(9.0)
                    .color(TEXT_MUTED),
                );
            });
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
pub(super) fn script_value_json(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::Number(number) => Value::from(*number),
        ScriptValue::Bool(flag) => Value::Bool(*flag),
        ScriptValue::String(text) => Value::String(text.clone()),
        ScriptValue::Reference(_) | ScriptValue::Null | ScriptValue::Unit => Value::Null,
    }
}
