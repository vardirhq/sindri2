from pathlib import Path

migration = Path('crates/sindri-core/src/migration.rs')
text = migration.read_text()
text = text.replace(
'''        migrator
            .register(4, 5, namespace_components)
            .expect("built-in steps are registered once and move forward");
        migrator
''',
'''        migrator
            .register(4, 5, namespace_components)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(5, 6, move_camera_look_at_into_transform)
            .expect("built-in steps are registered once and move forward");
        migrator
''')

marker = '''/// Which cell of which grid a normalized rect is, when it is one.\n'''
insert = r'''/// Format 6 makes a perspective camera's orientation part of its entity transform.
///
/// Format 5 stored an eye in `Transform3D.position` but kept the direction as
/// `target` and `up` inside `sindri.camera`. The new camera follows the ordinary
/// transform convention instead: local -Z faces forward and local +Y is up.
/// Migrating therefore turns that look-at basis into a quaternion and removes
/// the two camera-only direction fields. Existing transform scale is untouched.
fn move_camera_look_at_into_transform(document: &mut Value) -> Result<(), SceneMigrationError> {
    const EPSILON: f64 = 1.0e-12;
    const CAMERA_COMPONENT: &str = "sindri.camera";

    fn vec3(value: Option<&Value>, fallback: [f64; 3], what: &str) -> Result<[f64; 3], SceneMigrationError> {
        let Some(value) = value else { return Ok(fallback); };
        let values = value.as_array().ok_or_else(|| SceneMigrationError::Unconvertible(format!("{what} must be an array of three numbers")))?;
        if values.len() != 3 {
            return Err(SceneMigrationError::Unconvertible(format!("{what} must contain exactly three numbers")));
        }
        let mut out = [0.0; 3];
        for (index, item) in values.iter().enumerate() {
            out[index] = item.as_f64().filter(|v| v.is_finite()).ok_or_else(|| SceneMigrationError::Unconvertible(format!("{what} must contain only finite numbers")))?;
        }
        Ok(out)
    }
    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
    }
    fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
        let length2 = dot(v, v);
        if !length2.is_finite() || length2 <= EPSILON { return None; }
        let inv = length2.sqrt().recip();
        Some([v[0]*inv, v[1]*inv, v[2]*inv])
    }
    fn rotation(eye: [f64; 3], target: [f64; 3], authored_up: [f64; 3]) -> [f64; 4] {
        let Some(forward) = normalize(sub(target, eye)) else { return [0.0, 0.0, 0.0, 1.0]; };
        let mut up = normalize(authored_up).unwrap_or([0.0, 1.0, 0.0]);
        if normalize(cross(up, forward)).is_none() {
            up = if forward[1].abs() < 0.999 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
        }
        let right = normalize(cross(forward, up)).unwrap_or([1.0, 0.0, 0.0]);
        let corrected_up = normalize(cross(right, forward)).unwrap_or([0.0, 1.0, 0.0]);
        let back = [-forward[0], -forward[1], -forward[2]];

        let (m00,m01,m02) = (right[0], corrected_up[0], back[0]);
        let (m10,m11,m12) = (right[1], corrected_up[1], back[1]);
        let (m20,m21,m22) = (right[2], corrected_up[2], back[2]);
        let trace = m00 + m11 + m22;
        let (x,y,z,w) = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            ((m21-m12)/s, (m02-m20)/s, (m10-m01)/s, 0.25*s)
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            (0.25*s, (m01+m10)/s, (m02+m20)/s, (m21-m12)/s)
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            ((m01+m10)/s, 0.25*s, (m12+m21)/s, (m02-m20)/s)
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            ((m02+m20)/s, (m12+m21)/s, 0.25*s, (m10-m01)/s)
        };
        let length = (x*x+y*y+z*z+w*w).sqrt();
        if !length.is_finite() || length <= EPSILON { [0.0,0.0,0.0,1.0] }
        else { [x/length,y/length,z/length,w/length] }
    }

    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else { return Ok(()); };
    for entity in entities {
        let Some(fields) = entity.as_object_mut() else { continue; };
        let entity_id = fields.get("id").and_then(Value::as_str).unwrap_or("<an entity with no id>").to_owned();
        let Some(camera) = fields.get_mut("components").and_then(Value::as_object_mut).and_then(|c| c.get_mut(CAMERA_COMPONENT)).and_then(Value::as_object_mut) else { continue; };
        if camera.get("projection").and_then(Value::as_str) != Some("perspective") { continue; }
        let target = vec3(camera.get("target"), [0.0,0.0,0.0], &format!("camera target on entity '{entity_id}'"))?;
        let up = vec3(camera.get("up"), [0.0,1.0,0.0], &format!("camera up on entity '{entity_id}'"))?;
        camera.remove("target");
        camera.remove("up");

        let transform = fields.entry("transform_3d".to_owned()).or_insert_with(|| json!({}));
        let transform = transform.as_object_mut().ok_or_else(|| SceneMigrationError::Unconvertible(format!("transform_3d on camera entity '{entity_id}' must be an object")))?;
        let eye = vec3(transform.get("position"), [0.0,0.0,0.0], &format!("transform position on camera entity '{entity_id}'"))?;
        transform.insert("rotation".to_owned(), json!(rotation(eye, target, up)));
    }
    Ok(())
}

'''
text = text.replace(marker, insert + marker)

text = text.replace('''        assert_eq!(migrated["format_version"], json!(5));''', '''        assert_eq!(migrated["format_version"], json!(6));''')

test_marker = '''    #[test]\n    fn namespace_migration_refuses_ambiguous_duplicate_spellings() {\n'''
new_tests = r'''    #[test]
    fn format_five_camera_look_at_becomes_transform_rotation() {
        let old = json!({
            "format_version": 5,
            "entities": [{
                "id": "camera",
                "transform_3d": {
                    "position": [3.0, 2.0, 4.0],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "scale": [2.0, 2.0, 2.0]
                },
                "components": {
                    "sindri.camera": {
                        "projection": "perspective",
                        "target": [0.0, 0.0, 0.0],
                        "up": [0.0, 1.0, 0.0],
                        "vertical_fov_degrees": 60.0,
                        "near": 0.1,
                        "far": 100.0
                    }
                }
            }]
        });
        let migrated = SceneMigrator::builtin().migrate(old).unwrap();
        assert_eq!(migrated["format_version"], json!(6));
        let camera = &migrated["entities"][0]["components"]["sindri.camera"];
        assert!(camera.get("target").is_none());
        assert!(camera.get("up").is_none());
        assert_eq!(migrated["entities"][0]["transform_3d"]["scale"], json!([2.0, 2.0, 2.0]));

        let rotation = migrated["entities"][0]["transform_3d"]["rotation"].as_array().unwrap();
        let q = [rotation[0].as_f64().unwrap(), rotation[1].as_f64().unwrap(), rotation[2].as_f64().unwrap(), rotation[3].as_f64().unwrap()];
        let rotate = |v: [f64;3]| {
            let [x,y,z,w] = q;
            let u = [x,y,z];
            let uv = cross(u, v);
            let uuv = cross(u, uv);
            [v[0] + 2.0*(w*uv[0]+uuv[0]), v[1] + 2.0*(w*uv[1]+uuv[1]), v[2] + 2.0*(w*uv[2]+uuv[2])]
        };
        let forward = normalize([-3.0,-2.0,-4.0]).unwrap();
        let actual = rotate([0.0,0.0,-1.0]);
        assert!(dot(sub(actual, forward), sub(actual, forward)) < 1.0e-12);
    }

    #[test]
    fn format_five_orthographic_camera_is_not_reoriented() {
        let old = json!({
            "format_version": 5,
            "entities": [{
                "id": "overlay",
                "transform_3d": { "rotation": [0.1, 0.2, 0.3, 0.9] },
                "components": {
                    "sindri.camera": {
                        "projection": "orthographic",
                        "center": [0.0, 0.0],
                        "vertical_size": 2.0,
                        "near": -10.0,
                        "far": 10.0
                    }
                }
            }]
        });
        let migrated = SceneMigrator::builtin().migrate(old).unwrap();
        assert_eq!(migrated["entities"][0]["transform_3d"]["rotation"], json!([0.1,0.2,0.3,0.9]));
    }

'''
text = text.replace(test_marker, new_tests + test_marker)
migration.write_text(text)

scene = Path('crates/sindri-core/src/scene.rs')
scene.write_text(scene.read_text().replace('pub const SCENE_FORMAT_VERSION: u32 = 5;', 'pub const SCENE_FORMAT_VERSION: u32 = 6;'))

# Update docs that state the current scene format explicitly.
for name in ['docs/versioning.md', 'docs/scene-serialization.md']:
    p = Path(name)
    s = p.read_text()
    s = s.replace('format version 5', 'format version 6').replace('format_version: 5', 'format_version: 6').replace('"format_version": 5', '"format_version": 6')
    p.write_text(s)

# Temporary migration helper. It is removed by the workflow after rewriting files.
helper = Path('crates/sindri-core/examples/tmp_migrate_format6.rs')
helper.parent.mkdir(parents=True, exist_ok=True)
helper.write_text(r'''use std::{fs, path::{Path, PathBuf}};
use sindri_core::{SceneDocument, SceneMigrator};

fn visit(path: &Path, out: &mut Vec<PathBuf>) {
    if path.file_name().and_then(|n| n.to_str()) == Some("target") { return; }
    let Ok(entries) = fs::read_dir(path) else { return; };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() { visit(&p, out); }
        else if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".scene.json")) { out.push(p); }
    }
}

fn main() {
    let mut files = Vec::new();
    visit(Path::new("."), &mut files);
    files.sort();
    let migrator = SceneMigrator::builtin();
    for path in files {
        let raw = fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let version = value.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version != 5 { continue; }
        let doc = SceneDocument::from_json_migrated(&raw, &migrator).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        fs::write(&path, doc.to_canonical_json().unwrap()).unwrap();
        println!("migrated {}", path.display());
    }
}
''')
