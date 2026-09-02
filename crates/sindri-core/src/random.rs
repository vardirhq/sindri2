//! A deterministic pseudo-random stream the engine owns.
//!
//! Written out rather than pulled in. Every general-purpose crate reaches the
//! operating system for a seed, which on `wasm32-unknown-unknown` means
//! `getrandom` and a target that refuses to compile without an opt-in —
//! `docs/decay-direction.md` already records that hazard from Rhai. More to the
//! point, entropy is the opposite of what this is for: a run that cannot be
//! replayed from its seed is not seeded at all.
//!
//! The algorithm is **PCG-XSH-RR 64/32**, chosen because it is small enough to
//! read in one sitting and specified precisely enough that two hosts agree. It
//! is not cryptographic and must never be used as though it were: anyone who can
//! see a handful of outputs can recover the state and predict the rest.
//!
//! Everything here is integer arithmetic, and the one division is by a power of
//! two. That is what makes a seed mean the same thing in the editor, in a native
//! build, and in a browser — a float multiplied along the way would not.

/// A stream of numbers that a seed completely determines.
///
/// Same seed, same sequence of calls, same numbers — on every host. What it
/// does *not* promise is that adding a call somewhere leaves the rest alone:
/// one stream shared by everything means a number taken early shifts every
/// number after it. That is the honest cost of a single stream, and it is why
/// a run's seed is worth storing but a frame's numbers are not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rng {
    state: u64,
    /// The stream this generator walks, always odd.
    ///
    /// Two generators with the same state and different increments produce
    /// unrelated sequences, which is what lets a host give one part of a game
    /// its own numbers later without changing this type.
    increment: u64,
}

/// The multiplier PCG specifies. Not a number to tune.
const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// The default stream, from the same source.
const DEFAULT_STREAM: u64 = 1_442_695_040_888_963_407;

impl Default for Rng {
    /// A fixed seed, so a host that says nothing is repeatable rather than
    /// arbitrary.
    ///
    /// The engine has no way to be genuinely random without asking the platform
    /// for entropy, and it deliberately does not ask. A game that wants a
    /// different run each time seeds itself from something it knows — a counter
    /// it saved, the moment the person pressed Start — rather than from a source
    /// the engine pretends to have.
    fn default() -> Self {
        Self::from_seed(0)
    }
}

impl Rng {
    /// A stream determined by this seed.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self::with_stream(seed, DEFAULT_STREAM)
    }

    /// A stream determined by this seed, walking a chosen sequence.
    #[must_use]
    pub fn with_stream(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            // Always odd, which is what makes the sequence full-period.
            increment: (stream << 1) | 1,
        };
        // Two steps around the seed, as PCG specifies: seeding into the state
        // directly would make nearby seeds produce nearby first outputs, and
        // "seed 1 and seed 2 gave me almost the same wave" is a bug report
        // nobody enjoys.
        rng.step();
        rng.state = rng.state.wrapping_add(seed);
        rng.step();
        rng
    }

    /// Puts this stream back to the start of `seed`.
    #[must_use = "reseeding replaces the stream; use the returned generator"]
    pub fn reseeded(&self, seed: u64) -> Self {
        Self::with_stream(seed, self.increment >> 1)
    }

    /// The next 32 bits.
    pub fn next_u32(&mut self) -> u32 {
        let previous = self.state;
        self.step();
        // XSH RR: xor the high bits down, then rotate by the top five. The
        // rotation is what stops the low bits being predictable from the high
        // ones, which a plain shift would leave them.
        #[allow(clippy::cast_possible_truncation)]
        let xorshifted = (((previous >> 18) ^ previous) >> 27) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let rotation = (previous >> 59) as u32;
        xorshifted.rotate_right(rotation)
    }

    /// The next number in `[0, 1)`.
    ///
    /// Built from the top 24 bits, which is exactly what an `f32` mantissa
    /// holds, so every value it can return is equally likely and none of them is
    /// `1.0`. Dividing by `u32::MAX` instead — the obvious way — returns `1.0`
    /// on one draw in four billion, and a `[0, 1)` that is sometimes `1.0` is a
    /// bug that shows up once a month in someone else's code.
    pub fn next_f32(&mut self) -> f32 {
        // 2^24, the last integer an f32 counts to without skipping. Written out
        // rather than shifted so the value is the one being reasoned about.
        const SCALE: f32 = 1.0 / 16_777_216.0;
        #[allow(clippy::cast_precision_loss)]
        {
            (self.next_u32() >> 8) as f32 * SCALE
        }
    }

    /// A number in `[0, bound)`, every value equally likely.
    ///
    /// The obvious `next_u32() % bound` is biased whenever `bound` does not
    /// divide 2^32: the first few values come up slightly more often. On a
    /// six-sided die nobody notices; on a weighted drop table across a long run
    /// it is exactly the kind of wrongness that gets blamed on the game design.
    /// So the few draws that would land in the biased tail are thrown away.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        // Everything below this is in a complete cycle of `bound` values.
        let limit = u32::MAX - (u32::MAX % bound) - (bound - 1);
        loop {
            let drawn = self.next_u32();
            if drawn <= limit {
                return drawn % bound;
            }
        }
    }
}

impl Rng {
    /// One step of the state, which is all the sequence is.
    fn step(&mut self) {
        self.state = self
            .state
            .wrapping_mul(MULTIPLIER)
            .wrapping_add(self.increment);
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    /// The whole promise: a seed is a run.
    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let first: Vec<u32> = (0..32).map(|_| Rng::from_seed(7).next_u32()).collect();
        let mut stream = Rng::from_seed(7);
        let second: Vec<u32> = (0..32).map(|_| stream.next_u32()).collect();
        assert_eq!(first[0], second[0]);

        let mut a = Rng::from_seed(7);
        let mut b = Rng::from_seed(7);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_give_different_numbers() {
        let mut a = Rng::from_seed(1);
        let mut b = Rng::from_seed(2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    /// "Seed 1 and seed 2 gave me almost the same wave" is a bug report nobody
    /// enjoys, and seeding into the state directly would earn it.
    #[test]
    fn neighbouring_seeds_do_not_start_alike() {
        let firsts: Vec<u32> = (0..16)
            .map(|seed| Rng::from_seed(seed).next_u32())
            .collect();
        for (index, value) in firsts.iter().enumerate() {
            for other in &firsts[index + 1..] {
                let apart = value.abs_diff(*other);
                assert!(apart > 1_000_000, "{value} and {other} are too close");
            }
        }
    }

    #[test]
    fn reseeding_returns_to_the_start_of_that_seed() {
        let mut stream = Rng::from_seed(9);
        let expected: Vec<u32> = (0..8).map(|_| stream.next_u32()).collect();
        for _ in 0..100 {
            stream.next_u32();
        }
        let mut back = stream.reseeded(9);
        let again: Vec<u32> = (0..8).map(|_| back.next_u32()).collect();
        assert_eq!(expected, again);
    }

    /// A `[0, 1)` that is sometimes `1.0` is a bug that shows up once a month in
    /// someone else's code.
    #[test]
    fn a_fraction_is_never_one_and_never_negative() {
        let mut stream = Rng::from_seed(3);
        for _ in 0..100_000 {
            let value = stream.next_f32();
            assert!((0.0..1.0).contains(&value), "{value}");
        }
    }

    #[test]
    fn fractions_cover_the_whole_span() {
        let mut stream = Rng::from_seed(4);
        let mut buckets = [0_u32; 10];
        for _ in 0..100_000 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let bucket = (stream.next_f32() * 10.0) as usize;
            buckets[bucket.min(9)] += 1;
        }
        for (index, count) in buckets.iter().enumerate() {
            assert!(*count > 8_000, "bucket {index} held only {count}");
        }
    }

    #[test]
    fn a_bound_is_never_reached() {
        let mut stream = Rng::from_seed(5);
        for _ in 0..10_000 {
            assert!(stream.below(6) < 6);
        }
    }

    /// On a drop table across a long run, modulo bias is exactly the kind of
    /// wrongness that gets blamed on the game design.
    #[test]
    fn a_bound_that_does_not_divide_evenly_is_still_fair() {
        let mut stream = Rng::from_seed(6);
        let mut counts = [0_u32; 3];
        for _ in 0..300_000 {
            counts[stream.below(3) as usize] += 1;
        }
        for count in counts {
            assert!((99_000..101_000).contains(&count), "{counts:?}");
        }
    }

    #[test]
    fn a_bound_of_nothing_is_nothing_rather_than_a_panic() {
        assert_eq!(Rng::from_seed(1).below(0), 0);
        assert_eq!(Rng::from_seed(1).below(1), 0);
    }

    /// A host that says nothing is repeatable rather than arbitrary.
    #[test]
    fn the_default_stream_is_a_fixed_one() {
        assert_eq!(Rng::default(), Rng::from_seed(0));
        assert_eq!(Rng::default().next_u32(), Rng::default().next_u32());
    }

    /// Two streams let a host give one part of a game its own numbers without
    /// this type changing.
    #[test]
    fn two_streams_from_one_seed_do_not_agree() {
        let mut first = Rng::with_stream(11, 1);
        let mut second = Rng::with_stream(11, 2);
        assert_ne!(first.next_u32(), second.next_u32());
    }
}
