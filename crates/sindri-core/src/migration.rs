use std::collections::BTreeMap;

use serde_json::{Value, json};
use thiserror::Error;

use crate::SCENE_FORMAT_VERSION;

/// A single upgrade step between two adjacent scene format versions.
///
/// Steps operate on raw JSON because an old document cannot, by definition, be
/// deserialized into the current [`crate::SceneDocument`].
pub type SceneMigrationStep = fn(&mut Value) -> Result<(), SceneMigrationError>;

/// An ordered chain of scene format upgrades.
///
/// This exists before format version 2 so that the first real format change is
/// a registration rather than a redesign. A migrator with no registered steps
/// accepts current documents and rejects every other version with an
/// actionable error.
#[derive(Clone, Debug, Default)]
pub struct SceneMigrator {
    steps: BTreeMap<u32, (u32, SceneMigrationStep)>,
}

impl SceneMigrator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the upgrade applied to documents declaring `from_version`.
    ///
    /// Steps must move strictly forward and may not skip past the version this
    /// runtime understands, so a chain can never loop or overshoot.
    pub fn register(
        &mut self,
        from_version: u32,
        to_version: u32,
        step: SceneMigrationStep,
    ) -> Result<(), SceneMigrationError> {
        if to_version <= from_version {
            return Err(SceneMigrationError::NonProgressingStep {
                from_version,
                to_version,
            });
        }
        if to_version > SCENE_FORMAT_VERSION {
            return Err(SceneMigrationError::StepBeyondSupportedVersion {
                to_version,
                supported: SCENE_FORMAT_VERSION,
            });
        }
        if self.steps.contains_key(&from_version) {
            return Err(SceneMigrationError::DuplicateStep { from_version });
        }
        self.steps.insert(from_version, (to_version, step));
        Ok(())
    }

    /// The migrator with every built-in step registered.
    ///
    /// Anything that opens a scene a person may have written earlier should use
    /// this rather than assembling its own chain, so "can this runtime open
    /// that file" has one answer instead of one per caller.
    pub fn builtin() -> Self {
        let mut migrator = Self::new();
        migrator
            .register(1, 2, collapse_transform_2d)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(2, 3, sort_sprites_by_where_they_are)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(3, 4, name_the_parts_of_a_sheet)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(4, 5, namespace_components)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(5, 6, move_camera_look_at_into_transform)
            .expect("built-in steps are registered once and move forward");
        migrator
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Upgrades `document` to [`SCENE_FORMAT_VERSION`].
    ///
    /// Current documents pass through untouched. Each applied step has its
    /// declared target version written back, so individual migrations never
    /// have to remember to stamp `format_version` themselves.
    pub fn migrate(&self, mut document: Value) -> Result<Value, SceneMigrationError> {
        let mut version = read_format_version(&document)?;
        while version != SCENE_FORMAT_VERSION {
            if version > SCENE_FORMAT_VERSION {
                return Err(SceneMigrationError::FromTheFuture {
                    found: version,
                    supported: SCENE_FORMAT_VERSION,
                });
            }
            let Some(&(to_version, step)) = self.steps.get(&version) else {
                return Err(SceneMigrationError::NoRegisteredStep {
                    from_version: version,
                    supported: SCENE_FORMAT_VERSION,
                });
            };
            step(&mut document)?;
            write_format_version(&mut document, to_version)?;
            version = to_version;
        }
        Ok(document)
    }
}

fn read_format_version(document: &Value) -> Result<u32, SceneMigrationError> {
    let object = document
        .as_object()
        .ok_or(SceneMigrationError::NotADocument)?;
    let version = object
        .get("format_version")
        .ok_or(SceneMigrationError::MissingFormatVersion)?;
    version
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(SceneMigrationError::MissingFormatVersion)
}

fn write_format_version(document: &mut Value, version: u32) -> Result<(), SceneMigrationError> {
    let object = document
        .as_object_mut()
        .ok_or(SceneMigrationError::NotADocument)?;
    object.insert("format_version".to_owned(), Value::from(version));
    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SceneMigrationError {
    #[error("a scene document must be a JSON object")]
    NotADocument,
    #[error("a scene document must declare an integer 'format_version'")]
    MissingFormatVersion,
    #[error("scene format {found} is newer than this runtime's format {supported}")]
    FromTheFuture { found: u32, supported: u32 },
    #[error("{0}")]
    Unconvertible(String),
    #[error(
        "no registered migration upgrades scene format {from_version} toward format {supported}"
    )]
    NoRegisteredStep { from_version: u32, supported: u32 },
    #[error("a migration must move forward, but {from_version} to {to_version} does not")]
    NonProgressingStep { from_version: u32, to_version: u32 },
    #[error("a migration cannot target format {to_version}; this runtime supports {supported}")]
    StepBeyondSupportedVersion { to_version: u32, supported: u32 },
    #[error("scene format {from_version} already has a registered migration")]
    DuplicateStep { from_version: u32 },
    #[error("migrating scene format {from_version} failed: {reason}")]
    StepFailed { from_version: u32, reason: String },
    #[error(
        "entity '{entity}' has both a 2D and a 3D transform, which describe \
         positions in different spaces; remove one before upgrading the scene"
    )]
    ConflictingTransforms { entity: String },
}

/// Format 2 replaced the separate 2D transform with the single 3D one, so a 2D
/// transform becomes a 3D transform on the Z = 0 plane: the angle becomes a
/// quaternion about Z and the two-component scale gains a Z of 1. Nothing is
/// lost, so nothing here asks the author to choose.
///
/// Except in one case. An entity carrying both transforms is rejected rather
/// than resolved: the two describe positions in different spaces, so no merge
/// of them is reliably the same scene, and quietly preferring one would move
/// something without saying so.
fn collapse_transform_2d(document: &mut Value) -> Result<(), SceneMigrationError> {
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

/// Format 3 sorts transparent sprites by how far from the camera they are
/// rather than by a `depth` number typed beside them, so the field goes and the
/// transform's Z takes over the job.
///
/// A screen-space sprite's Z did nothing at all in format 2 — the overlay read
/// only X and Y — so its `depth` becomes a Z, negated, because the overlay
/// camera looks down the axis from `+Z` and a greater depth meant further away.
/// The stack it describes comes out in the same order it went in.
///
/// A world-space sprite already had a Z that placed it, and that Z is now what
/// orders it too, so its `depth` is simply dropped. That is the change itself
/// rather than a loss: a sort key that disagreed with where the sprite was is
/// exactly what this format stops allowing.
// The step signature is fixed by `SceneMigrationStep`, so this returns a
// `Result` it never uses: nothing here can fail, because a sprite either has a
// depth to move or does not.
#[allow(clippy::unnecessary_wraps)]
fn sort_sprites_by_where_they_are(document: &mut Value) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };

    for entity in entities {
        let Some(fields) = entity.as_object_mut() else {
            continue;
        };
        let Some(sprite) = fields
            .get_mut("components")
            .and_then(Value::as_object_mut)
            .and_then(|components| components.get_mut(SPRITE_COMPONENT))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let in_the_world = sprite.get("space").and_then(Value::as_str) == Some("world");
        let Some(depth) = sprite.remove("depth").as_ref().and_then(Value::as_f64) else {
            continue;
        };
        if in_the_world {
            continue;
        }
        set_transform_z(fields, -depth);
    }
    Ok(())
}

/// The component type name the format itself has to know, because the format is
/// what changed. `sindri-scene` owns what the component means.
const SPRITE_COMPONENT: &str = "sindri.sprite";

/// Writes `z` into an entity's transform, giving it one if it had none.
fn set_transform_z(fields: &mut serde_json::Map<String, Value>, z: f64) {
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

/// Format 3 to 4: a sheet is cut by the image, not by whoever draws it.
///
/// Before this, three components each said how a sheet was divided — a sprite
/// carried a raw rect, an animation carried a grid and cell numbers, a tilemap
/// carried a second grid and more cell numbers. After it, all three name
/// sprites and a sheet document beside the image says where those are.
///
/// **A migrated scene needs its sheets written.** This step can convert a
/// document; it cannot create the sidecar files beside the textures, because a
/// migration is handed JSON and not a project. What it does instead is emit the
/// names a default slice produces — cell `n` is called `"n"` — so a sheet
/// declaring the same grid the scene used to carry resolves every reference it
/// writes. The grid to declare is the one being removed here, which is why the
/// errors below name it.
fn name_the_parts_of_a_sheet(document: &mut Value) -> Result<(), SceneMigrationError> {
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

        if let Some(sprite) = components
            .get_mut("sindri.sprite")
            .and_then(Value::as_object_mut)
            && let Some(rect) = sprite.remove("uv_rect")
        {
            let cell = cell_of(&rect)?;
            if let Some(index) = cell
                && let Some(texture) = sprite.get("texture").and_then(Value::as_str)
            {
                let named = format!("{texture}#{index}");
                sprite.insert("texture".to_owned(), Value::String(named));
            }
        }

        if let Some(animation) = components
            .get_mut("sindri.sprite_animation")
            .and_then(Value::as_object_mut)
        {
            animation.remove("sheet");
            if let Some(clips) = animation.get_mut("clips").and_then(Value::as_object_mut) {
                for clip in clips.values_mut() {
                    let Some(frames) = clip.get_mut("frames").and_then(Value::as_array_mut) else {
                        continue;
                    };
                    for frame in frames.iter_mut() {
                        if let Some(cell) = frame.as_u64() {
                            *frame = Value::String(cell.to_string());
                        }
                    }
                }
            }
        }

        if let Some(tilemap) = components
            .get_mut("sindri.tilemap")
            .and_then(Value::as_object_mut)
        {
            tilemap.remove("sheet_columns");
            tilemap.remove("sheet_rows");
            // A palette naming every cell the map actually uses, in index
            // order, so the tile numbers already written keep pointing at what
            // they pointed at.
            let highest = tilemap
                .get("tiles")
                .and_then(Value::as_array)
                .map(|tiles| {
                    tiles
                        .iter()
                        .filter_map(Value::as_u64)
                        .max()
                        .map_or(0, |highest| highest + 1)
                })
                .unwrap_or_default();
            let palette: Vec<Value> = (0..highest)
                .map(|index| Value::String(index.to_string()))
                .collect();
            tilemap.insert("palette".to_owned(), Value::Array(palette));
        }
    }
    Ok(())
}

/// Format 5 gives subsystem-owned components stable hierarchical names.
///
/// Payloads are not changed. Only their keys move, so a format-4 scene carries
/// exactly the same authored data forward. A scene that somehow contains both
/// spellings is ambiguous and is rejected rather than silently overwriting one.
fn namespace_components(document: &mut Value) -> Result<(), SceneMigrationError> {
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

/// Format 6 makes a perspective camera's orientation part of its entity transform.
///
/// Format 5 stored an eye in `Transform3D.position` but kept the direction as
/// `target` and `up` inside `sindri.camera`. The new camera follows the ordinary
/// transform convention instead: local -Z faces forward and local +Y is up.
/// Migrating therefore turns that look-at basis into a quaternion and removes
/// the two camera-only direction fields. Existing transform scale is untouched.
fn move_camera_look_at_into_transform(document: &mut Value) -> Result<(), SceneMigrationError> {
    const EPSILON: f64 = 1.0e-12;
    const CAMERA_COMPONENT: &str = "sindri.camera";

    fn vec3(
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
    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
        let length2 = dot(v, v);
        if !length2.is_finite() || length2 <= EPSILON {
            return None;
        }
        let inv = length2.sqrt().recip();
        Some([v[0] * inv, v[1] * inv, v[2] * inv])
    }
    fn rotation(eye: [f64; 3], target: [f64; 3], authored_up: [f64; 3]) -> [f64; 4] {
        let Some(forward) = normalize(sub(target, eye)) else {
            return [0.0, 0.0, 0.0, 1.0];
        };
        let mut up = normalize(authored_up).unwrap_or([0.0, 1.0, 0.0]);
        if normalize(cross(up, forward)).is_none() {
            up = if forward[1].abs() < 0.999 {
                [0.0, 1.0, 0.0]
            } else {
                [0.0, 0.0, 1.0]
            };
        }
        let right = normalize(cross(forward, up)).unwrap_or([1.0, 0.0, 0.0]);
        let corrected_up = normalize(cross(right, forward)).unwrap_or([0.0, 1.0, 0.0]);
        let back = [-forward[0], -forward[1], -forward[2]];

        let (m00, m01, m02) = (right[0], corrected_up[0], back[0]);
        let (m10, m11, m12) = (right[1], corrected_up[1], back[1]);
        let (m20, m21, m22) = (right[2], corrected_up[2], back[2]);
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
        let length = (x * x + y * y + z * z + w * w).sqrt();
        if !length.is_finite() || length <= EPSILON {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            [x / length, y / length, z / length, w / length]
        }
    }

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
            .and_then(|c| c.get_mut(CAMERA_COMPONENT))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        if camera.get("projection").and_then(Value::as_str) != Some("perspective") {
            continue;
        }
        let target = vec3(
            camera.get("target"),
            [0.0, 0.0, 0.0],
            &format!("camera target on entity '{entity_id}'"),
        )?;
        let up = vec3(
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
        let eye = vec3(
            transform.get("position"),
            [0.0, 0.0, 0.0],
            &format!("transform position on camera entity '{entity_id}'"),
        )?;
        transform.insert("rotation".to_owned(), json!(rotation(eye, target, up)));
    }
    Ok(())
}

/// Which cell of which grid a normalized rect is, when it is one.
///
/// Every rect a hand-written scene ever carried was a cell of some uniform
/// grid — a sprite sheet is what a rect was added for — so this recovers the
/// index without being told the grid: a rect of width `w` is one of `1/w`
/// columns, and its `x` says which. A rect that is *not* a whole cell cannot
/// become a named sprite without a sheet to name it in, so it stops the
/// migration rather than quietly changing the picture.
fn cell_of(rect: &Value) -> Result<Option<u64>, SceneMigrationError> {
    /// How far a hand-typed rect may sit from the cell it means.
    const TOLERANCE: f64 = 1.0e-4;

    let Some(values) = rect.as_array() else {
        return Ok(None);
    };
    let [x, y, width, height] = <[f64; 4]>::try_from(
        values
            .iter()
            .filter_map(Value::as_f64)
            .collect::<Vec<f64>>()
            .as_slice(),
    )
    .map_err(|_| {
        SceneMigrationError::Unconvertible(format!(
            "a sprite's uv_rect must be four numbers, and this one is {rect}"
        ))
    })?;
    // Near enough rather than exactly, because these are numbers a person
    // typed into a file: 0.333 is the first of three columns and refusing it
    // over the last decimal place would help nobody.
    let close = |value: f64, to: f64| (value - to).abs() < TOLERANCE;
    // The whole image is not a cell of anything, and needs no name.
    if close(x, 0.0) && close(y, 0.0) && close(width, 1.0) && close(height, 1.0) {
        return Ok(None);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = |value: f64| -> Option<u64> {
        let rounded = value.round();
        (rounded >= 0.0 && close(value, rounded)).then_some(rounded as u64)
    };
    let unconvertible = || {
        SceneMigrationError::Unconvertible(format!(
            "a sprite's uv_rect {rect} is not a whole cell of a uniform grid, so format 4 has no \
             name for it — give its texture a sheet naming that rect, and point the sprite at it"
        ))
    };
    if width <= 0.0 || height <= 0.0 {
        return Err(unconvertible());
    }
    let columns = whole(1.0 / width).ok_or_else(unconvertible)?;
    let column = whole(x / width).ok_or_else(unconvertible)?;
    let row = whole(y / height).ok_or_else(unconvertible)?;
    Ok(Some(row * columns + column))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{SceneDocument, SceneJsonError};

    /// Stands in for a real historical upgrade: format 0 stored a flat
    /// `label` where format 1 stores `name`.
    fn rename_label_to_name(document: &mut Value) -> Result<(), SceneMigrationError> {
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

    fn legacy_document() -> String {
        json!({
            "format_version": 0,
            "entities": [{ "id": "player", "label": "Player" }],
        })
        .to_string()
    }

    #[test]
    fn current_documents_pass_through_untouched() {
        let migrator = SceneMigrator::new();
        assert!(migrator.is_empty());
        let document = json!({ "format_version": SCENE_FORMAT_VERSION, "entities": [] });
        assert_eq!(migrator.migrate(document.clone()).unwrap(), document);
    }

    #[test]
    fn registered_steps_upgrade_older_documents() {
        let mut migrator = SceneMigrator::builtin();
        migrator.register(0, 1, rename_label_to_name).unwrap();

        let document = SceneDocument::from_json_migrated(&legacy_document(), &migrator).unwrap();
        assert_eq!(document.format_version, SCENE_FORMAT_VERSION);
        assert_eq!(document.entities[0].name.as_deref(), Some("Player"));
    }

    #[test]
    fn format_four_component_names_migrate_without_touching_payloads() {
        let old = json!({
            "format_version": 4,
            "entities": [{
                "id": "player",
                "components": {
                    "sindri.grid_navigation": { "walls": [[[0, 0], [1, 0]]] },
                    "sindri.grid_occupant": { "grid": "floor", "footprint": [[0, 0]] },
                    "sindri.sprite_animation": { "clips": { "idle": { "frames": ["idle"] } } },
                    "sindri.audio": { "clip": "audio/pickup.wav", "volume": 0.75 }
                }
            }]
        });
        let migrated = SceneMigrator::builtin().migrate(old).unwrap();
        assert_eq!(migrated["format_version"], json!(6));
        let components = &migrated["entities"][0]["components"];
        assert_eq!(
            components["sindri.grid.navigation"],
            json!({ "walls": [[[0, 0], [1, 0]]] })
        );
        assert_eq!(
            components["sindri.grid.occupant"],
            json!({ "grid": "floor", "footprint": [[0, 0]] })
        );
        assert_eq!(
            components["sindri.animation.sprite"],
            json!({ "clips": { "idle": { "frames": ["idle"] } } })
        );
        assert_eq!(
            components["sindri.audio.source"],
            json!({ "clip": "audio/pickup.wav", "volume": 0.75 })
        );
        for old in [
            "sindri.grid_navigation",
            "sindri.grid_occupant",
            "sindri.sprite_animation",
            "sindri.audio",
        ] {
            assert!(components.get(old).is_none(), "legacy key {old} survived");
        }
    }

    #[test]
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
        assert_eq!(
            migrated["entities"][0]["transform_3d"]["scale"],
            json!([2.0, 2.0, 2.0])
        );

        let rotation = migrated["entities"][0]["transform_3d"]["rotation"]
            .as_array()
            .unwrap();
        let q = [
            rotation[0].as_f64().unwrap(),
            rotation[1].as_f64().unwrap(),
            rotation[2].as_f64().unwrap(),
            rotation[3].as_f64().unwrap(),
        ];
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let rotate = |v: [f64; 3]| {
            let [x, y, z, w] = q;
            let u = [x, y, z];
            let uv = cross(u, v);
            let uuv = cross(u, uv);
            [
                v[0] + 2.0 * (w * uv[0] + uuv[0]),
                v[1] + 2.0 * (w * uv[1] + uuv[1]),
                v[2] + 2.0 * (w * uv[2] + uuv[2]),
            ]
        };
        let length = 29.0_f64.sqrt();
        let forward = [-3.0 / length, -2.0 / length, -4.0 / length];
        let actual = rotate([0.0, 0.0, -1.0]);
        let error = [
            actual[0] - forward[0],
            actual[1] - forward[1],
            actual[2] - forward[2],
        ];
        assert!(error[0] * error[0] + error[1] * error[1] + error[2] * error[2] < 1.0e-12);
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
        assert_eq!(
            migrated["entities"][0]["transform_3d"]["rotation"],
            json!([0.1, 0.2, 0.3, 0.9])
        );
    }

    #[test]
    fn namespace_migration_refuses_ambiguous_duplicate_spellings() {
        let error = SceneMigrator::builtin()
            .migrate(json!({
                "format_version": 4,
                "entities": [{
                    "id": "player",
                    "components": {
                        "sindri.audio": { "clip": "old.wav" },
                        "sindri.audio.source": { "clip": "new.wav" }
                    }
                }]
            }))
            .unwrap_err();
        assert!(
            matches!(error, SceneMigrationError::Unconvertible(message) if message.contains("player") && message.contains("sindri.audio") && message.contains("sindri.audio.source"))
        );
    }

    #[test]
    fn unmigrated_versions_report_the_missing_step() {
        let migrator = SceneMigrator::new();
        let error = SceneDocument::from_json_migrated(&legacy_document(), &migrator).unwrap_err();
        assert!(matches!(
            error,
            SceneJsonError::Migration(SceneMigrationError::NoRegisteredStep {
                from_version: 0,
                supported: SCENE_FORMAT_VERSION,
            })
        ));
    }

    #[test]
    fn newer_documents_are_rejected_rather_than_guessed_at() {
        let migrator = SceneMigrator::new();
        let document = json!({ "format_version": SCENE_FORMAT_VERSION + 1, "entities": [] });
        assert_eq!(
            migrator.migrate(document),
            Err(SceneMigrationError::FromTheFuture {
                found: SCENE_FORMAT_VERSION + 1,
                supported: SCENE_FORMAT_VERSION,
            })
        );
    }

    #[test]
    fn registration_rejects_loops_duplicates_and_overshoot() {
        let mut migrator = SceneMigrator::new();
        assert_eq!(
            migrator.register(1, 1, rename_label_to_name),
            Err(SceneMigrationError::NonProgressingStep {
                from_version: 1,
                to_version: 1,
            })
        );
        assert_eq!(
            migrator.register(1, SCENE_FORMAT_VERSION + 1, rename_label_to_name),
            Err(SceneMigrationError::StepBeyondSupportedVersion {
                to_version: SCENE_FORMAT_VERSION + 1,
                supported: SCENE_FORMAT_VERSION,
            })
        );
        migrator.register(0, 1, rename_label_to_name).unwrap();
        assert_eq!(
            migrator.register(0, 1, rename_label_to_name),
            Err(SceneMigrationError::DuplicateStep { from_version: 0 })
        );
    }

    #[test]
    fn documents_without_a_version_are_rejected() {
        let migrator = SceneMigrator::new();
        assert_eq!(
            migrator.migrate(json!({ "entities": [] })),
            Err(SceneMigrationError::MissingFormatVersion)
        );
        assert_eq!(
            migrator.migrate(json!(["not", "a", "document"])),
            Err(SceneMigrationError::NotADocument)
        );
    }

    #[test]
    fn a_failing_step_surfaces_its_reason() {
        let mut migrator = SceneMigrator::new();
        migrator.register(0, 1, rename_label_to_name).unwrap();
        let error = migrator
            .migrate(json!({ "format_version": 0 }))
            .unwrap_err();
        assert!(matches!(
            error,
            SceneMigrationError::StepFailed {
                from_version: 0,
                ..
            }
        ));
    }
}
