//! Formats 5 and 6: the camera's own orientation, and dropping the
//! overlay camera.

use serde_json::{Value, json};

use crate::SceneMigrationError;

use super::vector::{migration_cross, migration_normalize, migration_sub, migration_vec3};

/// Format 6 makes a perspective camera's orientation part of its entity transform.
///
/// Format 5 stored an eye in `Transform3D.position` but kept the direction as
/// `target` and `up` inside `sindri.camera`. The new camera follows the ordinary
/// transform convention instead: local -Z faces forward and local +Y is up.
/// Migrating therefore turns that look-at basis into a quaternion and removes
/// the two camera-only direction fields. Existing transform scale is untouched.
pub(crate) const CAMERA_MIGRATION_EPSILON: f64 = 1.0e-12;

pub(crate) const CAMERA_COMPONENT: &str = "sindri.camera";

pub(crate) fn camera_rotation_from_legacy_look_at(
    eye: [f64; 3],
    target: [f64; 3],
    authored_up: [f64; 3],
) -> [f64; 4] {
    let Some(forward) = migration_normalize(migration_sub(target, eye)) else {
        return [0.0, 0.0, 0.0, 1.0];
    };
    let mut up = migration_normalize(authored_up).unwrap_or([0.0, 1.0, 0.0]);
    if migration_normalize(migration_cross(up, forward)).is_none() {
        up = if forward[1].abs() < 0.999 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
    }
    let right = migration_normalize(migration_cross(forward, up)).unwrap_or([1.0, 0.0, 0.0]);
    let corrected_up =
        migration_normalize(migration_cross(right, forward)).unwrap_or([0.0, 1.0, 0.0]);
    quaternion_from_basis(right, corrected_up, [-forward[0], -forward[1], -forward[2]])
}

pub(crate) fn quaternion_from_basis(right: [f64; 3], up: [f64; 3], back: [f64; 3]) -> [f64; 4] {
    let (m00, m01, m02) = (right[0], up[0], back[0]);
    let (m10, m11, m12) = (right[1], up[1], back[1]);
    let (m20, m21, m22) = (right[2], up[2], back[2]);
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
    normalize_quaternion([x, y, z, w])
}

pub(crate) fn normalize_quaternion([x, y, z, w]: [f64; 4]) -> [f64; 4] {
    let length = (x * x + y * y + z * z + w * w).sqrt();
    if !length.is_finite() || length <= CAMERA_MIGRATION_EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [x / length, y / length, z / length, w / length]
    }
}

pub(crate) fn move_camera_look_at_into_transform(
    document: &mut Value,
) -> Result<(), SceneMigrationError> {
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
        let target = migration_vec3(
            camera.get("target"),
            [0.0, 0.0, 0.0],
            &format!("camera target on entity '{entity_id}'"),
        )?;
        let up = migration_vec3(
            camera.get("up"),
            [0.0, 1.0, 0.0],
            &format!("camera up on entity '{entity_id}'"),
        )?;
        camera.remove("target");
        camera.remove("up");

        let transform = fields
            .entry("transform_3d".to_owned())
            .or_insert_with(|| json!({}));
        let transform = transform.as_object_mut().ok_or_else(|| {
            SceneMigrationError::Unconvertible(format!(
                "transform_3d on camera entity '{entity_id}' must be an object"
            ))
        })?;
        let eye = migration_vec3(
            transform.get("position"),
            [0.0, 0.0, 0.0],
            &format!("transform position on camera entity '{entity_id}'"),
        )?;
        transform.insert(
            "rotation".to_owned(),
            json!(camera_rotation_from_legacy_look_at(eye, target, up)),
        );
    }
    Ok(())
}

/// Format 7 removes the camera-backed screen overlay.
///
/// In format 6 every orthographic `sindri.camera` was the screen overlay;
/// orthographic world cameras did not exist yet. Format 7 makes both camera
/// projections world cameras and moves screen-space rendering to viewport-owned
/// projection state, so an old orthographic camera component has no runtime
/// responsibility left. The entity itself is preserved because its name,
/// editor state, transform, or unrelated components may still matter.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn remove_legacy_overlay_camera(
    document: &mut Value,
) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entity in entities {
        let Some(components) = entity
            .as_object_mut()
            .and_then(|fields| fields.get_mut("components"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let is_overlay = components
            .get(CAMERA_COMPONENT)
            .and_then(Value::as_object)
            .and_then(|camera| camera.get("projection"))
            .and_then(Value::as_str)
            == Some("orthographic");
        if is_overlay {
            components.remove(CAMERA_COMPONENT);
        }
    }
    Ok(())
}
