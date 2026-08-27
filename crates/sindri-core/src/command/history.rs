//! The undo and redo stacks, and the revision they move.

use thiserror::Error;

use crate::{World, WorldError};

use super::Transaction;

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

    /// Every step that can be undone, oldest first.
    ///
    /// The stack itself stays private — a caller that could reach the
    /// transactions could apply one out of order — but their labels are what a
    /// history panel is. Without them "what will Ctrl+Z do" is answerable one
    /// step at a time from a menu entry, and "how far back can I go" not at all.
    pub fn undo_steps(&self) -> impl ExactSizeIterator<Item = &str> {
        self.undo.iter().map(Transaction::label)
    }

    /// Every step that can be redone, in the order redoing them would replay.
    pub fn redo_steps(&self) -> impl ExactSizeIterator<Item = &str> {
        self.redo.iter().rev().map(Transaction::label)
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
