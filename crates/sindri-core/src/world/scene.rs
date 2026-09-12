//! Building a world from an authored scene, and writing one back.

use std::collections::{HashMap, HashSet};

use crate::{EntityId, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneMetadata};

use super::{EntityData, World, WorldError};

#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub world: World,
    pub entity_map: HashMap<SceneEntityId, EntityId>,
}

/// What [`World::add_scene`] put into an existing world.
#[derive(Clone, Debug)]
pub struct AddedScene {
    /// The scene's own IDs to the entities they became. Keyed by the ID as the
    /// file spells it, not as the world now holds it, because a caller reading
    /// its own scene knows the former and not the latter.
    pub entity_map: HashMap<SceneEntityId, EntityId>,
    /// The entities the scene authored at its root, in file order.
    ///
    /// With no parent given these are the world's new roots; with one they are
    /// now its children. Either way they are what a caller switches to take the
    /// scene out of play.
    pub roots: Vec<EntityId>,
    /// The scene's own IDs to the namespaced ones the world now holds.
    pub source_ids: HashMap<SceneEntityId, SceneEntityId>,
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
                disabled: entity.disabled,
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

    /// Loads a scene *into* this world rather than building a new one.
    ///
    /// This is what more than one scene at a time is built on. A game with a
    /// farm and a farmhouse wants both scenes live: walking indoors has to
    /// leave the crops growing, and reloading the farm from its file on the way
    /// back out would reset them, because the file holds the *authored* state
    /// and not the *played* one. So a scene is added and switched off rather
    /// than loaded and dropped.
    ///
    /// `under`, when given, becomes the parent of every entity the scene
    /// authored at its root. That is the switch: [`World::is_active`] walks
    /// ancestors, so disabling that one entity takes the whole scene out of
    /// drawing, stepping, scripting and picking without touching anything
    /// inside it.
    ///
    /// # Stable identities
    ///
    /// A scene's entity IDs are unique within that scene and nowhere else: two
    /// interiors may each author a `door`. Since [`World::source_id_map`] and
    /// [`World::entity_for_source_id`] answer for the whole world, every ID is
    /// prefixed with `namespace` on the way in, so `door` from `house` becomes
    /// `house/door`. A collision after that is a caller giving two scenes the
    /// same namespace, and is refused rather than resolved.
    ///
    /// An **empty** namespace keeps the authored identities exactly as the file
    /// spells them. That is for a project's own opening scene: everything that
    /// resolves an entity by stable ID — an editor showing a running world, an
    /// authoring proposal naming a dozen entities — was written against the
    /// identities in the file, and a runtime that silently renamed them would
    /// make the two disagree. The cost is that an unnamespaced scene can
    /// collide with another, which is refused the same way.
    ///
    /// This is a runtime capability. [`World::to_scene`] writes one document,
    /// so a world holding several scenes does not round-trip back into the
    /// files it came from — the editor still edits one scene at a time.
    pub fn add_scene(
        &mut self,
        scene: &SceneDocument,
        namespace: &str,
        under: Option<EntityId>,
    ) -> Result<AddedScene, WorldError> {
        scene.validate()?;
        if let Some(under) = under
            && self.get(under).is_none()
        {
            return Err(WorldError::InvalidEntity(under));
        }
        let taken: HashSet<SceneEntityId> = self
            .entities()
            .filter_map(|(_, data)| data.source_id.clone())
            .collect();

        // Every identity is resolved before anything is spawned, so a
        // collision leaves the world exactly as it was rather than half
        // holding a scene nobody asked for.
        let mut namespaced = HashMap::with_capacity(scene.entities.len());
        for entity in &scene.entities {
            let id = if namespace.is_empty() {
                entity.id.clone()
            } else {
                SceneEntityId::new(format!("{namespace}/{}", entity.id.as_str()))?
            };
            if taken.contains(&id) {
                return Err(WorldError::DuplicateSourceId(id));
            }
            namespaced.insert(entity.id.clone(), id);
        }

        let mut entity_map = HashMap::with_capacity(scene.entities.len());
        for entity in &scene.entities {
            let runtime = self.spawn(EntityData {
                source_id: Some(namespaced[&entity.id].clone()),
                name: entity.name.clone(),
                transform_3d: entity.transform_3d,
                components: entity.components.clone(),
                disabled: entity.disabled,
                editor: entity.editor.clone(),
                ..EntityData::default()
            });
            entity_map.insert(entity.id.clone(), runtime);
        }

        let mut roots = Vec::new();
        for entity in &scene.entities {
            let child = entity_map[&entity.id];
            if let Some(parent) = &entity.parent {
                self.set_parent(child, Some(entity_map[parent]))?;
            } else {
                roots.push(child);
                if under.is_some() {
                    self.set_parent(child, under)?;
                }
            }
        }

        Ok(AddedScene {
            entity_map,
            roots,
            source_ids: namespaced,
        })
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
                disabled: data.disabled,
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

    /// The entity a scene's stable identity names, if the world still holds it.
    ///
    /// The one direction that was missing. A world could *mint* stable IDs and
    /// *write* them out, but nothing could read one back — so any caller
    /// holding a serialized identity had to walk every entity itself and decide
    /// what to do about a tie. That is the shape of question an authoring tool
    /// asks constantly: a proposal, a saved selection, a console line, and a
    /// prefab reference all name entities the way the *file* does, never by the
    /// runtime handle, because a handle is meaningless the moment the scene is
    /// reloaded.
    ///
    /// Unambiguous by construction: `WorldCommand::SetSourceId` refuses to give
    /// two entities the same stable ID, and a scene carrying a duplicate fails
    /// to load. So this returns the entity or nothing, and never has to pick.
    #[must_use]
    pub fn entity_for_source_id(&self, source_id: &SceneEntityId) -> Option<EntityId> {
        self.entities().find_map(|(entity, data)| {
            (data.source_id.as_ref() == Some(source_id)).then_some(entity)
        })
    }

    /// Every stable identity the world currently holds, and what it names.
    ///
    /// For a caller resolving many references at once — an authoring proposal
    /// naming a dozen entities, say. One pass rather than a dozen, and the map
    /// is the same one [`World::from_scene`] hands back, so a caller that has
    /// just loaded a scene need not build it twice.
    #[must_use]
    pub fn source_id_map(&self) -> HashMap<SceneEntityId, EntityId> {
        self.entities()
            .filter_map(|(entity, data)| {
                data.source_id.clone().map(|source_id| (source_id, entity))
            })
            .collect()
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
