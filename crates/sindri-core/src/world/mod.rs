//! The runtime world.
//!
//! Gameplay writes the world; everything derived from it — renderer state,
//! navigation — is derived elsewhere. Entities are addressed by a
//! generation-checked [`EntityId`], which is not the [`SceneEntityId`] a file
//! carries: `scene` is the seam between the two.

mod hierarchy;
mod prefab;
mod scene;
mod space;

#[cfg(test)]
mod tests;

pub use prefab::SpawnedPrefab;
pub use scene::{AddedScene, LoadedScene};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use thiserror::Error;

use crate::{EntityId, PrefabError, SceneEntityId, SceneError, SceneMetadata, Transform3D};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityData {
    pub source_id: Option<SceneEntityId>,
    pub name: Option<String>,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
    pub transform_3d: Option<Transform3D>,
    pub components: BTreeMap<String, Value>,
    /// Whether this entity has been switched off.
    ///
    /// Off means it takes no part in the scene: nothing it carries is drawn,
    /// stepped, scripted or picked, and neither is anything under it. It is
    /// still in the world and still in the file — that is the difference
    /// between disabling something and deleting it, and it is why the answer is
    /// a flag rather than a delete you undo.
    ///
    /// `false` by default, so an entity that says nothing is on. See
    /// [`World::is_active`], which is the question anything drawing or stepping
    /// actually asks, because a parent's switch governs its children too.
    pub disabled: bool,
    /// Editor-only state that the runtime carries but never interprets.
    pub editor: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
struct EntitySlot {
    generation: u32,
    /// When this slot last changed, on the world's own clock.
    ///
    /// Not a count of changes: the world hands out a fresh number for each one,
    /// so a value is unique across the whole world and a slot cannot be
    /// mistaken for its former occupant after being freed and spawned into.
    revision: u64,
    data: Option<EntityData>,
}

/// Hands out entity revisions, for every world in the process.
///
/// Not per world, and the difference is a scene drawn as the one opened before
/// it. A world counting its own changes starts at zero, so an entity in a
/// freshly loaded scene carries the same revision as a different entity with
/// the same handle in the scene it replaced -- and anything that kept a derived
/// answer from the old one would conclude it still held. Cloning a world has
/// the same shape: a clone that counted from where its original left off would
/// hand the same number to a different edit.
static CLOCK: AtomicU64 = AtomicU64::new(0);

/// A revision nothing else has had.
fn tick() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed).saturating_add(1)
}

#[derive(Clone, Debug, Default)]
pub struct World {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
    len: usize,
    metadata: SceneMetadata,
}

impl World {
    /// Whether this entity takes part in the scene.
    ///
    /// False for an entity that has been switched off, for anything under one,
    /// and for a handle the world no longer holds. Ancestors are walked because
    /// switching off a HUD has to switch off the HUD, not leave its five pips
    /// drawn over nothing — and because the alternative, writing the flag down
    /// through a subtree, makes re-enabling ambiguous for a child that was off
    /// on its own account.
    #[must_use]
    pub fn is_active(&self, entity: EntityId) -> bool {
        let mut cursor = Some(entity);
        // Bounded by the number of entities: `check_set_parent` refuses to make
        // a cycle, and a handle the world has lost ends the walk.
        while let Some(current) = cursor {
            let Some(data) = self.get(current) else {
                return false;
            };
            if data.disabled {
                return false;
            }
            cursor = data.parent;
        }
        true
    }

    pub fn spawn(&mut self, data: EntityData) -> EntityId {
        self.len += 1;
        let revision = tick();
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.data = Some(data);
            slot.revision = revision;
            return EntityId::new(index, slot.generation);
        }

        let index = u32::try_from(self.slots.len()).expect("entity capacity exceeded u32::MAX");
        self.slots.push(EntitySlot {
            generation: 0,
            revision,
            data: Some(data),
        });
        EntityId::new(index, 0)
    }

    /// When this entity last changed, or `None` if the world does not hold it.
    ///
    /// What lets anything deriving state from an entity -- a renderer resolving
    /// a tile volume into faces, a navigation surface, a collider -- keep the
    /// derived answer and rebuild it only when the entity it came from has
    /// moved on. The number is opaque and only comparable for equality: a
    /// different value means "ask again", not how much changed.
    ///
    /// Deliberately pessimistic. Taking a mutable borrow of an entity bumps it
    /// whether or not the borrower wrote anything, because a borrow that is
    /// handed out cannot be watched. Re-deriving something that did not need it
    /// costs time; missing a change that did shows the player the wrong world.
    #[must_use]
    pub fn revision(&self, entity: EntityId) -> Option<u64> {
        let slot = self.slot(entity)?;
        slot.data.is_some().then_some(slot.revision)
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
                revision: 0,
                data: None,
            });
        }
        if self.slots[index].data.is_some() {
            return Err(WorldError::SlotOccupied(entity));
        }
        let revision = tick();
        self.slots[index].generation = entity.generation();
        self.slots[index].revision = revision;
        self.slots[index].data = Some(data);
        self.free.retain(|free| *free != entity.index());
        self.len += 1;
        Ok(())
    }

    fn require(&self, entity: EntityId) -> Result<(), WorldError> {
        self.contains(entity)
            .then_some(())
            .ok_or(WorldError::InvalidEntity(entity))
    }

    fn slot(&self, entity: EntityId) -> Option<&EntitySlot> {
        let slot = self.slots.get(entity.index() as usize)?;
        (slot.generation == entity.generation()).then_some(slot)
    }

    fn slot_mut(&mut self, entity: EntityId) -> Option<&mut EntitySlot> {
        let revision = tick();
        let slot = self.slots.get_mut(entity.index() as usize)?;
        if slot.generation != entity.generation() {
            return None;
        }
        slot.revision = revision;
        Some(slot)
    }
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
    #[error("another entity already has the stable ID '{}'", .0.as_str())]
    DuplicateSourceId(SceneEntityId),
    #[error(
        "entity {0:?} declares its transform's Z locked; unlock it before \
         moving it off that layer"
    )]
    TransformZLocked(EntityId),
    #[error(transparent)]
    InvalidScene(#[from] SceneError),
    #[error(transparent)]
    InvalidPrefab(PrefabError),
}
