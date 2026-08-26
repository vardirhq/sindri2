//! Commands collected before anything is applied.

use super::{Transaction, WorldCommand};

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
        Transaction::new(label, self.commands)
    }
}
