//! What a scene file holds, and what it refuses to hold.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{SceneMigrator, Transform3D};

use super::canonical::collapse_scalar_arrays;
use super::error::{SceneError, SceneJsonError};

pub const SCENE_FORMAT_VERSION: u32 = 7;

/// A stable, project-authored entity identifier used only in serialized data.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneEntityId(String);

impl SceneEntityId {
    pub fn new(value: impl Into<String>) -> Result<Self, SceneError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SceneError::EmptyEntityId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Document-level metadata.
///
/// Everything under `editor` is tooling state. Runtimes must load a scene
/// correctly while ignoring it, and shipping pipelines may remove it with
/// [`SceneDocument::strip_editor_metadata`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub editor: BTreeMap<String, Value>,
}

impl SceneMetadata {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.editor.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneDocument {
    pub format_version: u32,
    #[serde(default)]
    pub metadata: SceneMetadata,
    #[serde(default)]
    pub entities: Vec<SceneEntity>,
}

impl Default for SceneDocument {
    fn default() -> Self {
        Self {
            format_version: SCENE_FORMAT_VERSION,
            metadata: SceneMetadata::default(),
            entities: Vec::new(),
        }
    }
}

impl SceneDocument {
    /// Parses a scene without applying migrations.
    ///
    /// Documents that do not already declare [`SCENE_FORMAT_VERSION`] are
    /// rejected rather than silently reinterpreted.
    pub fn from_json(json: &str) -> Result<Self, SceneJsonError> {
        let document: Self = serde_json::from_str(json)?;
        document.validate()?;
        Ok(document)
    }

    /// Parses a scene, stepping older documents up to the current format with
    /// `migrator` before deserializing.
    pub fn from_json_migrated(
        json: &str,
        migrator: &SceneMigrator,
    ) -> Result<Self, SceneJsonError> {
        let raw: Value = serde_json::from_str(json)?;
        let migrated = migrator.migrate(raw)?;
        let document: Self = serde_json::from_value(migrated)?;
        document.validate()?;
        Ok(document)
    }

    /// Serializes the canonical form of this document.
    ///
    /// The output is deterministic, ends with a trailing newline, and re-parses
    /// to an equal document. Serializing an already canonical document is a
    /// fixed point, so files written this way produce minimal review diffs.
    pub fn to_canonical_json(&self) -> Result<String, SceneJsonError> {
        let canonical = self.canonicalized();
        canonical.validate()?;
        let mut json = collapse_scalar_arrays(&serde_json::to_string_pretty(&canonical)?);
        json.push('\n');
        Ok(json)
    }

    /// Returns the canonical ordering of this document.
    #[must_use]
    pub fn canonicalized(&self) -> Self {
        let mut canonical = self.clone();
        canonical.canonicalize();
        canonical
    }

    /// Reorders this document into canonical form.
    ///
    /// Entities are sorted by their stable ID. Document order carries no
    /// rendering meaning: draw order is expressed by explicit render layers and
    /// depths, so sorting keeps saves stable while entities are added, removed,
    /// and reparented.
    pub fn canonicalize(&mut self) {
        self.entities.sort_by(|left, right| left.id.cmp(&right.id));
    }

    pub fn is_canonical(&self) -> bool {
        self.entities.is_sorted_by(|left, right| left.id < right.id)
    }

    /// Removes every editor-only section from the document and its entities.
    pub fn strip_editor_metadata(&mut self) {
        self.metadata.editor.clear();
        for entity in &mut self.entities {
            entity.editor.clear();
        }
    }

    pub fn entity(&self, id: &SceneEntityId) -> Option<&SceneEntity> {
        self.entities.iter().find(|entity| &entity.id == id)
    }

    pub fn validate(&self) -> Result<(), SceneError> {
        if self.format_version != SCENE_FORMAT_VERSION {
            return Err(SceneError::UnsupportedVersion {
                found: self.format_version,
                supported: SCENE_FORMAT_VERSION,
            });
        }

        if self
            .entities
            .iter()
            .any(|entity| entity.id.as_str().trim().is_empty())
        {
            return Err(SceneError::EmptyEntityId);
        }

        let ids: HashSet<_> = self.entities.iter().map(|entity| &entity.id).collect();
        if ids.len() != self.entities.len() {
            return Err(SceneError::DuplicateEntityId);
        }

        for entity in &self.entities {
            if let Some(transform) = &entity.transform_3d
                && !transform_3d_is_finite(transform)
            {
                return Err(SceneError::NonFiniteTransform(entity.id.clone()));
            }

            if let Some(parent) = &entity.parent {
                if parent == &entity.id {
                    return Err(SceneError::HierarchyCycle(entity.id.clone()));
                }
                if !ids.contains(parent) {
                    return Err(SceneError::MissingParent {
                        entity: entity.id.clone(),
                        parent: parent.clone(),
                    });
                }
            }
        }

        self.reject_hierarchy_cycles()
    }

    /// Rejects a scene where following parents does not always reach a root.
    ///
    /// Each entity used to walk its own ancestors, and each step of that walk
    /// searched the whole entity list for the parent it named, so validating a
    /// scene cost time proportional to its size squared: ten thousand entities
    /// spent about 1.4 seconds here, and every load, save, and canonical
    /// serialization pays it.
    ///
    /// Parents resolve through a map instead, and an entity proven to reach a
    /// root is remembered, so a chain shared by many entities is walked once
    /// rather than once per descendant.
    fn reject_hierarchy_cycles(&self) -> Result<(), SceneError> {
        let parents: HashMap<&SceneEntityId, Option<&SceneEntityId>> = self
            .entities
            .iter()
            .map(|entity| (&entity.id, entity.parent.as_ref()))
            .collect();

        let mut grounded: HashSet<&SceneEntityId> = HashSet::with_capacity(self.entities.len());
        let mut walked: HashSet<&SceneEntityId> = HashSet::new();
        let mut path: Vec<&SceneEntityId> = Vec::new();

        for entity in &self.entities {
            walked.clear();
            path.clear();
            let mut cursor = Some(&entity.id);
            while let Some(current) = cursor {
                if grounded.contains(current) {
                    break;
                }
                if !walked.insert(current) {
                    return Err(SceneError::HierarchyCycle(entity.id.clone()));
                }
                path.push(current);
                cursor = parents.get(current).copied().flatten();
            }
            grounded.extend(path.iter().copied());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: SceneEntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SceneEntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_3d: Option<Transform3D>,
    /// Forward-compatible component payloads keyed by registered component name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<String, Value>,
    /// Editor-only state that runtimes must ignore.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub editor: BTreeMap<String, Value>,
}

impl SceneEntity {
    pub fn new(id: SceneEntityId) -> Self {
        Self {
            id,
            name: None,
            parent: None,
            transform_3d: None,
            components: BTreeMap::new(),
            editor: BTreeMap::new(),
        }
    }
}

fn transform_3d_is_finite(transform: &Transform3D) -> bool {
    transform.position.iter().all(|value| value.is_finite())
        && transform.rotation.iter().all(|value| value.is_finite())
        && transform.scale.iter().all(|value| value.is_finite())
}
