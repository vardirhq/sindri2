//! Building a world from an authored scene, and writing one back.

use std::collections::{HashMap, HashSet};

use crate::{EntityId, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneMetadata};

use super::{EntityData, World, WorldError};

#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub world: World,
    pub entity_map: HashMap<SceneEntityId, EntityId>,
}

impl World {
    pub fn from_scene(scene: &SceneDocument) -> Result<LoadedScene, WorldError> {
        scene.validate()?;
        let mut world = Self {
            metadata: scene.metadata.clone(),
            ..Self::default()
        };
        let mut entity_map = HashMap::new();

        for entity in &scene.entities {
            let runtime = world.spawn(EntityData {
                source_id: Some(entity.id.clone()),
                name: entity.name.clone(),
                transform_3d: entity.transform_3d,
                components: entity.components.clone(),
                editor: entity.editor.clone(),
                ..EntityData::default()
            });
            entity_map.insert(entity.id.clone(), runtime);
        }

        for entity in &scene.entities {
            if let Some(parent) = &entity.parent {
                world.set_parent(entity_map[&entity.id], Some(entity_map[parent]))?;
            }
        }

        Ok(LoadedScene { world, entity_map })
    }

    /// Document-level metadata carried through a load/edit/save cycle.
    pub const fn metadata(&self) -> &SceneMetadata {
        &self.metadata
    }

    pub fn set_metadata(&mut self, metadata: SceneMetadata) {
        self.metadata = metadata;
    }

    /// Serializes this world back into a canonical scene document.
    ///
    /// Stable IDs are preserved rather than regenerated, so saving a loaded
    /// scene reproduces the authored identities. Entities spawned at runtime
    /// have no stable ID and are reported instead of being silently dropped or
    /// given an arbitrary one; call [`World::assign_missing_source_ids`] first
    /// to give them persistent identities.
    pub fn to_scene(&self) -> Result<SceneDocument, WorldError> {
        let mut entities = Vec::with_capacity(self.len);
        for (entity_id, data) in self.entities() {
            let source_id = data
                .source_id
                .clone()
                .ok_or(WorldError::UnstableEntity(entity_id))?;
            let parent = match data.parent {
                Some(parent) => Some(
                    self.get(parent)
                        .and_then(|parent_data| parent_data.source_id.clone())
                        .ok_or(WorldError::UnstableEntity(parent))?,
                ),
                None => None,
            };
            entities.push(SceneEntity {
                name: data.name.clone(),
                parent,
                transform_3d: data.transform_3d,
                components: data.components.clone(),
                editor: data.editor.clone(),
                ..SceneEntity::new(source_id)
            });
        }

        let mut document = SceneDocument {
            format_version: crate::SCENE_FORMAT_VERSION,
            metadata: self.metadata.clone(),
            entities,
        };
        document.canonicalize();
        document.validate()?;
        Ok(document)
    }

    /// Gives every runtime-spawned entity a stable ID derived from `prefix`.
    ///
    /// IDs are minted in entity index order and skip identities already in use,
    /// so the same world always produces the same assignment. Returns the
    /// entities that gained an ID.
    pub fn assign_missing_source_ids(
        &mut self,
        prefix: &str,
    ) -> Result<Vec<(EntityId, SceneEntityId)>, SceneError> {
        let mut taken: HashSet<SceneEntityId> = self
            .entities()
            .filter_map(|(_, data)| data.source_id.clone())
            .collect();
        let pending: Vec<EntityId> = self
            .entities()
            .filter(|(_, data)| data.source_id.is_none())
            .map(|(entity_id, _)| entity_id)
            .collect();

        let mut assigned = Vec::with_capacity(pending.len());
        let mut next = 1_u32;
        for entity_id in pending {
            let source_id = loop {
                let candidate = SceneEntityId::new(format!("{prefix}-{next}"))?;
                next += 1;
                if !taken.contains(&candidate) {
                    break candidate;
                }
            };
            taken.insert(source_id.clone());
            self.get_mut(entity_id)
                .expect("entity listed by this world")
                .source_id = Some(source_id.clone());
            assigned.push((entity_id, source_id));
        }
        Ok(assigned)
    }
}
