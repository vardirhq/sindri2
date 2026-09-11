//! The models Sindri will recommend, and whether one fits this machine.
//!
//! Two rules decide everything here. A model is recommended only when it has
//! been *measured* against Sindri's own conformance cases rather than chosen
//! from a general coding leaderboard — the question is whether it can call
//! Sindri's tools and answer Sindri's diagnostics, not whether it scores well on
//! repository-scale benchmarks. And a model is never recommended onto hardware
//! that cannot hold it: a suggestion that ends in a swap-thrashing machine or an
//! out-of-memory failure is worse than no suggestion, because the person who
//! took it now believes local AI does not work.
//!
//! Profiles are tied to editor releases rather than hardcoded forever, because
//! what a machine can run depends on quantisation, context length, GPU offload,
//! and backend version, and all four move.

/// Gigabytes, as the unit everything here is stated in.
type Gb = f32;

/// How much room a model needs beyond its own weights.
///
/// Context, key/value cache and backend overhead all live in the same memory as
/// the weights. Ignoring them is how a 8.1 GB model gets recommended onto a
/// 8 GB card and then runs on the CPU at a tenth of the speed, which reads to
/// the person who tried it as "local AI is useless" rather than "that was the
/// wrong model".
const HEADROOM: Gb = 2.5;

/// What a model can do, as far as Sindri's own verification is concerned.
///
/// Reported rather than promised: each of these is a claim the setup verifies
/// against the real model before the editor enables anything that depends on
/// it. See `Feature`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Supports {
    /// Can be made to answer in a strict schema, which the proposal protocol
    /// requires of every response.
    pub structured_output: bool,
    pub tool_calling: bool,
    pub vision: bool,
}

/// One model Sindri knows about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// What the backend calls it, which is what gets pulled.
    pub tag: &'static str,
    /// The download, in gigabytes.
    pub download: Gb,
    /// What it needs resident to run at full speed.
    pub resident: Gb,
    pub licence: &'static str,
    pub supports: Supports,
    /// Why Sindri suggests it, in one line, shown before anything is downloaded.
    pub because: &'static str,
}

impl Profile {
    /// Whether this fits in `available` gigabytes with room to work.
    pub fn fits(self, available: Gb) -> bool {
        available >= self.resident + HEADROOM
    }

    /// What it needs in total, which is the number worth showing a person.
    pub fn needs(self) -> Gb {
        self.resident + HEADROOM
    }
}

/// The models this editor build has profiles for, roomiest last.
///
/// Ordered so that picking the best fit is a scan: the first that fits is the
/// smallest adequate one, and walking to the end finds the largest that does.
pub const PROFILES: [Profile; 3] = [
    Profile {
        tag: "qwen3:8b",
        download: 5.2,
        resident: 6.5,
        licence: "Apache-2.0",
        supports: Supports {
            structured_output: true,
            tool_calling: true,
            vision: false,
        },
        because: "The baseline Sindri tunes for: tool calling and strict schemas on a mid-range card",
    },
    Profile {
        tag: "gemma3:12b",
        download: 8.1,
        resident: 9.5,
        licence: "Gemma Terms of Use",
        supports: Supports {
            structured_output: true,
            tool_calling: true,
            vision: true,
        },
        because: "Adds vision, for asking about a sprite or a screenshot",
    },
    Profile {
        tag: "qwen3-coder:30b",
        download: 19.0,
        resident: 21.0,
        licence: "Apache-2.0",
        supports: Supports {
            structured_output: true,
            tool_calling: true,
            vision: false,
        },
        because: "Stronger Decay generation, for a machine with room for it",
    },
];

/// What Sindri would suggest for a machine with this much to spare.
///
/// The largest profile that fits, because the extra capability is worth having
/// when the memory is there. `None` when nothing fits, which is a real answer
/// and must be said rather than papered over with the smallest one — a person
/// whose machine cannot run any of these is better served by being told so and
/// pointed at a remote endpoint.
pub fn recommended(available: Gb) -> Option<Profile> {
    PROFILES
        .into_iter()
        .rfind(|profile| profile.fits(available))
}

/// The profile for a tag the backend reports, if this build knows it.
///
/// A model the user pulled themselves is not refused for being unknown — it is
/// simply unprofiled, and its capabilities come from verification rather than
/// from this table.
pub fn profile_for(tag: &str) -> Option<Profile> {
    PROFILES.into_iter().find(|profile| profile.tag == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_needs_room_beyond_its_own_weights() {
        let baseline = profile_for("qwen3:8b").expect("the baseline is profiled");
        assert!(
            !baseline.fits(baseline.resident),
            "a card with exactly the weights and nothing spare does not fit it"
        );
        assert!(baseline.fits(baseline.needs()));
    }

    /// The headline target the architecture names: a 12 GB card should get a
    /// real recommendation, and it should not be the largest profile.
    #[test]
    fn twelve_gigabytes_is_offered_something_that_runs_on_it() {
        let chosen = recommended(12.0).expect("12 GB can run something");
        assert!(chosen.fits(12.0));
        assert_ne!(
            chosen.tag, "qwen3-coder:30b",
            "the 30b package exceeds 12 GB before context and overhead"
        );
    }

    #[test]
    fn more_memory_is_offered_more_capability() {
        let small = recommended(9.5).expect("9.5 GB runs the baseline");
        let large = recommended(32.0).expect("32 GB runs the largest");
        assert!(large.resident > small.resident);
    }

    /// The rule this exists for. Recommending something that will not run is
    /// worse than recommending nothing, because the person who takes the
    /// suggestion concludes that local AI does not work.
    #[test]
    fn a_machine_that_can_run_none_of_them_is_told_so() {
        assert_eq!(recommended(4.0), None);
        assert_eq!(recommended(0.0), None);
    }

    #[test]
    fn every_recommendation_actually_fits_the_machine_it_was_made_for() {
        for available in [6.0, 9.0, 12.0, 16.0, 24.0, 48.0] {
            if let Some(profile) = recommended(available) {
                assert!(
                    profile.fits(available),
                    "{} was recommended for {available} GB and does not fit",
                    profile.tag
                );
            }
        }
    }

    /// Every profile has to be able to answer in a schema, because the proposal
    /// protocol is a schema and a model that cannot hold to one cannot author.
    #[test]
    fn every_profile_can_produce_structured_output() {
        for profile in PROFILES {
            assert!(
                profile.supports.structured_output,
                "{} cannot be recommended without structured output",
                profile.tag
            );
        }
    }

    #[test]
    fn every_profile_says_why_it_is_suggested_and_under_what_licence() {
        for profile in PROFILES {
            assert!(!profile.because.is_empty(), "{} has no reason", profile.tag);
            assert!(
                !profile.licence.is_empty(),
                "{} has no licence",
                profile.tag
            );
            assert!(profile.download > 0.0);
        }
    }

    #[test]
    fn the_profiles_are_ordered_roomiest_last() {
        let mut previous = 0.0;
        for profile in PROFILES {
            assert!(
                profile.resident > previous,
                "{} is out of order",
                profile.tag
            );
            previous = profile.resident;
        }
    }

    #[test]
    fn a_model_the_user_pulled_themselves_is_simply_unprofiled() {
        assert_eq!(profile_for("some-local-model:latest"), None);
    }
}
