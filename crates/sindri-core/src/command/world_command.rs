//! The commands themselves, and how one is applied to a world.

use serde_json::Value;

use crate::{EntityData, EntityId, Transform3D, World, WorldError};

/// A deferred mutation of a world.
///
/// Commands are the single write path for tools and hosts: an editor, the
/// eventual scripting boundary, and the web SDK all describe edits this way
/// rather than reaching into [`World`] directly. Every command knows how to
/// reverse itself, which is what makes undo a property of the core rather than
/// a bespoke editor feature.
///
/// Commands address entities by runtime [`EntityId`]. They deliberately do not
/// spawn or destroy entities: a respawned entity would receive a new handle and
/// silently invalidate every queued and recorded command referring to it. Scene
/// composition happens through [`World::from_scene`], and a rebuilt world
/// invalidates history — see [`CommandHistory::clear`].
#[derive(Clone, Debug, PartialEq)]
pub enum WorldCommand {
    SetName {
        entity: EntityId,
        name: Option<String>,
    },
    SetTransform3D {
        entity: EntityId,
        transform: Option<Transform3D>,
    },
    SetParent {
        entity: EntityId,
        parent: Option<EntityId>,
    },
    SetComponent {
        entity: EntityId,
        type_name: String,
        payload: Value,
    },
    RemoveComponent {
        entity: EntityId,
        type_name: String,
    },
    /// Creates an entity at an exact handle.
    ///
    /// The handle is chosen by the caller — from [`World::next_handle`] — rather
    /// than handed back afterwards, because a command has to be able to do the
    /// same thing twice. Redoing a spawn must produce the entity everything
    /// else in the history is already naming, and it can, for the reason
    /// [`World::spawn_at`] gives.
    Spawn {
        entity: EntityId,
        data: Box<EntityData>,
    },
    /// Removes an entity and everything under it.
    Despawn {
        entity: EntityId,
    },
    /// Puts a despawned subtree back exactly as it was.
    ///
    /// Produced as the inverse of a despawn rather than authored. Every entity
    /// returns to its own handle, so the selection and the rest of the history
    /// keep pointing at what they named.
    Restore {
        /// The subtree's root, named rather than derived so there is no
        /// "restore of nothing" case to invent an answer for.
        root: EntityId,
        /// Parents before their children.
        entities: Vec<(EntityId, Box<EntityData>)>,
        /// Where the subtree's root sat among its siblings, so undoing a
        /// delete puts it back in place rather than at the end of the list.
        sibling_index: Option<usize>,
    },
}

impl WorldCommand {
    /// The entity this command writes to.
    pub const fn entity(&self) -> EntityId {
        match self {
            Self::SetName { entity, .. }
            | Self::SetTransform3D { entity, .. }
            | Self::SetParent { entity, .. }
            | Self::SetComponent { entity, .. }
            | Self::RemoveComponent { entity, .. }
            | Self::Spawn { entity, .. }
            | Self::Despawn { entity }
            // The root of the subtree, which is the entity a label would name.
            | Self::Restore { root: entity, .. } => *entity,
        }
    }

    /// Applies this command and returns the command that reverses it.
    pub(super) fn apply(self, world: &mut World) -> Result<Self, WorldError> {
        match self {
            Self::SetName { entity, name } => {
                let data = world
                    .get_mut(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?;
                let previous = std::mem::replace(&mut data.name, name);
                Ok(Self::SetName {
                    entity,
                    name: previous,
                })
            }
            Self::SetTransform3D { entity, transform } => {
                let data = world
                    .get_mut(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?;
                // The one place a declared Z lock is respected, which is what
                // makes it worth declaring: every tool writes through here.
                // Refusing before the write is what keeps a transaction's
                // all-or-nothing promise honest — a rejected command has
                // changed nothing to roll back.
                if data
                    .transform_3d
                    .is_some_and(|current| current.z_lock_rejects(transform))
                {
                    return Err(WorldError::TransformZLocked(entity));
                }
                let previous = std::mem::replace(&mut data.transform_3d, transform);
                Ok(Self::SetTransform3D {
                    entity,
                    transform: previous,
                })
            }
            Self::SetParent { entity, parent } => {
                let previous = world
                    .get(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?
                    .parent;
                world.set_parent(entity, parent)?;
                Ok(Self::SetParent {
                    entity,
                    parent: previous,
                })
            }
            Self::SetComponent {
                entity,
                type_name,
                payload,
            } => {
                let data = world
                    .get_mut(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?;
                match data.components.insert(type_name.clone(), payload) {
                    Some(previous) => Ok(Self::SetComponent {
                        entity,
                        type_name,
                        payload: previous,
                    }),
                    None => Ok(Self::RemoveComponent { entity, type_name }),
                }
            }
            Self::RemoveComponent { entity, type_name } => {
                let data = world
                    .get_mut(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?;
                match data.components.remove(&type_name) {
                    Some(previous) => Ok(Self::SetComponent {
                        entity,
                        type_name,
                        payload: previous,
                    }),
                    // Removing an absent component is a no-op, and so is its reverse.
                    None => Ok(Self::RemoveComponent { entity, type_name }),
                }
            }
            Self::Spawn { entity, data } => {
                world.spawn_at(entity, *data)?;
                // Re-linked because the data carries the parent it belongs to,
                // and a parent's child list is the other half of that link.
                world.relink_child(entity, None)?;
                Ok(Self::Despawn { entity })
            }
            Self::Despawn { entity } => {
                // Captured before anything is removed: removing an entity edits
                // its parent's child list, so a capture taken part-way through
                // would record lists already missing their siblings.
                let sibling_index = world.sibling_index(entity);
                let captured = world.capture_subtree(entity)?;
                world.despawn_recursive(entity)?;
                Ok(Self::Restore {
                    root: entity,
                    entities: captured
                        .into_iter()
                        .map(|(entity, data)| (entity, Box::new(data)))
                        .collect(),
                    sibling_index,
                })
            }
            Self::Restore {
                root,
                entities,
                sibling_index,
            } => {
                for (entity, data) in entities {
                    world.spawn_at(entity, *data)?;
                }
                // Only the root's parent is outside the subtree and so lost a
                // child. Everything below kept its own list, which came back
                // with its data.
                world.relink_child(root, sibling_index)?;
                Ok(Self::Despawn { entity: root })
            }
        }
    }
}
