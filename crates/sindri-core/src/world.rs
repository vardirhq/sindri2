use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use thiserror::Error;

use crate::{EntityId, SceneDocument, SceneEntityId, SceneError, Transform2D, Transform3D};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityData {
    pub source_id: Option<SceneEntityId>,
    pub name: Option<String>,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
    pub transform_2d: Option<Transform2D>,
    pub transform_3d: Option<Transform3D>,
    pub components: BTreeMap<String, Value>,
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

    pub fn set_parent(
        &mut self,
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
        let mut world = Self::default();
        let mut entity_map = HashMap::new();

        for entity in &scene.entities {
            let runtime = world.spawn(EntityData {
                source_id: Some(entity.id.clone()),
                name: entity.name.clone(),
                transform_2d: entity.transform_2d,
                transform_3d: entity.transform_3d,
                components: entity.components.clone(),
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
    #[error("entity hierarchy cannot contain a cycle")]
    HierarchyCycle,
    #[error(transparent)]
    InvalidScene(#[from] SceneError),
}

#[cfg(test)]
mod tests {
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
}
