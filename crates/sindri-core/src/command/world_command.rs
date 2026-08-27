//! The commands themselves, and how one is applied to a world.

use serde_json::Value;

use crate::{EntityData, EntityId, SceneEntityId, Transform3D, World, WorldError};

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
    /// Changes the stable identity a scene stores this entity under.
    ///
    /// Distinct from [`Self::SetName`], which is a label. A stable ID is what
    /// the file keys an entity by, what a parent link names, what sibling order
    /// is derived from, and what a component like `sindri.grid.occupant`
    /// points at — so two entities sharing one is not a cosmetic collision, and
    /// this refuses rather than allowing it.
    SetSourceId {
        entity: EntityId,
        source_id: Option<SceneEntityId>,
    },
    /// Renames the scene itself.
    ///
    /// The one command here that is not about an entity. It goes through the
    /// command layer anyway, for the reason every other edit does: an editor
    /// tracks whether a document is unsaved by watching the history, so a
    /// change made outside it is a change the editor does not know it has and
    /// would let someone close the window on.
    SetSceneName {
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
    /// Switches an entity, and everything under it, off or back on.
    ///
    /// Not a delete you undo: the entity stays in the world and in the file,
    /// which is the whole difference. What changes is that nothing it carries
    /// is drawn, stepped, scripted or picked — see [`World::is_active`], which
    /// is the question those all ask, and which walks ancestors so that
    /// switching off a HUD switches off its pips too.
    SetDisabled {
        entity: EntityId,
        disabled: bool,
    },
    /// Writes one entry in an entity's editor-only map.
    ///
    /// That map is state a runtime carries but never interprets — where an
    /// entity sits among its siblings, say, which is a fact about a tree in a
    /// panel rather than about a scene being played. It goes through the
    /// command layer anyway, for the reason [`Self::SetSceneName`] does: it is
    /// saved with the document, so it has to be undoable and it has to make the
    /// document unsaved.
    SetEditorEntry {
        entity: EntityId,
        key: String,
        /// `None` removes the entry, which is how a value goes back to not
        /// being there rather than to being `null`.
        value: Option<Value>,
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
    /// The entity this command writes to, if it is about one.
    ///
    /// `None` for a command that edits the scene rather than something in it.
    pub const fn entity(&self) -> Option<EntityId> {
        match self {
            Self::SetSceneName { .. } => None,
            Self::SetName { entity, .. }
            | Self::SetSourceId { entity, .. }
            | Self::SetTransform3D { entity, .. }
            | Self::SetParent { entity, .. }
            | Self::SetComponent { entity, .. }
            | Self::RemoveComponent { entity, .. }
            | Self::SetDisabled { entity, .. }
            | Self::SetEditorEntry { entity, .. }
            | Self::Spawn { entity, .. }
            | Self::Despawn { entity }
            // The root of the subtree, which is the entity a label would name.
            | Self::Restore { root: entity, .. } => Some(*entity),
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
            Self::SetSourceId { entity, source_id } => set_source_id(world, entity, source_id),
            Self::SetSceneName { name } => Ok(set_scene_name(world, name)),
            Self::SetTransform3D { entity, transform } => {
                set_transform_3d(world, entity, transform)
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
            Self::SetDisabled { entity, disabled } => {
                let data = world
                    .get_mut(entity)
                    .ok_or(WorldError::InvalidEntity(entity))?;
                let previous = std::mem::replace(&mut data.disabled, disabled);
                Ok(Self::SetDisabled {
                    entity,
                    disabled: previous,
                })
            }
            Self::SetEditorEntry { entity, key, value } => {
                set_editor_entry(world, entity, key, value)
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

/// Gives an entity a new stable identity, refusing one another already holds.
///
/// Checked before the write, like every other refusal in `apply`: a rejected
/// command must leave the world exactly as it found it, which is what makes a
/// transaction all-or-nothing.
/// Writes an entity's transform, unless it has declared that it stays put.
fn set_transform_3d(
    world: &mut World,
    entity: EntityId,
    transform: Option<Transform3D>,
) -> Result<WorldCommand, WorldError> {
    let data = world
        .get_mut(entity)
        .ok_or(WorldError::InvalidEntity(entity))?;
    // The one place a declared Z lock is respected, which is what makes it
    // worth declaring: every tool writes through here. Refusing before the
    // write is what keeps a transaction's all-or-nothing promise honest — a
    // rejected command has changed nothing to roll back.
    if data
        .transform_3d
        .is_some_and(|current| current.z_lock_rejects(transform))
    {
        return Err(WorldError::TransformZLocked(entity));
    }
    let previous = std::mem::replace(&mut data.transform_3d, transform);
    Ok(WorldCommand::SetTransform3D {
        entity,
        transform: previous,
    })
}

/// Writes or clears one entry in an entity's editor-only map.
///
/// Its own function for the reason the identity write has one: the `apply`
/// match is the shape of the command set, and a body long enough to scroll
/// hides that shape.
fn set_editor_entry(
    world: &mut World,
    entity: EntityId,
    key: String,
    value: Option<Value>,
) -> Result<WorldCommand, WorldError> {
    let data = world
        .get_mut(entity)
        .ok_or(WorldError::InvalidEntity(entity))?;
    let previous = match value {
        Some(value) => data.editor.insert(key.clone(), value),
        None => data.editor.remove(&key),
    };
    Ok(WorldCommand::SetEditorEntry {
        entity,
        key,
        value: previous,
    })
}

fn set_source_id(
    world: &mut World,
    entity: EntityId,
    source_id: Option<SceneEntityId>,
) -> Result<WorldCommand, WorldError> {
    if let Some(wanted) = &source_id
        && world
            .entities()
            .any(|(other, data)| other != entity && data.source_id.as_ref() == Some(wanted))
    {
        return Err(WorldError::DuplicateSourceId(wanted.clone()));
    }
    let data = world
        .get_mut(entity)
        .ok_or(WorldError::InvalidEntity(entity))?;
    let previous = std::mem::replace(&mut data.source_id, source_id);
    Ok(WorldCommand::SetSourceId {
        entity,
        source_id: previous,
    })
}

/// Renames the scene, returning the command that names it back.
fn set_scene_name(world: &mut World, name: Option<String>) -> WorldCommand {
    let mut metadata = world.metadata().clone();
    let previous = std::mem::replace(&mut metadata.name, name);
    world.set_metadata(metadata);
    WorldCommand::SetSceneName { name: previous }
}
