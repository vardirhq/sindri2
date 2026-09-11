//! The models Sindri will recommend, how much room each needs, and how well it
//! would run on this machine.
//!
//! Two rules decide everything here. A model is graded on what it can *do* —
//! whether it holds a schema and emits a clean tool call and knows when to stop
//! — rather than on where it sits on a general coding leaderboard, because the
//! only question that matters is whether it can drive Sindri's protocol. And it
//! is never offered as though it will run well on hardware that cannot hold it:
//! a recommendation that ends in a swap-thrashing machine is worse than none,
//! because whoever took it concludes that local AI does not work.
//!
//! The models themselves live in a committed manifest rather than in this file.
//! A manifest is versioned, validated on load, and carries the licence and
//! source of everything it names — and a model added to it needs no code.

use std::sync::OnceLock;

use serde::Deserialize;

/// Gigabytes, as the unit everything here is stated in.
type Gb = f32;

/// The manifest schema this build understands.
///
/// Checked rather than carried: a manifest written to a later schema may mean
/// something different by the same field names, and reading it as though it did
/// not is how a model gets recommended on a number that has changed meaning.
const SCHEMA_VERSION: u32 = 1;

/// The manifest, compiled in.
///
/// Committed rather than fetched: what Sindri recommends is part of the build
/// that was tested, and a list that can change underneath an editor is a list
/// that can recommend something that build has never run.
const MANIFEST: &str = include_str!("../../assets/ai-models.json");

/// The context length residency is estimated against.
///
/// A variable rather than a constant of nature, and the reason estimation
/// beats a hardcoded figure per model: context is the part of the memory bill
/// the person changes, and a table of fixed numbers is wrong the moment they do.
pub const DEFAULT_CONTEXT: u32 = 16_384;

/// What a model can do, as far as Sindri's own verification is concerned.
///
/// Reported rather than promised: each is a claim setup verifies against the
/// real model before the editor enables anything that depends on it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Supports {
    /// Can be held to a strict schema, which every proposal is.
    #[serde(default)]
    pub structured_output: bool,
    #[serde(default)]
    pub tool_calling: bool,
    #[serde(default)]
    pub vision: bool,
}

/// One model Sindri knows about.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Profile {
    /// What the runner calls it, which is what gets pulled.
    pub id: String,
    /// Billions of parameters, which is what residency is estimated from.
    pub params_b: f32,
    pub quantisation: String,
    pub display_name: String,
    /// Why Sindri suggests it, shown before anything is downloaded.
    pub description: String,
    pub license: String,
    pub source: String,
    /// Whether this is the model Sindri leads with where it fits.
    ///
    /// A designated standard rather than "the biggest that fits", because
    /// bigger is not better past the point where a model drives the tool loop
    /// reliably: beyond it the extra parameters buy quality at the cost of
    /// context headroom and speed, which is a trade for someone to make
    /// deliberately rather than a default to be handed.
    #[serde(default)]
    pub standard: bool,
    #[serde(flatten)]
    pub supports: Supports,
}

/// How well a model would run here.
///
/// Four grades rather than a yes or no, because "runs, but slowly, and will
/// drop a tool call on a multi-step edit" is a real and common answer that a
/// boolean has nowhere to put — and hiding it reads as Sindri not supporting a
/// model the person can plainly see running.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tier {
    /// Does not fit. Will spill and crawl.
    Unsupported,
    /// Fits and is useful, but under the bar for driving the tool loop
    /// reliably through a multi-step edit.
    BestEffort,
    /// Fits, but near the ceiling: less context headroom, slower.
    Supported,
    /// The works-properly bar.
    Recommended,
}

impl Tier {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unsupported => "Will not fit",
            Self::BestEffort => "Best effort",
            Self::Supported => "Supported",
            Self::Recommended => "Recommended",
        }
    }

    /// Whether Sindri would put this forward on its own.
    pub const fn is_offered(self) -> bool {
        matches!(self, Self::Recommended | Self::Supported)
    }
}

/// Below this many billion parameters a model stops driving the tool loop
/// dependably, however well it converses.
const TOOL_LOOP_FLOOR: f32 = 4.0;

/// How much of the available memory a model may claim before it counts as
/// near the ceiling rather than comfortable.
///
/// Four fifths, which is what puts the reference card's 14B at its ceiling
/// rather than at its standard — the grading this is checked against calls
/// that model the best quality a 12 GB card can host, not the one to lead with.
const CEILING: f32 = 0.8;

impl Profile {
    /// What this needs resident, in gigabytes, at a given context length.
    ///
    /// Weights at roughly half a gigabyte per billion parameters at Q4, plus
    /// the key/value cache, which scales with context, plus the runner's own
    /// overhead. An estimate rather than a measurement, and deliberately a
    /// little pessimistic: the failure it exists to prevent is recommending
    /// something that does not fit.
    pub fn residency(&self, context_tokens: u32) -> Gb {
        let weights = self.params_b * 0.58;
        let cache =
            f32::from(u16::try_from(context_tokens.max(2_048) / 1_024).unwrap_or(u16::MAX)) / 16.0;
        weights + cache + 0.6
    }

    /// How well it would run on a machine with this much to spare.
    pub fn tier(&self, available: Gb, context_tokens: u32) -> Tier {
        let needs = self.residency(context_tokens);
        if needs > available {
            return Tier::Unsupported;
        }
        if self.params_b < TOOL_LOOP_FLOOR {
            return Tier::BestEffort;
        }
        if needs > available * CEILING {
            return Tier::Supported;
        }
        Tier::Recommended
    }
}

/// Every model the manifest names, smallest first.
///
/// Parsed once. A manifest that will not parse is a build problem rather than a
/// runtime one — `the_manifest_is_valid` fails before it ships — so this treats
/// a failure as empty rather than carrying an error nobody could act on.
pub fn profiles() -> &'static [Profile] {
    static PARSED: OnceLock<Vec<Profile>> = OnceLock::new();
    PARSED.get_or_init(|| {
        let Ok(manifest) = serde_json::from_str::<Manifest>(MANIFEST) else {
            return Vec::new();
        };
        if manifest.schema_version != SCHEMA_VERSION {
            return Vec::new();
        }
        let mut all: Vec<Profile> = manifest.models.into_values().flatten().collect();
        all.sort_by(|one, other| one.params_b.total_cmp(&other.params_b));
        all
    })
}

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    models: std::collections::BTreeMap<String, Vec<Profile>>,
}

/// What Sindri would put forward for a machine with this much to spare.
///
/// The most capable model that still grades as comfortable, never one that is
/// merely near the ceiling: the difference between those two is the difference
/// between an assistant that works and one that mostly works, and a first
/// recommendation should not be the second.
///
/// `None` when nothing qualifies, which is a real answer and must be said
/// rather than papered over with the smallest model in the list.
pub fn recommended(available: Gb, context_tokens: u32) -> Option<&'static Profile> {
    let comfortable =
        |profile: &&Profile| profile.tier(available, context_tokens) == Tier::Recommended;
    profiles()
        .iter()
        .find(|profile| profile.standard && comfortable(profile))
        .or_else(|| profiles().iter().rfind(comfortable))
}

/// The profile for an id the runner reports, if the manifest names it.
///
/// A model someone pulled themselves is not refused for being unknown — it is
/// unprofiled, and verification rather than this table decides what it can do.
pub fn profile_for(id: &str) -> Option<&'static Profile> {
    profiles().iter().find(|profile| profile.id == id)
}

#[cfg(test)]
mod tests;
