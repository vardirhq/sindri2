//! What a project may author, as a description something else can read.
//!
//! Read from the registry `SceneExtractor` starts with, which is where the
//! built-in `sindri.*` components are registered once for the editor, the
//! runtime, and scene validation alike.
//!
//! The registry's own distinction is preserved here rather than flattened,
//! because it is exactly the thing a tool needs and exactly the thing that has
//! been got wrong before. **Fields** are what a component *has*. A **default
//! payload** is what a *fresh* one is, and only some types have one: a
//! component naming a font, a sheet, a clip, or another entity has no blank the
//! engine can honestly invent. So `addable` is not an opinion this file forms —
//! it is whether the engine can produce a valid payload with nothing else to go
//! on, which is the difference between an authoring step that works and one
//! that is refused after it has been written down.

use serde_json::{Value, json};
use sindri_core::{ComponentSchemaRegistry, FieldMeaning, SCENE_FORMAT_VERSION};
use sindri_scene::SceneExtractor;

use crate::CapabilitiesError;

/// Describes what this engine build can author.
pub(crate) fn describe() -> Result<Value, CapabilitiesError> {
    let extractor = SceneExtractor::new()?;
    let registry = extractor.components();

    let mut components: Vec<Value> = registry
        .registered_components()
        .map(|metadata| {
            let type_name = metadata.type_name.as_str();
            let default_payload = registry.default_payload(type_name);
            json!({
                "type_name": type_name,
                "display_name": metadata.display_name,
                "schema_version": metadata.schema_version,
                "fields": registry.fields(type_name),
                "meanings": meanings(registry, type_name),
                "default_payload": default_payload,
                "addable": default_payload.is_some(),
            })
        })
        .collect();
    components.sort_by(|left, right| sort_key(left).cmp(sort_key(right)));

    Ok(json!({
        "schema_version": crate::SCHEMA_VERSION,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "scene_format_version": SCENE_FORMAT_VERSION,
        "generated_by": crate::REGENERATE_COMMAND,
        "about": "Every component this engine build registers. `fields` is what \
    the component has; `default_payload` is what a fresh one is, and is null for a \
    type with no honest blank — one naming an asset the engine cannot invent. \
    `addable` says whether a tool can add one without being given anything else. \
    `meanings` says what a field is *for* where the shape alone cannot: which \
    asset kind it names, which spellings it accepts, that it is a colour, an \
    angle, a bounded number, a collision mask, or another entity. A path is \
    dotted, and `[]` descends into a list, so `pieces[].friction` describes \
    every piece. A choice that also carries `variants` decides the shape of what \
    holds it: each spelling names the fields the object has when the tag says \
    that word, and writing the word without them is a payload the engine \
    refuses.",
        "components": components,
    }))
}

/// What a component's fields mean, as a path-keyed object.
///
/// The registry knows this now, so every tool reading this file knows it too —
/// which is the half of the old arrangement that was missing: the editor's
/// guesses lived in the editor, where nothing else could see them.
fn meanings(registry: &ComponentSchemaRegistry, type_name: &str) -> Value {
    let described: serde_json::Map<String, Value> = registry
        .meanings(type_name)
        .map(|(path, meaning)| {
            let mut described = json!({ "kind": meaning.kind() });
            match meaning {
                FieldMeaning::Asset(kind) => {
                    described["asset"] = json!(kind.as_str());
                }
                FieldMeaning::Choice(options) => {
                    described["options"] = json!(options);
                    // Where a choice decides more than itself, what each
                    // spelling makes the component hold. A tool writing one of
                    // these has to write the rest of them with it.
                    if let Some(variants) = registry.variants(type_name, path) {
                        described["variants"] = variants
                            .iter()
                            .map(|(name, template)| ((*name).to_owned(), template.clone()))
                            .collect::<serde_json::Map<String, Value>>()
                            .into();
                    }
                }
                FieldMeaning::Range { min, max } => {
                    described["min"] = json!(min);
                    // An unbounded end is written as null rather than as a
                    // float nothing can compare against.
                    described["max"] = if max.is_finite() {
                        json!(max)
                    } else {
                        Value::Null
                    };
                }
                FieldMeaning::Colour
                | FieldMeaning::Angle
                | FieldMeaning::Mask
                | FieldMeaning::Entity => {}
            }
            (path.to_owned(), described)
        })
        .collect();
    Value::Object(described)
}

/// Sorted by type name, because the registry holds these in a hash map and a
/// generated file whose order changes between runs cannot be diffed.
fn sort_key(component: &Value) -> &str {
    component
        .get("type_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::describe;
    use serde_json::Value;

    fn component<'a>(description: &'a Value, type_name: &str) -> &'a Value {
        description["components"]
            .as_array()
            .expect("components are a list")
            .iter()
            .find(|component| component["type_name"] == type_name)
            .unwrap_or_else(|| panic!("{type_name} is registered"))
    }

    #[test]
    fn a_component_with_an_honest_blank_is_addable() {
        let description = describe().expect("the built-in components describe themselves");
        let shape = component(&description, "sindri.shape");

        assert_eq!(shape["addable"], Value::Bool(true));
        assert!(
            shape["default_payload"].is_object(),
            "a fresh shape is a payload, not nothing"
        );
    }

    #[test]
    fn a_component_needing_a_project_asset_is_not_addable() {
        let description = describe().expect("the built-in components describe themselves");
        let text = component(&description, "sindri.ui.text");

        // The distinction this file exists to carry: seven fields, and no blank,
        // because there is no font the engine can invent.
        assert_eq!(text["addable"], Value::Bool(false));
        assert_eq!(text["default_payload"], Value::Null);
        assert!(text["fields"].is_object(), "it still says what it has");
    }
}
