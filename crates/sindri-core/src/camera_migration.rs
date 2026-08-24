use serde_json::{Value, json};

use crate::SceneMigrationError;

const CAMERA_COMPONENT: &str = "sindri.camera";
const EPSILON: f64 = 1.0e-12;

/// Converts legacy perspective-camera look-at data into `Transform3D.rotation`.
///
/// This is kept separate from the registered migration chain until the current
/// camera schema and extractor consume transform rotation. Registering it before
/// that switch would create a scene format that this same runtime could migrate
/// to but not load.
pub(crate) fn orient_perspective_cameras(document: &mut Value) -> Result<(), SceneMigrationError> {
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

        let Some(camera) = fields
            .get_mut("components")
            .and_then(Value::as_object_mut)
            .and_then(|components| components.get_mut(CAMERA_COMPONENT))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        if camera.get("projection").and_then(Value::as_str) != Some("perspective") {
            continue;
        }

        let target = vec3(camera.get("target"), "target", &entity_id)?;
        let authored_up = vec3(camera.get("up"), "up", &entity_id)?;
        let eye = fields
            .get("transform_3d")
            .and_then(|transform| transform.get("position"))
            .map_or(Ok([0.0, 0.0, 0.0]), |value| {
                vec3(Some(value), "transform_3d.position", &entity_id)
            })?;
        let rotation = rotation_from_look_at(eye, target, authored_up);

        let transform = fields
            .entry("transform_3d".to_owned())
            .or_insert_with(|| json!({}));
        let Some(transform) = transform.as_object_mut() else {
            return Err(SceneMigrationError::Unconvertible(format!(
                "camera entity '{entity_id}' has a transform_3d that is not an object"
            )));
        };
        transform.insert("rotation".to_owned(), json!(rotation));

        camera.remove("target");
        camera.remove("up");
    }
    Ok(())
}

fn vec3(
    value: Option<&Value>,
    field: &str,
    entity_id: &str,
) -> Result<[f64; 3], SceneMigrationError> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Err(SceneMigrationError::Unconvertible(format!(
            "camera entity '{entity_id}' needs '{field}' as three numbers"
        )));
    };
    let numbers = values.iter().filter_map(Value::as_f64).collect::<Vec<_>>();
    <[f64; 3]>::try_from(numbers.as_slice()).map_err(|_| {
        SceneMigrationError::Unconvertible(format!(
            "camera entity '{entity_id}' needs '{field}' as three numbers"
        ))
    })
}

fn rotation_from_look_at(eye: [f64; 3], target: [f64; 3], authored_up: [f64; 3]) -> [f64; 4] {
    let Some(forward) = normalize(sub(target, eye)) else {
        return [0.0, 0.0, 0.0, 1.0];
    };
    let mut up = normalize(authored_up).unwrap_or([0.0, 1.0, 0.0]);
    if length_squared(cross(up, forward)) <= EPSILON {
        up = fallback_up(forward);
    }
    let right = normalize(cross(forward, up)).unwrap_or([1.0, 0.0, 0.0]);
    let corrected_up = normalize(cross(right, forward)).unwrap_or([0.0, 1.0, 0.0]);
    let back = mul(forward, -1.0);
    quaternion_from_basis(right, corrected_up, back)
}

fn quaternion_from_basis(right: [f64; 3], up: [f64; 3], back: [f64; 3]) -> [f64; 4] {
    // Matrix rows for a matrix whose columns are right, up, and local +Z/back.
    let m00 = right[0];
    let m01 = up[0];
    let m02 = back[0];
    let m10 = right[1];
    let m11 = up[1];
    let m12 = back[1];
    let m20 = right[2];
    let m21 = up[2];
    let m22 = back[2];
    let trace = m00 + m11 + m22;

    let (x, y, z, w) = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        ((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s)
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        (0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        ((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        ((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
    };
    let magnitude = (x * x + y * y + z * z + w * w).sqrt();
    if magnitude <= EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [x / magnitude, y / magnitude, z / magnitude, w / magnitude]
    }
}

fn fallback_up(forward: [f64; 3]) -> [f64; 3] {
    if forward[1].abs() < 0.999 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn normalize(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = length_squared(value).sqrt();
    (length > EPSILON).then(|| mul(value, 1.0 / length))
}

const fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

const fn mul(value: [f64; 3], scalar: f64) -> [f64; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

const fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

const fn length_squared(value: [f64; 3]) -> f64 {
    value[0] * value[0] + value[1] * value[1] + value[2] * value[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotate(rotation: [f64; 4], vector: [f64; 3]) -> [f64; 3] {
        let [x, y, z, w] = rotation;
        let q = [x, y, z];
        let t = mul(cross(q, vector), 2.0);
        add(add(vector, mul(t, w)), cross(q, t))
    }

    const fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }

    fn assert_near(actual: [f64; 3], expected: [f64; 3]) {
        assert!(
            length_squared(sub(actual, expected)).sqrt() < 1.0e-9,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn conversion_moves_look_at_into_the_transform_and_removes_legacy_fields() {
        let mut document = json!({
            "format_version": 5,
            "entities": [{
                "id": "camera",
                "transform_3d": { "position": [3.0, 2.0, 4.0] },
                "components": {
                    "sindri.camera": {
                        "projection": "perspective",
                        "target": [0.0, 0.0, 0.0],
                        "up": [0.0, 1.0, 0.0],
                        "vertical_fov_degrees": 45.0,
                        "near": 0.1,
                        "far": 100.0
                    }
                }
            }]
        });
        orient_perspective_cameras(&mut document).unwrap();

        let camera = &document["entities"][0]["components"][CAMERA_COMPONENT];
        assert!(camera.get("target").is_none());
        assert!(camera.get("up").is_none());
        let values = document["entities"][0]["transform_3d"]["rotation"]
            .as_array()
            .unwrap()
            .iter()
            .map(Value::as_f64)
            .collect::<Option<Vec<_>>>()
            .unwrap();
        let rotation = <[f64; 4]>::try_from(values.as_slice()).unwrap();
        assert_near(
            rotate(rotation, [0.0, 0.0, -1.0]),
            normalize([-3.0, -2.0, -4.0]).unwrap(),
        );
    }

    #[test]
    fn orthographic_camera_is_untouched() {
        let mut document = json!({
            "entities": [{
                "id": "overlay",
                "components": {
                    "sindri.camera": {
                        "projection": "orthographic",
                        "center": [0.0, 0.0],
                        "vertical_size": 10.0,
                        "near": -1.0,
                        "far": 1.0
                    }
                }
            }]
        });
        let before = document.clone();
        orient_perspective_cameras(&mut document).unwrap();
        assert_eq!(document, before);
    }

    #[test]
    fn malformed_legacy_camera_is_rejected_instead_of_guessed_at() {
        let mut document = json!({
            "entities": [{
                "id": "camera",
                "components": {
                    "sindri.camera": {
                        "projection": "perspective",
                        "target": [0.0, 0.0],
                        "up": [0.0, 1.0, 0.0]
                    }
                }
            }]
        });
        let error = orient_perspective_cameras(&mut document).unwrap_err();
        assert!(matches!(
            error,
            SceneMigrationError::Unconvertible(message)
                if message.contains("camera") && message.contains("target")
        ));
    }
}
