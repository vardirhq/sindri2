//! The documents the migration tests start from.

use serde_json::{Value, json};

use crate::SceneMigrationError;

pub(super) fn rename_label_to_name(document: &mut Value) -> Result<(), SceneMigrationError> {
    let entities = document
        .get_mut("entities")
        .and_then(Value::as_array_mut)
        .ok_or(SceneMigrationError::StepFailed {
            from_version: 0,
            reason: "document has no 'entities' array".to_owned(),
        })?;
    for entity in entities {
        let object = entity
            .as_object_mut()
            .ok_or(SceneMigrationError::StepFailed {
                from_version: 0,
                reason: "every entity must be an object".to_owned(),
            })?;
        if let Some(label) = object.remove("label") {
            object.insert("name".to_owned(), label);
        }
    }
    Ok(())
}

pub(super) fn legacy_document() -> String {
    json!({
        "format_version": 0,
        "entities": [{ "id": "player", "label": "Player" }],
    })
    .to_string()
}
