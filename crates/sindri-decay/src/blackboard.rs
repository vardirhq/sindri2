//! The numbers scripts leave for each other.
//!
//! Runtime state, held beside the world exactly as script instances are, and
//! for the same reason: what a game has counted halfway through a run is not
//! something a scene should be saved with.

use std::collections::BTreeMap;

/// A name-to-number board every script in a world can read and write.
#[derive(Clone, Debug, Default)]
pub struct Blackboard {
    notes: BTreeMap<String, f64>,
}

impl Blackboard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number left under a name, or `fallback` when nothing has been.
    #[must_use]
    pub fn get(&self, name: &str, fallback: f64) -> f64 {
        self.notes.get(name).copied().unwrap_or(fallback)
    }

    pub fn set(&mut self, name: impl Into<String>, value: f64) {
        self.notes.insert(name.into(), value);
    }

    /// Whether anything has been left under a name.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.notes.contains_key(name)
    }

    /// Everything on the board, for a host that wants to show it.
    pub fn notes(&self) -> impl Iterator<Item = (&str, f64)> {
        self.notes
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
    }

    pub fn clear(&mut self) {
        self.notes.clear();
    }
}
