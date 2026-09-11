//! Getting from no local AI at all to a verified assistant, one step at a time.
//!
//! The setup is the feature. A person who cannot get a local model running has
//! no opinion about how good Sindri's assistant is, and the ways this goes
//! wrong are mostly not interesting: the backend is not installed, or it is
//! installed but not running, or it is running but holds no model that can do
//! the job. Each of those is a different sentence and a different next action,
//! and an editor that says "could not connect" to all three has answered none
//! of them.
//!
//! So the states are named and exhaustive, and every one of them knows the
//! single next thing to do. The UI renders that; it does not decide it. A CLI
//! could render the same thing.
//!
//! **Nobody is asked to type anything.** Not a command, not a model name, not a
//! path. The one exception is a password, and only where the operating system
//! itself demands one to install software — that prompt belongs to the OS, not
//! to Sindri, and Sindri never sees what is typed into it. Everything else is a
//! button: install the runner, start it, take the model Sindri suggests or pick
//! a different one from a list, prove it works.
//!
//! An editor that prints `curl … | sh` and waits has not automated anything; it
//! has moved the work into a terminal and called that a setup flow. The person
//! who cannot get past that step is exactly the person this exists for.
//!
//! What that buys has to be paid for in care, because installing software on
//! someone's machine is a real thing to do. So: the source is pinned rather
//! than discovered, what will run is shown before it runs, nothing is elevated
//! without the person seeing why, and consent is explicit at every step that
//! costs disk, time, or privilege. Where a runner can be installed without
//! privilege at all, that route is preferred — see [`Elevation`].

pub mod catalogue;
pub mod probe;

pub use catalogue::{Profile, Supports};

/// What the editor could see when it last looked.
///
/// Gathered by something that can talk to a backend and read the machine; kept
/// separate from the state machine so the states can be tested without a
/// network, a GPU, or a model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    /// Whether a supported backend appears to be installed at all.
    pub backend_installed: bool,
    /// Whether it answered.
    pub backend_reachable: bool,
    /// What it calls itself, when it answered.
    pub backend_version: Option<String>,
    /// The models it reports holding.
    pub models: Vec<String>,
    /// Memory available to a model, in gigabytes: video memory where there is a
    /// usable GPU, otherwise what the system can spare.
    ///
    /// `None` when it could not be determined, which is a distinct answer from
    /// zero and must not be treated as "nothing fits" — an undetectable GPU is
    /// common and the person can still choose for themselves.
    pub available_memory: Option<f32>,
}

/// One thing Sindri verified the chosen model can actually do.
///
/// Reported individually rather than as a connection light, because "connected"
/// is not a capability and a model that answers HTTP but cannot hold a schema
/// will fail every proposal it is asked for. A feature that fails verification
/// stays switched off rather than failing later in front of a person's scene.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Feature {
    /// Answers in a strict schema.
    StructuredOutput,
    /// Calls a read-only tool with valid arguments.
    ToolCalling,
    /// Resolves a stable entity reference from a small fixture.
    ReferenceResolution,
    /// Produces a valid component proposal from a supplied schema.
    ComponentProposals,
    /// Writes a small valid Decay function.
    DecayAuthoring,
    /// Corrects an invalid Decay function from compiler diagnostics.
    DecayRepair,
    /// Reads an image.
    Vision,
}

impl Feature {
    /// Every feature verification can report on, in the order it is shown.
    pub const ALL: [Self; 7] = [
        Self::StructuredOutput,
        Self::ToolCalling,
        Self::ReferenceResolution,
        Self::ComponentProposals,
        Self::DecayAuthoring,
        Self::DecayRepair,
        Self::Vision,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::StructuredOutput => "Structured answers",
            Self::ToolCalling => "Tool calling",
            Self::ReferenceResolution => "Scene inspection",
            Self::ComponentProposals => "Component proposals",
            Self::DecayAuthoring => "Decay editing",
            Self::DecayRepair => "Decay repair",
            Self::Vision => "Vision",
        }
    }

    /// Whether the assistant is usable at all without it.
    ///
    /// The protocol is a schema and every proposal is a tool call, so a model
    /// failing either of those cannot author anything however well it chats.
    /// The rest degrade: no vision means no asking about a sprite, not a broken
    /// assistant.
    pub const fn required(self) -> bool {
        matches!(self, Self::StructuredOutput | Self::ToolCalling)
    }
}

/// How far along setup is, and therefore what to say.
#[derive(Clone, Debug, PartialEq)]
pub enum Readiness {
    /// No supported backend on the machine.
    BackendMissing,
    /// Installed, but nothing is answering.
    BackendStopped,
    /// Answering, but holding no model Sindri can use.
    NoModel,
    /// A usable model is present but has not been put to the test.
    Unverified { model: String },
    /// Being put to the test now.
    Verifying { model: String },
    /// Verified. Carries what it proved it can do.
    Ready {
        model: String,
        verified: Vec<Feature>,
    },
    /// Present and answering, but failed something the assistant cannot work
    /// without.
    Unusable { model: String, failed: Feature },
}

/// What the editor should do next, and what it should say while offering it.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub title: String,
    pub detail: String,
    pub action: Action,
}

/// Whether performing an action needs the operating system's permission.
///
/// The only thing in the whole setup a person types, and only when the platform
/// leaves no alternative. A user-space install is preferred wherever one exists
/// precisely so this can stay [`Elevation::NotNeeded`]: a setup that does not
/// need a password cannot be blocked by not having one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Elevation {
    /// Installs into the user's own directory. No prompt.
    NotNeeded,
    /// The OS will ask for a password. Sindri never sees it.
    Password,
}

/// What the editor will do, shown before it does it.
#[derive(Clone, Debug, PartialEq)]
pub struct InstallPlan {
    /// Where it comes from, pinned rather than discovered.
    pub source: &'static str,
    /// The download, in gigabytes, so consent is informed.
    pub download: f32,
    pub elevation: Elevation,
    /// What will actually be run, in one line, for anyone who wants to know.
    ///
    /// Shown, not typed. Nobody has to copy it anywhere; it is there so the
    /// step is inspectable rather than opaque.
    pub performs: &'static str,
}

/// The next action, as data.
///
/// Data rather than a closure for the same reason the palette's is: the part of
/// the editor that owns the state performs it, a test can assert which one was
/// offered without performing anything, and a CLI can offer the same set.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Install the runner. The editor does this; the person presses the button.
    Install { plan: InstallPlan },
    /// Start the runner the editor already found.
    Start,
    /// Download a model, with consent, having said what it costs.
    Pull { profile: Profile },
    /// Offer models to click.
    ///
    /// Never a field to type into. `fits` says which of them this machine can
    /// actually hold, so an unusable choice is visibly unusable rather than
    /// merely absent.
    PickFrom { choices: Vec<(Profile, bool)> },
    /// Run the verification suite.
    Verify { model: String },
    /// Nothing to do.
    None,
}

/// Where the runner comes from. Pinned, not discovered.
const INSTALL_SOURCE: &str = "https://ollama.com";

/// How the runner gets installed on this platform.
///
/// Linux takes the user-space route, which needs no password: the binary goes
/// under the user's own directory and is started as them. macOS and Windows
/// have one supported installer each and both may ask the OS for permission, so
/// the plan says so in advance rather than surprising anyone with a prompt.
fn install_plan() -> InstallPlan {
    #[cfg(target_os = "linux")]
    {
        InstallPlan {
            source: INSTALL_SOURCE,
            download: 1.6,
            elevation: Elevation::NotNeeded,
            performs: "Unpacks the Ollama binary into ~/.local/share/sindri/ollama",
        }
    }
    #[cfg(target_os = "macos")]
    {
        InstallPlan {
            source: INSTALL_SOURCE,
            download: 1.2,
            elevation: Elevation::Password,
            performs: "Runs the signed Ollama installer package",
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        InstallPlan {
            source: INSTALL_SOURCE,
            download: 1.1,
            elevation: Elevation::Password,
            performs: "Runs the signed Ollama installer",
        }
    }
}

/// Every profiled model, each marked with whether this machine can hold it.
///
/// All of them, not just the ones that fit: a list that silently omits the
/// choice someone was looking for reads as Sindri not supporting it. Marking it
/// says the true thing instead, and leaves the decision where it belongs.
fn choices_for(available: Option<f32>) -> Vec<(Profile, bool)> {
    catalogue::PROFILES
        .into_iter()
        .map(|profile| {
            let fits = available.is_none_or(|room| profile.fits(room));
            (profile, fits)
        })
        .collect()
}

/// Reads a probe as one of the named states.
///
/// Order matters and is the diagnosis: not installed is asked before not
/// running, which is asked before no model, because each later question is
/// meaningless when an earlier one is the answer. This is the whole reason the
/// editor can say something specific instead of "could not connect".
pub fn readiness(probe: &Probe, verified: Option<(&str, &[Feature])>) -> Readiness {
    if !probe.backend_installed {
        return Readiness::BackendMissing;
    }
    if !probe.backend_reachable {
        return Readiness::BackendStopped;
    }
    let Some(model) = usable_model(probe) else {
        return Readiness::NoModel;
    };
    match verified {
        Some((tested, features)) if tested == model => {
            if let Some(missing) = Feature::ALL
                .into_iter()
                .find(|feature| feature.required() && !features.contains(feature))
            {
                Readiness::Unusable {
                    model: model.to_owned(),
                    failed: missing,
                }
            } else {
                Readiness::Ready {
                    model: model.to_owned(),
                    verified: features.to_vec(),
                }
            }
        }
        _ => Readiness::Unverified {
            model: model.to_owned(),
        },
    }
}

/// Which of the reported models the editor would use.
///
/// A profiled one first, roomiest of those the machine can hold, because those
/// are the ones Sindri has actually measured. Otherwise whatever is there: a
/// model someone pulled themselves is not refused for being unknown, it is just
/// unprofiled, and verification rather than a table decides what it can do.
fn usable_model(probe: &Probe) -> Option<&str> {
    let room = probe.available_memory;
    let profiled = catalogue::PROFILES.into_iter().rev().find(|profile| {
        probe.models.iter().any(|held| held == profile.tag)
            && room.is_none_or(|available| profile.fits(available))
    });
    profiled
        .map(|profile| profile.tag)
        .or_else(|| probe.models.first().map(String::as_str))
}

impl Readiness {
    /// The one next thing to do, and what to say while offering it.
    pub fn step(&self, probe: &Probe) -> Step {
        match self {
            Self::BackendMissing => Step {
                title: "Install the model runner".to_owned(),
                detail: format!(
                    "Sindri runs models on your machine through Ollama. This installs it {}.",
                    match install_plan().elevation {
                        Elevation::NotNeeded =>
                            "into your own user folder, with nothing to confirm",
                        Elevation::Password =>
                            "system-wide, so your operating system will ask for \
                                                your password",
                    }
                ),
                action: Action::Install {
                    plan: install_plan(),
                },
            },
            Self::BackendStopped => Step {
                title: "Start the model runner".to_owned(),
                detail: "Ollama is installed but not answering. Sindri will start it.".to_owned(),
                action: Action::Start,
            },
            Self::NoModel => match probe.available_memory.and_then(catalogue::recommended) {
                Some(profile) => Step {
                    title: format!("Download {}", profile.tag),
                    detail: format!(
                        "{}. {:.1} GB download, needs about {:.1} GB free to run, {}.",
                        profile.because,
                        profile.download,
                        profile.needs(),
                        profile.licence
                    ),
                    action: Action::Pull { profile },
                },
                None => Step {
                    title: "Pick a model".to_owned(),
                    detail: match probe.available_memory {
                        // Said plainly rather than softened: a person whose
                        // machine cannot hold any profiled model is better
                        // served by knowing it than by a suggestion that will
                        // not run. The choice stays open anyway — it is their
                        // machine, and the report can be wrong.
                        Some(available) => format!(
                            "This machine reports {available:.1} GB for a model, less than any \
                             model Sindri has measured needs. These will be slow or will not \
                             load, but the choice is yours."
                        ),
                        None => "Sindri could not tell how much memory is available, so it will \
                                 not choose for you."
                            .to_owned(),
                    },
                    action: Action::PickFrom {
                        choices: choices_for(probe.available_memory),
                    },
                },
            },
            Self::Unverified { model } => Step {
                title: format!("Check what {model} can do"),
                detail: "Sindri runs its own cases against the model rather than trusting that \
                         a reachable model is a working one."
                    .to_owned(),
                action: Action::Verify {
                    model: model.clone(),
                },
            },
            Self::Verifying { model } => Step {
                title: format!("Checking {model}…"),
                detail: String::new(),
                action: Action::None,
            },
            Self::Ready { model, .. } => Step {
                title: format!("{model} is ready"),
                detail: String::new(),
                action: Action::None,
            },
            Self::Unusable { model, failed } => Step {
                title: format!("{model} cannot author"),
                detail: format!(
                    "It failed {}, which every proposal depends on. Another model is likely to \
                     do better than tuning this one.",
                    failed.label().to_lowercase()
                ),
                action: Action::PickFrom {
                    choices: choices_for(probe.available_memory),
                },
            },
        }
    }

    /// Whether the assistant can be used.
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Whether the editor should keep probing.
    ///
    /// True exactly while the next move is the person's and happens outside the
    /// editor. This is what makes installing feel smooth rather than like a
    /// form: the editor is watching, so the moment the backend appears the
    /// screen moves on by itself.
    pub const fn awaiting_the_machine(&self) -> bool {
        matches!(self, Self::BackendMissing | Self::BackendStopped)
    }
}

#[cfg(test)]
mod tests;
