use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{SceneMigrationError, SceneMigrator, Transform3D};

pub const SCENE_FORMAT_VERSION: u32 = 5;

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

/// The column budget for keeping an array of scalars on one line.
const INLINE_ARRAY_WIDTH: usize = 96;

/// Collapses arrays that contain only scalars onto a single line.
///
/// `serde_json` gives every array element its own line, which turns a
/// three-component position into five lines and buries real changes in review.
/// An array of scalars is unambiguous on one line, so it is collapsed whenever
/// the result stays inside [`INLINE_ARRAY_WIDTH`] columns. Arrays holding
/// objects or nested arrays, and scalar arrays too long to fit, keep the
/// expanded form. The decision depends only on the already deterministic
/// pretty output, so canonical serialization stays a fixed point.
fn collapse_scalar_arrays(pretty: &str) -> String {
    let bytes = pretty.as_bytes();
    let mut output = String::with_capacity(pretty.len());
    let mut index = 0;
    let mut line_start = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                output.push('\n');
                index += 1;
                line_start = output.len();
            }
            b'"' => {
                let end = string_end(bytes, index);
                output.push_str(&pretty[index..end]);
                index = end;
            }
            b'[' => {
                let inlined = scalar_array_end(bytes, index)
                    .map(|end| (inline_array(&pretty[index..end]), end));
                match inlined {
                    Some((inline, end))
                        if output.len() - line_start + inline.len() < INLINE_ARRAY_WIDTH =>
                    {
                        output.push_str(&inline);
                        index = end;
                    }
                    _ => {
                        output.push('[');
                        index += 1;
                    }
                }
            }
            _ => {
                let character = pretty[index..]
                    .chars()
                    .next()
                    .expect("index stays on a character boundary");
                output.push(character);
                index += character.len_utf8();
            }
        }
    }
    output
}

/// Returns the index just past the closing quote of the string starting at `start`.
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// Returns the index just past the closing bracket when the array starting at
/// `start` holds only scalars, or `None` when it nests objects or arrays.
fn scalar_array_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index),
            b']' => return Some(index + 1),
            b'[' | b'{' | b'}' => return None,
            _ => index += 1,
        }
    }
    None
}

/// Rewrites an already validated scalar array as a single line.
fn inline_array(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    output.push('[');
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b']' => break,
            b',' => {
                output.push_str(", ");
                index += 1;
            }
            b'"' => {
                let end = string_end(bytes, index);
                output.push_str(&source[index..end]);
                index = end;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            _ => {
                let character = source[index..]
                    .chars()
                    .next()
                    .expect("index stays on a character boundary");
                output.push(character);
                index += character.len_utf8();
            }
        }
    }
    output.push(']');
    output
}

fn transform_3d_is_finite(transform: &Transform3D) -> bool {
    transform.position.iter().all(|value| value.is_finite())
        && transform.rotation.iter().all(|value| value.is_finite())
        && transform.scale.iter().all(|value| value.is_finite())
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SceneError {
    #[error("scene entity IDs cannot be empty")]
    EmptyEntityId,
    #[error("scene contains duplicate entity IDs")]
    DuplicateEntityId,
    #[error("scene format {found} is unsupported; this runtime supports {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("entity {entity:?} refers to missing parent {parent:?}")]
    MissingParent {
        entity: SceneEntityId,
        parent: SceneEntityId,
    },
    #[error("hierarchy cycle detected at entity {0:?}")]
    HierarchyCycle(SceneEntityId),
    #[error("entity {0:?} has a transform containing a non-finite value")]
    NonFiniteTransform(SceneEntityId),
}

/// Failures raised while reading or writing serialized scenes.
///
/// [`SceneError`] stays comparable and free of I/O concerns; this type carries
/// the JSON and migration failures that surround it.
#[derive(Debug, Error)]
pub enum SceneJsonError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Invalid(#[from] SceneError),
    #[error(transparent)]
    Migration(#[from] SceneMigrationError),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn entity(id: &str, parent: Option<&str>) -> SceneEntity {
        SceneEntity {
            parent: parent.map(|value| SceneEntityId::new(value).unwrap()),
            ..SceneEntity::new(SceneEntityId::new(id).unwrap())
        }
    }

    #[test]
    fn rejects_hierarchy_cycles() {
        let scene = SceneDocument {
            entities: vec![entity("a", Some("b")), entity("b", Some("a"))],
            ..SceneDocument::default()
        };
        assert!(matches!(
            scene.validate(),
            Err(SceneError::HierarchyCycle(_))
        ));
    }

    /// Entities sharing one ancestor chain are the case memoisation exists for:
    /// the chain must be walked once, and still be correct for every entity.
    #[test]
    fn a_long_shared_chain_validates() {
        let mut entities = vec![entity("root", None)];
        for index in 1..2_000 {
            entities.push(entity(
                &format!("node-{index}"),
                Some(&format!("node-{}", index - 1)),
            ));
        }
        // The second entity's parent is the root rather than "node-0".
        entities[1].parent = Some(SceneEntityId::new("root").unwrap());

        let scene = SceneDocument {
            entities,
            ..SceneDocument::default()
        };
        assert_eq!(scene.validate(), Ok(()));
    }

    /// Remembering that an entity reaches a root must never let a cycle pass:
    /// nothing on a path that loops is ever recorded as grounded.
    #[test]
    fn a_cycle_behind_a_long_chain_is_still_caught() {
        let mut entities = vec![entity("a", Some("b")), entity("b", Some("a"))];
        for index in 0..500 {
            let parent = if index == 0 {
                "a".to_owned()
            } else {
                format!("tail-{}", index - 1)
            };
            entities.push(entity(&format!("tail-{index}"), Some(&parent)));
        }

        let scene = SceneDocument {
            entities,
            ..SceneDocument::default()
        };
        assert!(matches!(
            scene.validate(),
            Err(SceneError::HierarchyCycle(_))
        ));
    }

    #[test]
    fn rejects_non_finite_transforms() {
        let scene = SceneDocument {
            entities: vec![SceneEntity {
                transform_3d: Some(Transform3D {
                    position: [f32::NAN, 0.0, 0.0],
                    ..Transform3D::default()
                }),
                ..entity("drifting", None)
            }],
            ..SceneDocument::default()
        };
        assert_eq!(
            scene.validate(),
            Err(SceneError::NonFiniteTransform(
                SceneEntityId::new("drifting").unwrap()
            ))
        );
    }

    #[test]
    fn round_trips_scene_json() {
        let scene = SceneDocument {
            metadata: SceneMetadata {
                name: Some("Test".into()),
                editor: BTreeMap::new(),
            },
            entities: vec![entity("player", None)],
            ..SceneDocument::default()
        };
        let json = serde_json::to_string(&scene).unwrap();
        let decoded: SceneDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, scene);
        decoded.validate().unwrap();
    }

    #[test]
    fn canonical_json_sorts_entities_and_is_a_fixed_point() {
        let scene = SceneDocument {
            entities: vec![entity("zeta", None), entity("alpha", None)],
            ..SceneDocument::default()
        };
        let json = scene.to_canonical_json().unwrap();
        assert!(json.ends_with('\n'));

        let decoded = SceneDocument::from_json(&json).unwrap();
        assert_eq!(
            decoded
                .entities
                .iter()
                .map(|entity| entity.id.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
        assert!(decoded.is_canonical());
        assert_eq!(decoded.to_canonical_json().unwrap(), json);
    }

    #[test]
    fn canonical_json_omits_empty_sections() {
        let scene = SceneDocument {
            entities: vec![entity("solo", None)],
            ..SceneDocument::default()
        };
        let json = scene.to_canonical_json().unwrap();
        assert!(!json.contains("null"));
        assert!(!json.contains("components"));
        assert!(!json.contains("editor"));
        assert!(json.contains(&format!("\"format_version\": {SCENE_FORMAT_VERSION}")));
    }

    #[test]
    fn editor_metadata_round_trips_and_can_be_stripped() {
        let mut scene = SceneDocument {
            metadata: SceneMetadata {
                name: Some("Authored".into()),
                editor: BTreeMap::from([("camera_bookmark".to_owned(), json!([1.0, 2.0]))]),
            },
            entities: vec![SceneEntity {
                editor: BTreeMap::from([("collapsed".to_owned(), json!(true))]),
                ..entity("root", None)
            }],
            ..SceneDocument::default()
        };

        let json = scene.to_canonical_json().unwrap();
        assert_eq!(SceneDocument::from_json(&json).unwrap(), scene);

        scene.strip_editor_metadata();
        let stripped = scene.to_canonical_json().unwrap();
        assert!(!stripped.contains("collapsed"));
        assert!(!stripped.contains("camera_bookmark"));
        assert!(stripped.contains("Authored"));

        // Runtimes ignore editor state, so stripping must not change the scene.
        let with_editor = SceneDocument::from_json(&json).unwrap();
        let without_editor = SceneDocument::from_json(&stripped).unwrap();
        assert_eq!(with_editor.entities.len(), without_editor.entities.len());
        assert_eq!(
            with_editor.entities[0].transform_3d,
            without_editor.entities[0].transform_3d
        );
    }

    #[test]
    fn scalar_arrays_are_inlined_but_structured_arrays_are_not() {
        let scene = SceneDocument {
            entities: vec![SceneEntity {
                transform_3d: Some(Transform3D {
                    position: [3.0, 2.0, 4.0],
                    ..Transform3D::default()
                }),
                components: BTreeMap::from([(
                    "game.path".to_owned(),
                    json!({ "waypoints": [[0, 1], [2, 3]], "tags": ["a", "b"] }),
                )]),
                ..entity("mover", None)
            }],
            ..SceneDocument::default()
        };

        let json = scene.to_canonical_json().unwrap();
        assert!(json.contains("\"position\": [3.0, 2.0, 4.0]"));
        assert!(json.contains("\"tags\": [\"a\", \"b\"]"));
        // An array of arrays keeps one element per line.
        assert!(json.contains("\"waypoints\": [\n"));
        assert_eq!(SceneDocument::from_json(&json).unwrap(), scene);
        assert_eq!(
            SceneDocument::from_json(&json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            json
        );
    }

    #[test]
    fn long_scalar_arrays_stay_expanded() {
        let scene = SceneDocument {
            entities: vec![SceneEntity {
                components: BTreeMap::from([(
                    "game.tilemap".to_owned(),
                    json!({ "tiles": (0..64).collect::<Vec<u32>>() }),
                )]),
                ..entity("map", None)
            }],
            ..SceneDocument::default()
        };

        let json = scene.to_canonical_json().unwrap();
        assert!(json.contains("\"tiles\": [\n"));
        assert_eq!(
            SceneDocument::from_json(&json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            json
        );
    }

    #[test]
    fn brackets_inside_strings_do_not_confuse_the_formatter() {
        let scene = SceneDocument {
            entities: vec![SceneEntity {
                name: Some("a [ b { c \" d ] e".into()),
                ..entity("tricky", None)
            }],
            ..SceneDocument::default()
        };

        let json = scene.to_canonical_json().unwrap();
        let decoded = SceneDocument::from_json(&json).unwrap();
        assert_eq!(decoded, scene);
        assert_eq!(decoded.to_canonical_json().unwrap(), json);
    }

    #[test]
    fn unknown_document_versions_are_rejected() {
        let json = r#"{"format_version": 99, "entities": []}"#;
        assert!(matches!(
            SceneDocument::from_json(json),
            Err(SceneJsonError::Invalid(SceneError::UnsupportedVersion {
                found: 99,
                supported: SCENE_FORMAT_VERSION,
            }))
        ));
    }
}
