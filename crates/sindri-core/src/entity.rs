use serde::{Deserialize, Serialize};

/// A compact, generation-checked runtime entity handle.
///
/// Runtime handles are intentionally not persisted in scene files. Serialized
/// scenes use [`crate::SceneEntityId`] so allocation details can change safely.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EntityId {
    index: u32,
    generation: u32,
}

impl EntityId {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}
