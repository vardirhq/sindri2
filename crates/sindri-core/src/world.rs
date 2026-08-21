use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;
use thiserror::Error;

use crate::{
    EntityId, SceneDocument, SceneEntity, SceneEntityId, SceneError, SceneMetadata, Transform3D,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityData {
    pub source_id: Option<SceneEntityId>,
    pub name: Option<String>,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
    pub transform_3d: Option<Transform3D>,
    pub components: BTreeMap<String, Value>,
    /// Editor-only state that the runtime carries but never interprets.
    pub editor: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
struct EntitySlot {
    generation: u32,
    data: Option<EntityData>,
}

#[derive(Clone, Debug, Default)]
pub struct World {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
    len: usize,
    metadata: SceneMetadata,
}

impl World {
    pub fn spawn(&mut self, data: EntityData) -> EntityId {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.data = Some(data);
            return EntityId::new(index, slot.generation);
        }

        let index = u32::try_from(self.slots.len()).expect("entity capacity exceeded u32::MAX");
        self.slots.push(EntitySlot {
            generation: 0,
            data: Some(data),
        });
        EntityId::new(index, 0)
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.slot(entity).is_some_and(|slot| slot.data.is_some())
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, entity: EntityId) -> Option<&EntityData> {
        self.slot(entity)?.data.as_ref()
    }

    pub fn get_mut(&mut self, entity: EntityId) -> Option<&mut EntityData> {
        self.slot_mut(entity)?.data.as_mut()
    }

    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &EntityData)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.data.as_ref().map(|data| {
                (
                    EntityId::new(
                        u32::try_from(index).expect("entity index exceeded u32::MAX"),
                        slot.generation,
                    ),
                    data,
                )
            })
        })
    }

    /// Whether [`World::set_parent`] would accept this move, without making it.
    ///
    /// An interface that offers reparenting has to answer this before the move
    /// happens — a hierarchy refusing a drop while it is being dragged is a
    /// different thing from one that accepts it and then reports an error.
    /// Sharing the rule with `set_parent` is what keeps the two answers the
    /// same; asking it twice in two places is how they stop agreeing.
    pub fn check_set_parent(
        &self,
        child: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), WorldError> {
        self.require(child)?;
        if let Some(parent) = parent {
            self.require(parent)?;
            if child == parent || self.is_descendant(parent, child) {
                return Err(WorldError::HierarchyCycle);
            }
        }
        Ok(())
    }

    pub fn set_parent(
        &mut self,
        child: EntityId,
        parent: Option<EntityId>,
    ) -> Result<(), WorldError> {
        self.check_set_parent(child, parent)?;

        let previous = self.get(child).and_then(|data| data.parent);
        if previous == parent {
            return Ok(());
        }
        if let Some(previous) = previous
            && let Some(data) = self.get_mut(previous)
        {
            data.children.retain(|candidate| *candidate != child);
        }
        self.get_mut(child).expect("validated child").parent = parent;
        if let Some(parent) = parent {
            self.get_mut(parent)
                .expect("validated parent")
                .children
                .push(child);
        }
        Ok(())
    }

    /// The handle the next [`Self::spawn`] would hand out.
    ///
    /// Read-only, and deterministic given the world's state: a caller that
    /// needs to know an entity's handle *before* creating it — a command that
    /// must be reversible, an editor that wants to select what it just made —
    /// asks here and then creates it with [`Self::spawn_at`].
    #[must_use]
    pub fn next_handle(&self) -> EntityId {
        match self.free.last() {
            Some(index) => EntityId::new(*index, self.slots[*index as usize].generation),
            None => EntityId::new(u32::try_from(self.slots.len()).unwrap_or(u32::MAX), 0),
        }
    }

    /// Creates an entity at an exact handle, rather than at whichever one is
    /// free.
    ///
    /// This is what makes despawning reversible. A generation-checked handle
    /// normally changes when a slot is reused, so putting an entity back after
    /// an undo would hand out a *different* handle — and the selection, and
    /// every earlier command in the history naming the old one, would be left
    /// pointing at nothing.
    ///
    /// Restoring the generation does not weaken the check it exists for. Only
    /// the one handle being restored becomes valid again; every older handle to
    /// the same slot stays stale, because their generations are different
    /// numbers. And a handle to an entity that came back is not a use-after-free
    /// — the entity is there.
    ///
    /// Safe to use for undo because [`crate::CommandHistory`] undoes in strict
    /// order: reaching a despawn means everything after it has already been
    /// undone, so the slot it freed is free again. The occupied case is refused
    /// rather than assumed.
    pub fn spawn_at(&mut self, entity: EntityId, data: EntityData) -> Result<(), WorldError> {
        let index = entity.index() as usize;
        if index >= self.slots.len() {
            // Only ever reached for a handle `next_handle` invented past the
            // end, which is one slot at a time.
            self.slots.resize_with(index + 1, || EntitySlot {
                generation: 0,
                data: None,
            });
        }
        if self.slots[index].data.is_some() {
            return Err(WorldError::SlotOccupied(entity));
        }
        self.slots[index].generation = entity.generation();
        self.slots[index].data = Some(data);
        self.free.retain(|free| *free != entity.index());
        self.len += 1;
        Ok(())
    }

    /// Every entity in a subtree, parents before their children, with the data
    /// each holds right now.
    ///
    /// Captured before anything is removed, because removing an entity edits
    /// its parent's child list — so a capture taken during a removal would
    /// record lists already missing their siblings.
    pub fn capture_subtree(
        &self,
        entity: EntityId,
    ) -> Result<Vec<(EntityId, EntityData)>, WorldError> {
        self.require(entity)?;
        let mut captured = Vec::new();
        let mut queue = vec![entity];
        while let Some(current) = queue.pop() {
            let Some(data) = self.get(current) else {
                continue;
            };
            queue.extend(data.children.iter().copied());
            captured.push((current, data.clone()));
        }
        Ok(captured)
    }

    /// Where an entity sits among its parent's children, so putting it back
    /// puts it back in the same place rather than at the end.
    #[must_use]
    pub fn sibling_index(&self, entity: EntityId) -> Option<usize> {
        let parent = self.get(entity)?.parent?;
        self.get(parent)?
            .children
            .iter()
            .position(|child| *child == entity)
    }

    /// Puts an entity back into its parent's child list at a known position.
    pub fn relink_child(
        &mut self,
        entity: EntityId,
        sibling_index: Option<usize>,
    ) -> Result<(), WorldError> {
        let Some(parent) = self.get(entity).and_then(|data| data.parent) else {
            return Ok(());
        };
        let Some(data) = self.get_mut(parent) else {
            return Err(WorldError::InvalidEntity(parent));
        };
        if data.children.contains(&entity) {
            return Ok(());
        }
        let at = sibling_index
            .unwrap_or(data.children.len())
            .min(data.children.len());
        data.children.insert(at, entity);
        Ok(())
    }

    pub fn despawn_recursive(&mut self, entity: EntityId) -> Result<Vec<EntityId>, WorldError> {
        self.require(entity)?;
        let mut stack = vec![entity];
        let mut removed = Vec::new();
        while let Some(current) = stack.pop() {
            if let Some(data) = self.get(current) {
                stack.extend(data.children.iter().copied());
            }
            if self.remove_one(current) {
                removed.push(current);
            }
        }
        Ok(removed)
    }

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

    fn is_descendant(&self, candidate: EntityId, ancestor: EntityId) -> bool {
        let mut cursor = Some(candidate);
        while let Some(entity) = cursor {
            if entity == ancestor {
                return true;
            }
            cursor = self.get(entity).and_then(|data| data.parent);
        }
        false
    }

    fn require(&self, entity: EntityId) -> Result<(), WorldError> {
        self.contains(entity)
            .then_some(())
            .ok_or(WorldError::InvalidEntity(entity))
    }

    fn remove_one(&mut self, entity: EntityId) -> bool {
        let parent = self.get(entity).and_then(|data| data.parent);
        if let Some(parent) = parent
            && let Some(data) = self.get_mut(parent)
        {
            data.children.retain(|candidate| *candidate != entity);
        }
        let Some(slot) = self.slot_mut(entity) else {
            return false;
        };
        if slot.data.take().is_none() {
            return false;
        }
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(entity.index());
        self.len -= 1;
        true
    }

    fn slot(&self, entity: EntityId) -> Option<&EntitySlot> {
        let slot = self.slots.get(entity.index() as usize)?;
        (slot.generation == entity.generation()).then_some(slot)
    }

    fn slot_mut(&mut self, entity: EntityId) -> Option<&mut EntitySlot> {
        let slot = self.slots.get_mut(entity.index() as usize)?;
        (slot.generation == entity.generation()).then_some(slot)
    }
}

#[derive(Clone, Debug)]
pub struct LoadedScene {
    pub world: World,
    pub entity_map: HashMap<SceneEntityId, EntityId>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum WorldError {
    #[error("invalid or stale entity handle {0:?}")]
    InvalidEntity(EntityId),
    #[error("entity slot {0:?} is already occupied")]
    SlotOccupied(EntityId),
    #[error("entity hierarchy cannot contain a cycle")]
    HierarchyCycle,
    #[error("entity {0:?} has no stable scene ID and cannot be saved")]
    UnstableEntity(EntityId),
    #[error(
        "entity {0:?} declares its transform's Z locked; unlock it before \
         moving it off that layer"
    )]
    TransformZLocked(EntityId),
    #[error(transparent)]
    InvalidScene(#[from] SceneError),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn stale_handles_do_not_access_reused_slots() {
        let mut world = World::default();
        let first = world.spawn(EntityData::default());
        world.despawn_recursive(first).unwrap();
        let second = world.spawn(EntityData::default());
        assert_eq!(first.index(), second.index());
        assert_ne!(first.generation(), second.generation());
        assert!(world.get(first).is_none());
        assert!(world.get(second).is_some());
    }

    #[test]
    fn recursive_despawn_removes_hierarchy() {
        let mut world = World::default();
        let parent = world.spawn(EntityData::default());
        let child = world.spawn(EntityData::default());
        world.set_parent(child, Some(parent)).unwrap();
        let removed = world.despawn_recursive(parent).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(world.is_empty());
    }

    fn authored_scene() -> SceneDocument {
        let mut root = SceneEntity::new(SceneEntityId::new("root").unwrap());
        root.name = Some("Root".into());
        root.transform_3d = Some(Transform3D::default());
        root.components = BTreeMap::from([("game.marker".to_owned(), json!({ "kind": "spawn" }))]);
        root.editor = BTreeMap::from([("collapsed".to_owned(), json!(false))]);

        let mut child = SceneEntity::new(SceneEntityId::new("child").unwrap());
        child.parent = Some(SceneEntityId::new("root").unwrap());
        child.transform_3d = Some(Transform3D {
            position: [1.5, -2.25, 0.0],
            ..Transform3D::default()
        });

        SceneDocument {
            metadata: SceneMetadata {
                name: Some("Round trip".into()),
                editor: BTreeMap::from([("grid_snap".to_owned(), json!(0.25))]),
            },
            // Deliberately unsorted so the save has to canonicalize.
            entities: vec![root, child],
            ..SceneDocument::default()
        }
    }

    #[test]
    fn saving_a_loaded_world_reproduces_the_canonical_scene() {
        let authored = authored_scene();
        let loaded = World::from_scene(&authored).unwrap();
        let saved = loaded.world.to_scene().unwrap();
        assert_eq!(saved, authored.canonicalized());
        assert!(saved.is_canonical());
        assert_eq!(saved.metadata, authored.metadata);
        assert_eq!(
            saved
                .entity(&SceneEntityId::new("child").unwrap())
                .unwrap()
                .parent,
            Some(SceneEntityId::new("root").unwrap())
        );
    }

    #[test]
    fn editing_a_transform_survives_a_save_and_reload() {
        let authored = authored_scene();
        let loaded = World::from_scene(&authored).unwrap();
        let mut world = loaded.world;
        let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
        world.get_mut(child).unwrap().transform_3d = Some(Transform3D {
            position: [4.0, 8.0, -1.5],
            rotation: [0.0, 0.0, 0.247_404, 0.968_912],
            scale: [2.0, 2.0, 1.0],
            ..Transform3D::default()
        });

        let saved = world.to_scene().unwrap();
        let reloaded = World::from_scene(&saved).unwrap();
        let reloaded_child = reloaded.entity_map[&SceneEntityId::new("child").unwrap()];
        assert_eq!(
            reloaded.world.get(reloaded_child).unwrap().transform_3d,
            Some(Transform3D {
                position: [4.0, 8.0, -1.5],
                rotation: [0.0, 0.0, 0.247_404, 0.968_912],
                scale: [2.0, 2.0, 1.0],
                ..Transform3D::default()
            })
        );
        assert_eq!(reloaded.world.to_scene().unwrap(), saved);
    }

    #[test]
    fn reparenting_is_preserved_without_losing_stable_ids() {
        let authored = authored_scene();
        let loaded = World::from_scene(&authored).unwrap();
        let mut world = loaded.world;
        let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
        world.set_parent(child, None).unwrap();

        let saved = world.to_scene().unwrap();
        assert_eq!(
            saved
                .entity(&SceneEntityId::new("child").unwrap())
                .unwrap()
                .parent,
            None
        );
        assert_eq!(saved.entities.len(), 2);
    }

    #[test]
    fn runtime_entities_without_stable_ids_cannot_be_saved_silently() {
        let mut world = World::default();
        let spawned = world.spawn(EntityData::default());
        assert_eq!(world.to_scene(), Err(WorldError::UnstableEntity(spawned)));

        let assigned = world.assign_missing_source_ids("entity").unwrap();
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].1.as_str(), "entity-1");
        assert_eq!(world.to_scene().unwrap().entities.len(), 1);
    }

    #[test]
    fn assigned_ids_skip_identities_already_in_use() {
        let mut world = World::default();
        world.spawn(EntityData {
            source_id: Some(SceneEntityId::new("entity-1").unwrap()),
            ..EntityData::default()
        });
        world.spawn(EntityData::default());
        world.spawn(EntityData::default());

        let assigned = world.assign_missing_source_ids("entity").unwrap();
        let minted: Vec<_> = assigned
            .iter()
            .map(|(_, source_id)| source_id.as_str())
            .collect();
        assert_eq!(minted, ["entity-2", "entity-3"]);
        world.to_scene().unwrap().validate().unwrap();
    }

    #[test]
    fn saving_survives_slot_reuse_after_despawn() {
        let authored = authored_scene();
        let loaded = World::from_scene(&authored).unwrap();
        let mut world = loaded.world;
        let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
        world.despawn_recursive(child).unwrap();
        world.spawn(EntityData {
            source_id: Some(SceneEntityId::new("alpha").unwrap()),
            ..EntityData::default()
        });

        // The new entity reuses the despawned slot, so document order would
        // otherwise depend on allocation history rather than identity.
        let saved = world.to_scene().unwrap();
        assert_eq!(
            saved
                .entities
                .iter()
                .map(|entity| entity.id.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "root"]
        );
    }

    #[test]
    fn hierarchy_rejects_cycles() {
        let mut world = World::default();
        let parent = world.spawn(EntityData::default());
        let child = world.spawn(EntityData::default());
        world.set_parent(child, Some(parent)).unwrap();
        assert_eq!(
            world.set_parent(parent, Some(child)),
            Err(WorldError::HierarchyCycle)
        );
    }

    /// The check exists so an interface can refuse a drop before making it. It
    /// is only worth having if it answers exactly what the move would, so this
    /// asks both about every pair in a three-deep chain.
    #[test]
    fn checking_a_reparent_agrees_with_making_one() {
        let mut world = World::default();
        let root = world.spawn(EntityData::default());
        let middle = world.spawn(EntityData::default());
        let leaf = world.spawn(EntityData::default());
        world.set_parent(middle, Some(root)).unwrap();
        world.set_parent(leaf, Some(middle)).unwrap();
        let stale = world.spawn(EntityData::default());
        world.despawn_recursive(stale).unwrap();

        let entities = [root, middle, leaf, stale];
        for child in entities {
            for parent in entities.map(Some).into_iter().chain([None]) {
                let checked = world.check_set_parent(child, parent);
                let mut copy = world.clone();
                let made = copy.set_parent(child, parent);
                assert_eq!(
                    checked, made,
                    "check and move disagreed about {child:?} under {parent:?}"
                );
            }
        }
    }
}
