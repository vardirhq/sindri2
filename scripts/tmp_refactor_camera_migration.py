from pathlib import Path

p = Path('crates/sindri-core/src/migration.rs')
s = p.read_text()
start = s.index('fn move_camera_look_at_into_transform(document: &mut Value) -> Result<(), SceneMigrationError> {')
end = s.index('/// Which cell of which grid a normalized rect is, when it is one.')
replacement = r'''const CAMERA_MIGRATION_EPSILON: f64 = 1.0e-12;
const CAMERA_COMPONENT: &str = "sindri.camera";

fn migration_vec3(
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
            SceneMigrationError::Unconvertible(format!(
                "{what} must contain only finite numbers"
            ))
        })?;
    }
    Ok(out)
}

fn migration_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn migration_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn migration_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn migration_normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length2 = migration_dot(v, v);
    if !length2.is_finite() || length2 <= CAMERA_MIGRATION_EPSILON {
        return None;
    }
    let inv = length2.sqrt().recip();
    Some([v[0] * inv, v[1] * inv, v[2] * inv])
}

fn camera_rotation_from_legacy_look_at(
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

fn quaternion_from_basis(right: [f64; 3], up: [f64; 3], back: [f64; 3]) -> [f64; 4] {
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

fn normalize_quaternion([x, y, z, w]: [f64; 4]) -> [f64; 4] {
    let length = (x * x + y * y + z * z + w * w).sqrt();
    if !length.is_finite() || length <= CAMERA_MIGRATION_EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [x / length, y / length, z / length, w / length]
    }
}

fn move_camera_look_at_into_transform(document: &mut Value) -> Result<(), SceneMigrationError> {
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

'''
p.write_text(s[:start] + replacement + s[end:])
