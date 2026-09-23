//! Format 10: the sun leaves the environment and becomes a light entity.
//!
//! Format 9 stored the sun as `sindri.environment.directional`: a direction
//! vector, a colour and an intensity. Nothing drew it and nothing said which
//! way it pointed, so a vector with a positive Y lit the world from below and
//! put shadows on hilltops without anyone being able to see why. Format 10
//! makes it an entity carrying `sindri.light`, aimed by its rotation along
//! local -Z, the way a camera is.
//!
//! A sun of zero intensity lit nothing, and a scene that never said anything
//! about one had zero intensity; neither gains an entity.

use serde_json::{Map, Value, json};

use crate::SceneMigrationError;

use super::camera::camera_rotation_from_legacy_look_at;

const ENVIRONMENT: &str = "sindri.environment";
const LIGHT: &str = "sindri.light";

/// The direction and colour a format-9 sun had when it did not say.
const DEFAULT_DIRECTION: [f64; 3] = [-0.45, -1.0, -0.35];
const DEFAULT_COLOR: [f64; 3] = [1.0, 0.95, 0.86];

/// Where the new entity stands. A directional light's position lights
/// nothing; it is only where the Scene view draws it, so it goes above the
/// origin where it will be seen.
const SUN_POSITION: [f64; 3] = [0.0, 10.0, 0.0];

pub(crate) fn move_the_sun_into_a_light(document: &mut Value) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let mut suns = Vec::new();
    for entity in entities.iter_mut() {
        let Some(environment) = entity
            .get_mut("components")
            .and_then(|components| components.get_mut(ENVIRONMENT))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let Some(directional) = environment.remove("directional") else {
            continue;
        };
        if let Some(sun) = sun_from(&directional)? {
            suns.push(sun);
        }
    }
    for sun in suns {
        let id = unused_id(entities, "sun");
        entities.push(json!({
            "id": id,
            "name": "Sun",
            "transform_3d": {
                "position": SUN_POSITION,
                "rotation": sun.rotation,
                "scale": [1.0, 1.0, 1.0]
            },
            "components": { LIGHT: {
                "kind": "directional",
                "color": sun.color,
                "intensity": sun.intensity
            } }
        }));
    }
    Ok(())
}

struct LegacySun {
    rotation: [f64; 4],
    color: Value,
    intensity: f64,
}

fn sun_from(directional: &Value) -> Result<Option<LegacySun>, SceneMigrationError> {
    let Some(fields) = directional.as_object() else {
        return Err(failed("`sindri.environment.directional` must be an object"));
    };
    let intensity = match fields.get("intensity") {
        None => 0.0,
        Some(value) => value
            .as_f64()
            .ok_or_else(|| failed("`directional.intensity` must be a number"))?,
    };
    if intensity <= 0.0 {
        return Ok(None);
    }
    let direction = vector(fields, "direction")?.unwrap_or(DEFAULT_DIRECTION);
    let color = fields
        .get("color")
        .cloned()
        .unwrap_or_else(|| json!(DEFAULT_COLOR));
    Ok(Some(LegacySun {
        rotation: camera_rotation_from_legacy_look_at([0.0; 3], direction, [0.0, 1.0, 0.0]),
        color,
        intensity,
    }))
}

fn vector(fields: &Map<String, Value>, key: &str) -> Result<Option<[f64; 3]>, SceneMigrationError> {
    let Some(value) = fields.get(key) else {
        return Ok(None);
    };
    let components = value
        .as_array()
        .filter(|items| items.len() == 3)
        .and_then(|items| items.iter().map(Value::as_f64).collect::<Option<Vec<_>>>())
        .ok_or_else(|| failed("`directional.direction` must be three numbers"))?;
    Ok(Some([components[0], components[1], components[2]]))
}

/// `base`, or `base-2`, `base-3`… whichever no entity already has.
fn unused_id(entities: &[Value], base: &str) -> String {
    let taken = |id: &str| {
        entities
            .iter()
            .any(|entity| entity.get("id").and_then(Value::as_str) == Some(id))
    };
    if !taken(base) {
        return base.to_owned();
    }
    // One more candidate than there are entities: at least one is free.
    (2..=entities.len() + 2)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|id| !taken(id))
        .expect("more candidates than entities, so one is unused")
}

fn failed(reason: &str) -> SceneMigrationError {
    SceneMigrationError::StepFailed {
        from_version: 9,
        reason: reason.to_owned(),
    }
}
