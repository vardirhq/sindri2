//! The vector arithmetic a migration needs.
//!
//! Written out rather than reached for from `glam`, because a migration
//! must keep producing the same numbers for as long as the format it
//! upgrades exists, and a dependency is free to change its mind.

use serde_json::Value;

use crate::SceneMigrationError;

use super::camera::CAMERA_MIGRATION_EPSILON;

pub(crate) fn migration_vec3(
    value: Option<&Value>,
    fallback: [f64; 3],
    what: &str,
) -> Result<[f64; 3], SceneMigrationError> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let values = value.as_array().ok_or_else(|| {
        SceneMigrationError::Unconvertible(format!("{what} must be an array of three numbers"))
    })?;
    if values.len() != 3 {
        return Err(SceneMigrationError::Unconvertible(format!(
            "{what} must contain exactly three numbers"
        )));
    }
    let mut out = [0.0; 3];
    for (index, item) in values.iter().enumerate() {
        out[index] = item.as_f64().filter(|v| v.is_finite()).ok_or_else(|| {
            SceneMigrationError::Unconvertible(format!("{what} must contain only finite numbers"))
        })?;
    }
    Ok(out)
}

pub(crate) fn migration_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn migration_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn migration_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn migration_normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length2 = migration_dot(v, v);
    if !length2.is_finite() || length2 <= CAMERA_MIGRATION_EPSILON {
        return None;
    }
    let inv = length2.sqrt().recip();
    Some([v[0] * inv, v[1] * inv, v[2] * inv])
}
