use serde_json::Value;
use thiserror::Error;

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
    fn apply(self, world: &mut World) -> Result<Self, WorldError> {
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

/// A labelled group of commands applied and reversed as one unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    label: String,
    commands: Vec<WorldCommand>,
    merge_key: Option<String>,
    /// The history revision of the state this entry moves back to.
    ///
    /// Written by [`CommandHistory`] as a transaction enters a stack, and
    /// meaningless until then. It rides along with the entry so that undo and
    /// redo restore the revision the world actually had, rather than a stack
    /// depth that says nothing about which edits are in it.
    previous_revision: u64,
}

impl Transaction {
    /// The human-readable name shown by undo and redo affordances.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Marks this transaction as continuing the run identified by `key`.
    ///
    /// A pointer drag produces one transaction per frame. Merging collapses a
    /// run of them into a single undo step that returns to the value from
    /// before the drag began, rather than stepping back through every
    /// intermediate position.
    #[must_use]
    pub fn merging(self, key: impl Into<String>) -> Self {
        Self {
            merge_key: Some(key.into()),
            ..self
        }
    }

    /// The revision the world returns to when this entry is applied.
    const fn previous_revision(&self) -> u64 {
        self.previous_revision
    }

    /// The merge run this transaction belongs to, if any.
    pub fn merge_key(&self) -> Option<&str> {
        self.merge_key.as_deref()
    }

    pub fn commands(&self) -> &[WorldCommand] {
        &self.commands
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Applies every command in order, returning the transaction that reverses it.
    ///
    /// Application is all-or-nothing: a rejected command rolls back the ones
    /// already applied, so a failed edit never leaves a half-written world.
    fn apply(self, world: &mut World) -> Result<Self, CommandError> {
        let mut inverses = Vec::with_capacity(self.commands.len());
        for (index, command) in self.commands.into_iter().enumerate() {
            match command.apply(world) {
                Ok(inverse) => inverses.push(inverse),
                Err(source) => {
                    for applied in inverses.into_iter().rev() {
                        applied
                            .apply(world)
                            .expect("reversing an applied command cannot fail");
                    }
                    return Err(CommandError::Rejected {
                        label: self.label,
                        index,
                        source,
                    });
                }
            }
        }
        // Reversing a group means undoing its commands newest to oldest.
        inverses.reverse();
        Ok(Self {
            label: self.label,
            commands: inverses,
            merge_key: self.merge_key,
            previous_revision: self.previous_revision,
        })
    }
}

/// Queues commands so a tool can describe an edit before anything is written.
#[derive(Clone, Debug, Default)]
pub struct CommandBuffer {
    commands: Vec<WorldCommand>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: WorldCommand) -> &mut Self {
        self.commands.push(command);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn commands(&self) -> &[WorldCommand] {
        &self.commands
    }

    /// Groups the queued commands under `label` so they undo as one step.
    #[must_use]
    pub fn into_transaction(self, label: impl Into<String>) -> Transaction {
        Transaction {
            label: label.into(),
            commands: self.commands,
            merge_key: None,
            // Replaced by the history when this enters a stack.
            previous_revision: 0,
        }
    }
}

/// Bounded undo and redo stacks over applied transactions.
#[derive(Clone, Debug)]
pub struct CommandHistory {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    limit: usize,
    /// Which state the world is in, as far as this history knows.
    revision: u64,
    /// The next unused revision. Only ever moves forward, so a state that has
    /// been left behind can never be mistaken for one returned to.
    next_revision: u64,
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::with_limit(Self::DEFAULT_LIMIT)
    }
}

impl CommandHistory {
    pub const DEFAULT_LIMIT: usize = 128;

    /// Creates a history retaining at most `limit` undo steps.
    ///
    /// A limit of zero applies edits without recording them, which is how a
    /// host runs commands that should not be user-reversible.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
            revision: 0,
            next_revision: 1,
        }
    }

    /// Identifies the state the world is in, as far as history is concerned.
    ///
    /// Two equal revisions mean the same edits have been applied, so a caller
    /// that remembers the revision it last saved at can ask whether the world
    /// has come back to it — including by undoing, which restores the revision
    /// along with the state. A fresh edit never reuses one, so a different
    /// route to a similar-looking world does not read as unchanged.
    ///
    /// This exists because the obvious alternatives are wrong. A flag set on
    /// every edit can never return to clean. A stack depth does not move during
    /// a merged drag, though the world does, and it repeats itself once the
    /// bounded stack starts dropping its oldest entry.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Moves to a state nothing has been at before.
    fn advance_revision(&mut self) {
        self.revision = self.next_revision;
        self.next_revision += 1;
    }

    /// Applies `transaction`, recording it as one undo step.
    ///
    /// Empty transactions are not recorded, so a drag that ends where it began
    /// does not leave an undo step that appears to do nothing.
    pub fn apply(
        &mut self,
        transaction: Transaction,
        world: &mut World,
    ) -> Result<(), CommandError> {
        if transaction.is_empty() {
            return Ok(());
        }
        let continues_run = transaction.merge_key.as_deref().is_some_and(|key| {
            self.undo
                .last()
                .is_some_and(|previous| previous.merge_key.as_deref() == Some(key))
        });
        let leaving = self.revision;
        let mut inverse = transaction.apply(world)?;
        self.redo.clear();
        // Every applied edit moves to a state nothing has been at before,
        // including one that merges into the run above it: the drag changed the
        // world even though the stack did not grow.
        self.advance_revision();
        if continues_run {
            // The run's first inverse already restores the value from before
            // the run started, so later ones are dropped rather than stacked.
            return Ok(());
        }
        inverse.previous_revision = leaving;
        self.push_undo(inverse);
        Ok(())
    }

    /// Ends the current merge run so the next edit starts a new undo step.
    ///
    /// Hosts call this when a continuous interaction finishes — a pointer
    /// release, or a text field losing focus.
    pub fn break_merge_run(&mut self) {
        if let Some(transaction) = self.undo.last_mut() {
            transaction.merge_key = None;
        }
    }

    /// Reverses the most recent transaction, returning its label.
    pub fn undo(&mut self, world: &mut World) -> Result<Option<String>, CommandError> {
        let Some(transaction) = self.undo.pop() else {
            return Ok(None);
        };
        let label = transaction.label().to_owned();
        let returning_to = transaction.previous_revision();
        let mut inverse = transaction.apply(world)?;
        // The entry going onto the other stack carries the state being left,
        // so redoing arrives back at exactly the revision undo departed from.
        inverse.previous_revision = self.revision;
        self.revision = returning_to;
        self.redo.push(inverse);
        Ok(Some(label))
    }

    /// Re-applies the most recently undone transaction, returning its label.
    pub fn redo(&mut self, world: &mut World) -> Result<Option<String>, CommandError> {
        let Some(transaction) = self.redo.pop() else {
            return Ok(None);
        };
        let label = transaction.label().to_owned();
        let returning_to = transaction.previous_revision();
        let mut inverse = transaction.apply(world)?;
        inverse.previous_revision = self.revision;
        self.revision = returning_to;
        self.push_undo(inverse);
        Ok(Some(label))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(Transaction::label)
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(Transaction::label)
    }

    /// Discards all recorded history.
    ///
    /// Rebuilding a world invalidates every recorded [`EntityId`], so a host
    /// must call this whenever it reloads or resets a scene.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        // A cleared history belongs to a world that was rebuilt, which is a
        // state nothing has been at before rather than the one this history
        // happened to be showing.
        self.advance_revision();
    }

    fn push_undo(&mut self, transaction: Transaction) {
        if self.limit == 0 {
            return;
        }
        self.undo.push(transaction);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CommandError {
    #[error("command {index} of '{label}' was rejected and the transaction was rolled back")]
    Rejected {
        label: String,
        index: usize,
        #[source]
        source: WorldError,
    },
}

#[cfg(test)]
mod tests {

    /// The claim the whole design rests on: undoing a despawn hands back the
    /// *same* handle, so the selection and every earlier command naming it are
    /// still pointing at the entity they named.
    ///
    /// A generation-checked handle normally changes when a slot is reused, and
    /// a respawn that handed back a new one would quietly invalidate the rest
    /// of the history.
    #[test]
    fn undoing_a_despawn_gives_back_the_same_handle() {
        let mut world = World::default();
        let entity = world.spawn(EntityData {
            name: Some("Doomed".to_owned()),
            ..EntityData::default()
        });

        let mut history = CommandHistory::default();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Despawn { entity });
        history
            .apply(buffer.into_transaction("Delete"), &mut world)
            .unwrap();
        assert!(world.get(entity).is_none());

        history.undo(&mut world).unwrap();
        assert_eq!(
            world.get(entity).and_then(|data| data.name.as_deref()),
            Some("Doomed"),
            "the same handle finds the same entity"
        );

        // And redo takes it away again, so the pair is total rather than
        // one-way.
        history.redo(&mut world).unwrap();
        assert!(world.get(entity).is_none());
    }

    /// The slot being free when undo reaches a despawn is not luck: the
    /// history undoes in order, so everything that could have taken the slot
    /// has already been undone. This is that argument, executed.
    #[test]
    fn a_slot_reused_after_a_despawn_is_free_again_by_the_time_undo_needs_it() {
        let mut world = World::default();
        let first = world.spawn(EntityData {
            name: Some("First".to_owned()),
            ..EntityData::default()
        });

        let mut history = CommandHistory::default();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Despawn { entity: first });
        history
            .apply(buffer.into_transaction("Delete"), &mut world)
            .unwrap();

        // A second entity takes the freed slot, with a new generation.
        let second = world.next_handle();
        assert_eq!(second.index(), first.index(), "the same slot");
        assert_ne!(second, first, "but not the same handle");
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Spawn {
            entity: second,
            data: Box::new(EntityData {
                name: Some("Second".to_owned()),
                ..EntityData::default()
            }),
        });
        history
            .apply(buffer.into_transaction("Create"), &mut world)
            .unwrap();

        history.undo(&mut world).unwrap();
        history.undo(&mut world).unwrap();

        assert_eq!(
            world.get(first).and_then(|data| data.name.as_deref()),
            Some("First"),
            "the first entity is back at its own handle"
        );
        assert!(
            world.get(second).is_none(),
            "and the one that had borrowed its slot is gone"
        );
    }

    /// Deleting a parent takes its children, and undoing brings the whole
    /// subtree back — with its links and its place among its siblings.
    #[test]
    fn undoing_a_despawn_restores_the_whole_subtree_in_place() {
        let mut world = World::default();
        let root = world.spawn(EntityData::default());
        let first = world.spawn(EntityData::default());
        let doomed = world.spawn(EntityData {
            name: Some("Doomed".to_owned()),
            ..EntityData::default()
        });
        let last = world.spawn(EntityData::default());
        let child = world.spawn(EntityData {
            name: Some("Child".to_owned()),
            ..EntityData::default()
        });
        for entity in [first, doomed, last] {
            world.set_parent(entity, Some(root)).unwrap();
        }
        world.set_parent(child, Some(doomed)).unwrap();
        let before = world.get(root).unwrap().children.clone();

        let mut history = CommandHistory::default();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Despawn { entity: doomed });
        history
            .apply(buffer.into_transaction("Delete"), &mut world)
            .unwrap();
        assert!(world.get(child).is_none(), "a child goes with its parent");
        assert_eq!(world.get(root).unwrap().children, vec![first, last]);

        history.undo(&mut world).unwrap();
        assert_eq!(
            world.get(root).unwrap().children,
            before,
            "and comes back between the siblings it was between, not at the end"
        );
        assert_eq!(
            world.get(child).and_then(|data| data.name.as_deref()),
            Some("Child")
        );
        assert_eq!(world.get(child).unwrap().parent, Some(doomed));
    }

    /// Spawning at an occupied handle is refused rather than overwriting what
    /// is there. Nothing should reach it, and that is exactly why it is checked.
    #[test]
    fn spawning_onto_a_live_entity_is_refused() {
        let mut world = World::default();
        let entity = world.spawn(EntityData::default());
        assert!(matches!(
            world.spawn_at(entity, EntityData::default()),
            Err(WorldError::SlotOccupied(_))
        ));
    }
    use serde_json::json;

    use super::*;
    use crate::EntityData;

    fn world_with_two_entities() -> (World, EntityId, EntityId) {
        let mut world = World::default();
        let parent = world.spawn(EntityData {
            name: Some("Parent".into()),
            ..EntityData::default()
        });
        let child = world.spawn(EntityData::default());
        (world, parent, child)
    }

    /// The layer an entity is on, as bits, because every assertion about it
    /// here is that it is exactly where it was or exactly where it was put.
    fn layer_bits(world: &World, entity: EntityId) -> u32 {
        position(world, entity)[2].to_bits()
    }

    fn position(world: &World, entity: EntityId) -> [f32; 3] {
        world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .expect("entity has a 3D transform")
            .position
    }

    fn edit(label: impl Into<String>, commands: Vec<WorldCommand>) -> Transaction {
        let mut buffer = CommandBuffer::new();
        for command in commands {
            buffer.push(command);
        }
        buffer.into_transaction(label)
    }

    #[test]
    fn undo_and_redo_restore_each_recorded_value() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();

        history
            .apply(
                edit(
                    "Rename",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some("Renamed".into()),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Renamed"));

        assert_eq!(history.undo(&mut world).unwrap().as_deref(), Some("Rename"));
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Parent"));

        assert_eq!(history.redo(&mut world).unwrap().as_deref(), Some("Rename"));
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Renamed"));
    }

    /// A declared Z lock is a check the command layer makes, which is what
    /// makes declaring it worth anything: every tool writes through here.
    #[test]
    fn a_locked_transform_refuses_to_be_moved_off_its_layer() {
        let (mut world, entity, _) = world_with_two_entities();
        let background = Transform3D {
            position: [0.0, 0.0, -50.0],
            z_locked: true,
            ..Transform3D::default()
        };
        world.get_mut(entity).unwrap().transform_3d = Some(background);

        let mut flattened = background;
        flattened.position[2] = 0.0;
        let mut history = CommandHistory::default();
        let error = history
            .apply(
                edit(
                    "Flatten",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(flattened),
                    }],
                ),
                &mut world,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            CommandError::Rejected {
                source: WorldError::TransformZLocked(named),
                ..
            } if named == entity
        ));
        assert_eq!(
            layer_bits(&world, entity),
            (-50.0_f32).to_bits(),
            "the refused command must not have moved anything"
        );
        assert!(
            history.undo(&mut world).unwrap().is_none(),
            "a refused command must not enter the history"
        );
    }

    /// Locked is about the layer alone: the same transform still moves around
    /// its plane, and unlocking is what grants permission to leave it.
    #[test]
    fn a_locked_transform_still_moves_within_its_layer_and_unlocks() {
        let (mut world, entity, _) = world_with_two_entities();
        let background = Transform3D {
            position: [0.0, 0.0, -50.0],
            z_locked: true,
            ..Transform3D::default()
        };
        world.get_mut(entity).unwrap().transform_3d = Some(background);
        let mut history = CommandHistory::default();

        let mut slid = background;
        slid.translate_2d([3.0, 1.0]);
        history
            .apply(
                edit(
                    "Slide",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(slid),
                    }],
                ),
                &mut world,
            )
            .expect("moving in the plane is what the lock leaves alone");

        let unlocked = Transform3D {
            z_locked: false,
            ..slid
        };
        let mut moved = unlocked;
        moved.position[2] = -10.0;
        history
            .apply(
                edit(
                    "Unlock",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(unlocked),
                    }],
                ),
                &mut world,
            )
            .expect("unlocking changes no layer");
        history
            .apply(
                edit(
                    "Move",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(moved),
                    }],
                ),
                &mut world,
            )
            .expect("an unlocked transform moves");
        assert_eq!(layer_bits(&world, entity), (-10.0_f32).to_bits());
    }

    /// A transaction is all or nothing, and a refusal is what tests that: the
    /// rename beside the rejected move must not survive it.
    #[test]
    fn a_refused_move_rolls_back_the_rest_of_its_transaction() {
        let (mut world, entity, other) = world_with_two_entities();
        world.get_mut(entity).unwrap().transform_3d = Some(Transform3D {
            position: [0.0, 0.0, -50.0],
            z_locked: true,
            ..Transform3D::default()
        });
        let mut history = CommandHistory::default();

        let error = history
            .apply(
                edit(
                    "Move selection",
                    vec![
                        WorldCommand::SetName {
                            entity: other,
                            name: Some("Renamed".into()),
                        },
                        WorldCommand::SetTransform3D {
                            entity,
                            transform: None,
                        },
                    ],
                ),
                &mut world,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            CommandError::Rejected {
                source: WorldError::TransformZLocked(_),
                index: 1,
                ..
            }
        ));
        assert_eq!(world.get(other).unwrap().name, None);
        assert_eq!(layer_bits(&world, entity), (-50.0_f32).to_bits());
    }

    /// What the revision is for: a caller can ask whether the world has come
    /// back to a state it remembers, without comparing whole documents.
    #[test]
    fn undoing_back_to_a_state_returns_its_revision() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let saved = history.revision();

        let moved = Transform3D {
            position: [1.0, 2.0, 3.0],
            ..Transform3D::default()
        };
        history
            .apply(
                edit(
                    "Move",
                    vec![WorldCommand::SetTransform3D {
                        entity,
                        transform: Some(moved),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert_ne!(history.revision(), saved, "an edit is a different state");

        history.undo(&mut world).unwrap();
        assert_eq!(
            history.revision(),
            saved,
            "undoing back to where it was saved must read as saved"
        );

        history.redo(&mut world).unwrap();
        assert_ne!(history.revision(), saved);
        history.undo(&mut world).unwrap();
        assert_eq!(history.revision(), saved, "and again, however many times");
    }

    /// A drag merges into one undo step, so the stack does not grow while the
    /// world keeps changing. The revision has to move anyway, or a drag that
    /// began at the saved state would read as though nothing had happened.
    #[test]
    fn a_merged_run_moves_the_revision_every_time_it_changes_the_world() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let saved = history.revision();

        let mut seen = vec![saved];
        for step in [1.0_f32, 2.0, 3.0, 4.0] {
            let mut buffer = CommandBuffer::new();
            buffer.push(WorldCommand::SetTransform3D {
                entity,
                transform: Some(Transform3D {
                    position: [step, 0.0, 0.0],
                    ..Transform3D::default()
                }),
            });
            history
                .apply(buffer.into_transaction("Drag").merging("drag"), &mut world)
                .unwrap();
            assert!(
                !seen.contains(&history.revision()),
                "every step of a drag is a state of its own"
            );
            seen.push(history.revision());
        }

        history.undo(&mut world).unwrap();
        assert_eq!(
            history.revision(),
            saved,
            "and undoing the run returns to before it started"
        );
    }

    /// A different edit made after undoing cannot land on the abandoned state,
    /// however much the stacks happen to line up.
    #[test]
    fn an_edit_after_undoing_never_reuses_the_state_it_replaced() {
        let (mut world, entity, other) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let rename = |name: &str| {
            edit(
                "Rename",
                vec![WorldCommand::SetName {
                    entity,
                    name: Some(name.to_owned()),
                }],
            )
        };

        history.apply(rename("First"), &mut world).unwrap();
        let abandoned = history.revision();
        history.undo(&mut world).unwrap();
        history
            .apply(
                edit(
                    "Rename other",
                    vec![WorldCommand::SetName {
                        entity: other,
                        name: Some("Other".to_owned()),
                    }],
                ),
                &mut world,
            )
            .unwrap();

        assert_ne!(history.revision(), abandoned);
        assert!(history.undo(&mut world).unwrap().is_some());
        assert_ne!(
            history.revision(),
            abandoned,
            "the stacks match the abandoned state's shape, and the world does not"
        );
    }

    /// Rebuilding a world is not a return to anything.
    #[test]
    fn clearing_the_history_moves_to_a_state_of_its_own() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let empty = history.revision();
        history
            .apply(
                edit(
                    "Rename",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some("Renamed".to_owned()),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        history.clear();
        assert_ne!(history.revision(), empty);
    }

    #[test]
    fn a_transaction_undoes_as_one_step() {
        let (mut world, parent, child) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let moved = Transform3D {
            position: [1.0, 2.0, 3.0],
            ..Transform3D::default()
        };

        history
            .apply(
                edit(
                    "Move selection",
                    vec![
                        WorldCommand::SetTransform3D {
                            entity: parent,
                            transform: Some(moved),
                        },
                        WorldCommand::SetTransform3D {
                            entity: child,
                            transform: Some(moved),
                        },
                    ],
                ),
                &mut world,
            )
            .unwrap();

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(parent).unwrap().transform_3d, None);
        assert_eq!(world.get(child).unwrap().transform_3d, None);
        assert!(!history.can_undo());
    }

    #[test]
    fn a_rejected_command_rolls_the_whole_transaction_back() {
        let (mut world, parent, child) = world_with_two_entities();
        let stale = world.spawn(EntityData::default());
        world.despawn_recursive(stale).unwrap();
        let mut history = CommandHistory::default();

        let error = history
            .apply(
                edit(
                    "Partly invalid",
                    vec![
                        WorldCommand::SetName {
                            entity: parent,
                            name: Some("Applied first".into()),
                        },
                        WorldCommand::SetName {
                            entity: stale,
                            name: Some("Never applied".into()),
                        },
                        WorldCommand::SetName {
                            entity: child,
                            name: Some("Never reached".into()),
                        },
                    ],
                ),
                &mut world,
            )
            .unwrap_err();

        assert_eq!(
            error,
            CommandError::Rejected {
                label: "Partly invalid".to_owned(),
                index: 1,
                source: WorldError::InvalidEntity(stale),
            }
        );
        // The first command was applied, then reversed by the rollback.
        assert_eq!(world.get(parent).unwrap().name.as_deref(), Some("Parent"));
        assert_eq!(world.get(child).unwrap().name, None);
        assert!(!history.can_undo());
    }

    #[test]
    fn a_rejected_reparent_leaves_the_hierarchy_untouched() {
        let (mut world, parent, child) = world_with_two_entities();
        let mut history = CommandHistory::default();
        history
            .apply(
                edit(
                    "Reparent",
                    vec![WorldCommand::SetParent {
                        entity: child,
                        parent: Some(parent),
                    }],
                ),
                &mut world,
            )
            .unwrap();

        // Parenting the parent under its own child would close a cycle.
        let error = history
            .apply(
                edit(
                    "Close a cycle",
                    vec![
                        WorldCommand::SetName {
                            entity: child,
                            name: Some("Doomed".into()),
                        },
                        WorldCommand::SetParent {
                            entity: parent,
                            parent: Some(child),
                        },
                    ],
                ),
                &mut world,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            CommandError::Rejected {
                index: 1,
                source: WorldError::HierarchyCycle,
                ..
            }
        ));
        assert_eq!(world.get(child).unwrap().name, None);
        assert_eq!(world.get(parent).unwrap().parent, None);
        assert_eq!(world.get(child).unwrap().parent, Some(parent));
    }

    #[test]
    fn components_round_trip_through_set_and_remove() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();

        history
            .apply(
                edit(
                    "Add health",
                    vec![WorldCommand::SetComponent {
                        entity,
                        type_name: "game.health".into(),
                        payload: json!({ "current": 3 }),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        history
            .apply(
                edit(
                    "Remove health",
                    vec![WorldCommand::RemoveComponent {
                        entity,
                        type_name: "game.health".into(),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert!(world.get(entity).unwrap().components.is_empty());

        history.undo(&mut world).unwrap();
        assert_eq!(
            world.get(entity).unwrap().components["game.health"],
            json!({ "current": 3 })
        );
        history.undo(&mut world).unwrap();
        assert!(world.get(entity).unwrap().components.is_empty());
    }

    #[test]
    fn reparenting_is_reversible() {
        let (mut world, parent, child) = world_with_two_entities();
        let mut history = CommandHistory::default();

        history
            .apply(
                edit(
                    "Reparent",
                    vec![WorldCommand::SetParent {
                        entity: child,
                        parent: Some(parent),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert_eq!(world.get(child).unwrap().parent, Some(parent));
        assert_eq!(world.get(parent).unwrap().children, vec![child]);

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(child).unwrap().parent, None);
        assert!(world.get(parent).unwrap().children.is_empty());
    }

    #[test]
    fn applying_a_new_edit_discards_the_redo_stack() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();

        history
            .apply(
                edit(
                    "First",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some("First".into()),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        history.undo(&mut world).unwrap();
        assert!(history.can_redo());

        history
            .apply(
                edit(
                    "Second",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some("Second".into()),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert!(!history.can_redo());
        assert_eq!(history.undo_label(), Some("Second"));
    }

    #[test]
    fn empty_transactions_are_not_recorded() {
        let (mut world, _, _) = world_with_two_entities();
        let mut history = CommandHistory::default();
        history
            .apply(edit("Nothing", Vec::new()), &mut world)
            .unwrap();
        assert!(!history.can_undo());
        assert_eq!(history.undo(&mut world).unwrap(), None);
        assert_eq!(history.redo(&mut world).unwrap(), None);
    }

    #[test]
    fn history_is_bounded_by_its_limit() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::with_limit(2);
        for index in 0..4 {
            history
                .apply(
                    edit(
                        format!("Edit {index}"),
                        vec![WorldCommand::SetName {
                            entity,
                            name: Some(format!("Name {index}")),
                        }],
                    ),
                    &mut world,
                )
                .unwrap();
        }
        assert_eq!(history.undo_label(), Some("Edit 3"));
        history.undo(&mut world).unwrap();
        history.undo(&mut world).unwrap();
        assert!(!history.can_undo());
    }

    #[test]
    fn a_merge_run_collapses_into_one_undo_step() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();
        let start = Transform3D::default();
        world.get_mut(entity).unwrap().transform_3d = Some(start);

        // Stand in for a drag: one transaction per frame, all merging.
        for step in [1.0, 2.0, 3.0, 4.0, 5.0] {
            let moved = Transform3D {
                position: [step, 0.0, 0.0],
                ..start
            };
            history
                .apply(
                    edit(
                        "Move",
                        vec![WorldCommand::SetTransform3D {
                            entity,
                            transform: Some(moved),
                        }],
                    )
                    .merging("drag:torso"),
                    &mut world,
                )
                .unwrap();
        }
        assert!(position(&world, entity)[0] > 4.9);

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(entity).unwrap().transform_3d, Some(start));
        assert!(!history.can_undo(), "the run should be a single step");

        history.redo(&mut world).unwrap();
        assert!(position(&world, entity)[0] > 4.9);
    }

    #[test]
    fn breaking_a_run_starts_a_new_undo_step() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::default();

        let drag = |history: &mut CommandHistory, world: &mut World, name: &str| {
            history
                .apply(
                    edit(
                        "Rename",
                        vec![WorldCommand::SetName {
                            entity,
                            name: Some(name.to_owned()),
                        }],
                    )
                    .merging("rename"),
                    world,
                )
                .unwrap();
        };

        drag(&mut history, &mut world, "First");
        drag(&mut history, &mut world, "Second");
        history.break_merge_run();
        drag(&mut history, &mut world, "Third");

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Second"));
        history.undo(&mut world).unwrap();
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Parent"));
        assert!(!history.can_undo());
    }

    #[test]
    fn different_merge_keys_do_not_collapse_together() {
        let (mut world, parent, child) = world_with_two_entities();
        let mut history = CommandHistory::default();

        for (entity, key) in [(parent, "drag:parent"), (child, "drag:child")] {
            history
                .apply(
                    edit(
                        "Move",
                        vec![WorldCommand::SetTransform3D {
                            entity,
                            transform: Some(Transform3D::default()),
                        }],
                    )
                    .merging(key),
                    &mut world,
                )
                .unwrap();
        }

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(child).unwrap().transform_3d, None);
        assert!(history.can_undo());
        history.undo(&mut world).unwrap();
        assert_eq!(world.get(parent).unwrap().transform_3d, None);
    }

    #[test]
    fn a_zero_limit_applies_without_recording() {
        let (mut world, entity, _) = world_with_two_entities();
        let mut history = CommandHistory::with_limit(0);
        history
            .apply(
                edit(
                    "Unrecorded",
                    vec![WorldCommand::SetName {
                        entity,
                        name: Some("Applied".into()),
                    }],
                ),
                &mut world,
            )
            .unwrap();
        assert_eq!(world.get(entity).unwrap().name.as_deref(), Some("Applied"));
        assert!(!history.can_undo());
    }
}
