//! Format 4: component type names gain their `sindri.` namespace.

use serde_json::Value;

use crate::SceneMigrationError;

pub(crate) fn namespace_components(document: &mut Value) -> Result<(), SceneMigrationError> {
    const RENAMES: [(&str, &str); 4] = [
        ("sindri.grid_navigation", "sindri.grid.navigation"),
        ("sindri.grid_occupant", "sindri.grid.occupant"),
        ("sindri.sprite_animation", "sindri.animation.sprite"),
        ("sindri.audio", "sindri.audio.source"),
    ];

    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entity in entities {
        let Some(fields) = entity.as_object_mut() else {
            continue;
        };
        let entity_id = fields
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<an entity with no id>")
            .to_owned();
        let Some(components) = fields.get_mut("components").and_then(Value::as_object_mut) else {
            continue;
        };

        for (old, new) in RENAMES {
            if components.contains_key(old) && components.contains_key(new) {
                return Err(SceneMigrationError::Unconvertible(format!(
                    "entity '{entity_id}' carries both legacy component '{old}' and canonical component '{new}'"
                )));
            }
            if let Some(payload) = components.remove(old) {
                components.insert(new.to_owned(), payload);
            }
        }
    }
    Ok(())
}
