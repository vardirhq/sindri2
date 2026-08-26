//! One undo step: the commands that went in, and how to put them back.

use crate::World;

use super::{CommandError, WorldCommand};

/// A labelled group of commands applied and reversed as one unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    label: String,
    commands: Vec<WorldCommand>,
    pub(super) merge_key: Option<String>,
    /// The history revision of the state this entry moves back to.
    ///
    /// Written by [`CommandHistory`] as a transaction enters a stack, and
    /// meaningless until then. It rides along with the entry so that undo and
    /// redo restore the revision the world actually had, rather than a stack
    /// depth that says nothing about which edits are in it.
    pub(super) previous_revision: u64,
}

impl Transaction {
    /// A transaction of `commands`, not yet in any stack.
    pub(super) fn new(label: impl Into<String>, commands: Vec<WorldCommand>) -> Self {
        Self {
            label: label.into(),
            commands,
            merge_key: None,
            // Replaced by the history when this enters a stack.
            previous_revision: 0,
        }
    }

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
    pub(super) const fn previous_revision(&self) -> u64 {
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
    pub(super) fn apply(self, world: &mut World) -> Result<Self, CommandError> {
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
