//! The prefabs a world's scripts can spawn, by asset ID.

use std::collections::BTreeMap;

use sindri_core::PrefabDocument;

/// The prefab documents `World.spawn` can name.
///
/// The same shape as [`crate::ScriptSources`] and for the same reason: this
/// crate does no I/O. A prefab arrives already parsed, so a document that
/// cannot be read is a failure the host reports once when it loads the asset
/// rather than a failure a script discovers on the frame it spawns.
///
/// A name nothing answers to is refused at the call, not read as "spawn
/// nothing" — a spawn that silently produced no entity is a bug report nobody
/// can reproduce.
#[derive(Clone, Debug, Default)]
pub struct PrefabSources {
    prefabs: BTreeMap<String, PrefabDocument>,
}

impl PrefabSources {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, prefab: PrefabDocument) {
        self.prefabs.insert(id.into(), prefab);
    }

    pub fn remove(&mut self, id: &str) -> Option<PrefabDocument> {
        self.prefabs.remove(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&PrefabDocument> {
        self.prefabs.get(id)
    }

    /// Every prefab ID the host has loaded, for a diagnostic that wants to say
    /// what *was* available when a script named something that was not.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.prefabs.keys().map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prefabs.is_empty()
    }
}
