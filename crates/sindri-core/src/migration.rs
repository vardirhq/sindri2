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
