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

    /// The handle packed into one number, for crossing a boundary that carries
    /// numbers and nothing else.
    ///
    /// Decay is that boundary: a script has to be able to name another entity
    /// to say anything about it, and the language holds an opaque number rather
    /// than knowing what a world is.
    ///
    /// **This is not a scene ID and must never be written to a file.** It is a
    /// runtime handle with a runtime handle's lifetime — see
    /// `docs/FEASIBILITY.md`. Packing it does not weaken the generation check
    /// either: bits that name a slot whose generation has moved on decode to an
    /// `EntityId` that no longer resolves, which is exactly what a stale handle
    /// should do.
    pub const fn to_bits(self) -> u64 {
        ((self.index as u64) << 32) | self.generation as u64
    }

    /// The inverse of [`Self::to_bits`].
    ///
    /// Every `u64` decodes, because every `u64` names some slot and some
    /// generation; whether it resolves to a live entity is [`World`]'s
    /// question, and it answers it the same way it does for any other handle.
    ///
    /// [`World`]: crate::World
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_bits(bits: u64) -> Self {
        Self::new((bits >> 32) as u32, bits as u32)
    }
}

#[cfg(test)]
mod bits_tests {
    use super::EntityId;

    /// Packing is only useful if it is exact, since a handle that comes back
    /// slightly different names a different entity or none at all.
    #[test]
    fn a_handle_survives_being_packed_into_a_number() {
        for (index, generation) in [(0, 0), (1, 0), (0, 1), (7, 3), (u32::MAX, u32::MAX)] {
            let id = EntityId::new(index, generation);
            assert_eq!(EntityId::from_bits(id.to_bits()), id);
        }
    }

    /// Two entities never share a packing, including the pair that would
    /// collide if the two halves were added rather than placed.
    #[test]
    fn different_handles_pack_differently() {
        assert_ne!(
            EntityId::new(1, 0).to_bits(),
            EntityId::new(0, 1).to_bits(),
            "the slot and the generation must not be interchangeable"
        );
    }
}
