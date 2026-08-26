//! The single write path into a world.
//!
//! Tools and hosts — the editor, the scripting boundary, the web SDK — describe
//! an edit as a [`WorldCommand`] rather than reaching into [`World`] directly.
//! Every command knows how to reverse itself, which is what makes undo a
//! property of the core rather than a bespoke editor feature.
//!
//! The pieces: a [`CommandBuffer`] collects commands, [`CommandHistory`]
//! applies a buffer and records what it did as a [`Transaction`], and that
//! transaction is what undo puts back.

mod buffer;
mod history;
mod transaction;
mod world_command;

#[cfg(test)]
mod tests;

pub use buffer::CommandBuffer;
pub use history::{CommandError, CommandHistory};
pub use transaction::Transaction;
pub use world_command::WorldCommand;
