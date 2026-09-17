//! Choosing one of several things, the same way every time.
//!
//! Five hundred patches of grass should not all be the same patch, and they
//! should not be a different five hundred next time the scene loads. What
//! decides is a hash of where the thing is, so nothing is stored per instance
//! and nothing is rolled at runtime: the map is the same on a phone, on a
//! desktop, and in a render capture taken by CI a year from now.
//!
//! The hash is written here rather than taken from `std`. `DefaultHasher` is
//! explicitly allowed to change between releases, and a scene whose grass
//! rearranges itself when the toolchain moves would be a render capture that
//! fails for no reason anybody can act on. FNV-1a is a few lines, and those
//! lines are the contract.

/// FNV-1a over the bytes of each value, in order.
///
/// Small, stable and good enough for picking between a handful of sprites. Not
/// a cryptographic hash and not trying to be: what matters is that the same
/// inputs give the same answer everywhere, forever.
#[must_use]
pub fn stable_hash(values: &[i64]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}

/// Folds a string into the same hash, so a tile ID can decide a choice.
///
/// Without this, two tiles at the same coordinate would make the same choice,
/// and a scene where the grass and the gravel varied in lockstep would look
/// authored by somebody with a very particular illness.
#[must_use]
pub fn stable_hash_with(values: &[i64], text: &str) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = stable_hash(values);
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Which of `weights` a hash lands on.
///
/// Weights are how often a look should appear relative to the others: four
/// plain patches to one with a flower is `[4, 1]`. A weight of zero is a look
/// that is defined and never chosen, which is a useful thing to be able to say
/// while working.
///
/// `None` when there is nothing to choose from, or when every weight is zero:
/// there is no answer to give and inventing one would mean drawing something
/// the author said not to.
#[must_use]
pub fn weighted_index(hash: u64, weights: &[u32]) -> Option<usize> {
    let total: u64 = weights.iter().map(|weight| u64::from(*weight)).sum();
    if total == 0 {
        return None;
    }
    let mut landed = hash % total;
    for (index, weight) in weights.iter().enumerate() {
        let weight = u64::from(*weight);
        if landed < weight {
            return Some(index);
        }
        landed -= weight;
    }
    // Unreachable while `landed < total`, which the modulo guarantees. Answered
    // rather than panicked because a tile picking a variant is not the place to
    // discover arithmetic has stopped working.
    None
}

#[cfg(test)]
mod tests {
    use super::{stable_hash, stable_hash_with, weighted_index};

    /// The whole point: the same cell gets the same answer, always.
    ///
    /// The literals are deliberate. If a change to the hash moves them, that
    /// change rearranges every scene that ever used it, and this test is the
    /// place to notice rather than a render capture three jobs later.
    #[test]
    fn the_hash_is_pinned_to_known_values() {
        // Nothing hashed is the offset basis, by definition.
        assert_eq!(stable_hash(&[]), 0xcbf2_9ce4_8422_2325);
        // Eight zero bytes rather than one: a value here is an i64, so the
        // single-byte FNV test vector is the wrong thing to compare against
        // and was the first thing this test caught.
        assert_eq!(stable_hash(&[0]), 0xa8c7_f832_281a_39c5);
        assert_eq!(stable_hash(&[1, 2, 3]), 0xda2b_fb22_5e0d_1f05);
    }

    #[test]
    fn different_places_get_different_answers() {
        let a = stable_hash(&[1, 1, 0]);
        let b = stable_hash(&[1, 2, 0]);
        let c = stable_hash(&[2, 1, 0]);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(b, c);
    }

    /// Two tiles in one cell must not vary in lockstep.
    #[test]
    fn the_name_changes_the_answer() {
        let cell = [4_i64, 7, 0];
        assert_ne!(
            stable_hash_with(&cell, "grass"),
            stable_hash_with(&cell, "gravel")
        );
    }

    #[test]
    fn one_weight_is_always_chosen() {
        for hash in [0, 1, 7, u64::MAX] {
            assert_eq!(weighted_index(hash, &[1]), Some(0));
        }
    }

    #[test]
    fn nothing_to_choose_from_has_no_answer() {
        assert_eq!(weighted_index(9, &[]), None);
        assert_eq!(weighted_index(9, &[0, 0]), None, "defined and never chosen");
    }

    /// A zero weight is skipped rather than occasionally landed on.
    #[test]
    fn a_zero_weight_is_never_chosen() {
        for hash in 0..64 {
            assert_ne!(weighted_index(hash, &[3, 0, 1]), Some(1));
        }
    }

    /// Weights are proportions, so over many cells they should hold.
    ///
    /// Not a distribution test dressed up as a unit test: the tolerance is wide
    /// on purpose. What would fail this is a walk that always picks the first
    /// bucket or ignores a weight, which is the mistake worth catching.
    #[test]
    fn weights_are_roughly_proportions() {
        let weights = [4, 1];
        let mut counts = [0_u32; 2];
        for cell in 0..4_000_i64 {
            let hash = stable_hash(&[cell % 64, cell / 64, 0]);
            if let Some(index) = weighted_index(hash, &weights) {
                counts[index] += 1;
            }
        }
        let ratio = f64::from(counts[0]) / f64::from(counts[1]);
        assert!(
            (2.0..6.5).contains(&ratio),
            "four to one should not come out {ratio:.2} to one: {counts:?}"
        );
    }
}
