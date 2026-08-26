//! Formats 1 and 2: the 2D transform folded into the 3D one.

use serde_json::{Value, json};

use crate::SceneMigrationError;

/// Format 2 replaced the separate 2D transform with the single 3D one, so a 2D
/// transform becomes a 3D transform on the Z = 0 plane: the angle becomes a
/// quaternion about Z and the two-component scale gains a Z of 1. Nothing is
/// lost, so nothing here asks the author to choose.
///
/// Except in one case. An entity carrying both transforms is rejected rather
/// than resolved: the two describe positions in different spaces, so no merge
/// of them is reliably the same scene, and quietly preferring one would move
/// something without saying so.
pub(crate) fn collapse_transform_2d(document: &mut Value) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };

    for entity in entities {
        let Some(fields) = entity.as_object_mut() else {
            continue;
        };
        let Some(flat) = fields.remove("transform_2d") else {
            continue;
        };
        if fields.contains_key("transform_3d") {
            return Err(SceneMigrationError::ConflictingTransforms {
                entity: fields
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("<an entity with no id>")
                    .to_owned(),
            });
        }

        let pair = |key: &str, fallback: [f64; 2]| -> [f64; 2] {
            flat.get(key)
                .and_then(Value::as_array)
                .filter(|values| values.len() == 2)
                .and_then(|values| Some([values[0].as_f64()?, values[1].as_f64()?]))
                .unwrap_or(fallback)
        };
        let [x, y] = pair("position", [0.0, 0.0]);
        let [scale_x, scale_y] = pair("scale", [1.0, 1.0]);
        let angle = flat
            .get("rotation_radians")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let half = angle / 2.0;

        fields.insert(
            "transform_3d".to_owned(),
            json!({
                "position": [x, y, 0.0],
                // Quaternion in [x, y, z, w] order, turning about Z alone.
                "rotation": [0.0, 0.0, half.sin(), half.cos()],
                "scale": [scale_x, scale_y, 1.0],
            }),
        );
    }
    Ok(())
}

/// Writes `z` into an entity's transform, giving it one if it had none.
pub(crate) fn set_transform_z(fields: &mut serde_json::Map<String, Value>, z: f64) {
    let transform = fields
        .entry("transform_3d".to_owned())
        .or_insert_with(|| json!({}));
    let Some(transform) = transform.as_object_mut() else {
        return;
    };
    let position = transform
        .entry("position".to_owned())
        .or_insert_with(|| json!([0.0, 0.0, 0.0]));
    let Some(position) = position.as_array_mut() else {
        return;
    };
    while position.len() < 3 {
        position.push(json!(0.0));
    }
    position[2] = json!(z);
}
