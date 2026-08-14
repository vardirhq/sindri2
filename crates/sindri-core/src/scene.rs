use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{Transform2D, Transform3D};

pub const SCENE_FORMAT_VERSION: u32 = 1;

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneMetadata {
    pub name: Option<String>,
    pub editor: BTreeMap<String, Value>,
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

            let mut visited = HashSet::new();
            let mut cursor = entity.parent.as_ref();
            while let Some(parent) = cursor {
                if !visited.insert(parent) {
                    return Err(SceneError::HierarchyCycle(entity.id.clone()));
                }
                cursor = self
                    .entities
                    .iter()
                    .find(|candidate| &candidate.id == parent)
                    .and_then(|candidate| candidate.parent.as_ref());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneEntity {
    pub id: SceneEntityId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub parent: Option<SceneEntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_2d: Option<Transform2D>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_3d: Option<Transform3D>,
    /// Forward-compatible component payloads keyed by registered component name.
    #[serde(default)]
    pub components: BTreeMap<String, Value>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: &str, parent: Option<&str>) -> SceneEntity {
        SceneEntity {
            id: SceneEntityId::new(id).unwrap(),
            name: None,
            parent: parent.map(|value| SceneEntityId::new(value).unwrap()),
            transform_2d: None,
            transform_3d: None,
            components: BTreeMap::new(),
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
}
