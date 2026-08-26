//! Opening a scene written by an older version of Sindri.
//!
//! A document written against an older format is upgraded step by step until
//! it declares the version this runtime understands. Steps work on raw JSON,
//! because an old document cannot by definition be deserialized into the
//! current [`crate::SceneDocument`].
//!
//! Each step lives in `step/`, and the ordered chain of them is
//! [`SceneMigrator::builtin`]. A new format version adds a file there and one
//! `register` line here; nothing already written changes, because a step that
//! has run against a file someone wrote must keep producing what it produced.

mod step;

#[cfg(test)]
mod tests;

use step::{
    camera::{move_camera_look_at_into_transform, remove_legacy_overlay_camera},
    namespace::namespace_components,
    sprites::{name_the_parts_of_a_sheet, sort_sprites_by_where_they_are},
    transform::collapse_transform_2d,
    ui::split_the_screen_from_the_world,
};

use std::collections::BTreeMap;

use serde_json::Value;
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
            .register(6, 7, remove_legacy_overlay_camera)
            .expect("built-in steps are registered once and move forward");
        migrator
            .register(7, 8, split_the_screen_from_the_world)
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
