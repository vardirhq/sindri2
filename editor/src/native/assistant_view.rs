//! The setup panel: one step at a time, each of them a button.
//!
//! Renders whatever [`Readiness`] the probe resolved to, and performs whichever
//! [`Action`] that state offered. It decides nothing — the state machine in
//! `crate::assistant` decides, and this draws the decision, which is what keeps
//! the flow testable without a window.
//!
//! Nothing here asks anyone to type. See the module note on `crate::assistant`.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use crate::assistant::{
    Action, Feature, InstallPlan, Probe, Profile, Readiness, Tier, probe, readiness,
};
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{
    button::{self, Intent},
    panel,
};

use super::EditorApp;

/// How often the machine is looked at again while setup is waiting on it.
///
/// The thing that makes an install feel finished the moment it is: nobody hunts
/// for a refresh button, the screen simply moves on. Slower once there is
/// nothing to wait for, because polling a settled machine is just heat.
const WATCHING: Duration = Duration::from_secs(2);
const SETTLED: Duration = Duration::from_secs(30);

/// What the editor knows about the local assistant.
#[derive(Default)]
pub(crate) struct AssistantState {
    /// The machine as the last completed probe saw it.
    seen: Probe,
    /// Which model was verified, and what it proved it could do.
    verified: Option<(String, Vec<Feature>)>,
    /// A probe in flight, so the frame never waits on a socket.
    looking: Option<Receiver<Probe>>,
    last_looked: Option<Instant>,
    /// What is happening right now, when something is.
    working: Option<String>,
}

impl EditorApp {
    /// The setup flow, or the assistant once there is one.
    pub(super) fn assistant_body(&mut self, ui: &mut egui::Ui) {
        self.poll_probe(ui.ctx());
        let state = readiness(
            &self.assistant.seen,
            self.assistant
                .verified
                .as_ref()
                .map(|(model, features)| (model.as_str(), features.as_slice())),
        );
        panel::body(ui, |ui| {
            if let Readiness::Ready { model, verified } = &state {
                ready(ui, model, verified);
                return;
            }
            let step = state.step(&self.assistant.seen);
            ui.add_space(4.0);
            ui.label(
                RichText::new(&step.title)
                    .size(text::HEADING)
                    .color(color::TEXT),
            );
            if !step.detail.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(&step.detail)
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                );
            }
            ui.add_space(12.0);
            if let Some(working) = self.assistant.working.clone() {
                ui.label(
                    RichText::new(working)
                        .size(text::LABEL)
                        .color(color::FORGE_BRIGHT),
                );
                return;
            }
            if let Some(chosen) = offer(ui, &step.action) {
                self.perform(chosen, ui.ctx());
            }
        });
    }

    /// The panel's own control: look again, now.
    pub(super) fn assistant_actions(&mut self, ui: &mut egui::Ui) {
        if button::row_icon(ui, icons::REFRESH, Intent::Quiet, "Look again").clicked() {
            self.assistant.last_looked = None;
        }
    }

    /// Starts a probe when one is due, and takes the answer when it lands.
    ///
    /// On a worker rather than in the frame: a shut port costs a connection
    /// timeout, and a frame that waits on one drops.
    fn poll_probe(&mut self, context: &egui::Context) {
        if let Some(channel) = &self.assistant.looking {
            match channel.try_recv() {
                Ok(seen) => {
                    self.assistant.seen = seen;
                    self.assistant.looking = None;
                    self.assistant.working = None;
                }
                Err(TryRecvError::Empty) => {
                    context.request_repaint_after(Duration::from_millis(200));
                    return;
                }
                Err(TryRecvError::Disconnected) => self.assistant.looking = None,
            }
        }
        let state = readiness(&self.assistant.seen, None);
        let every = if state.awaiting_the_machine() || self.assistant.working.is_some() {
            WATCHING
        } else {
            SETTLED
        };
        let due = self
            .assistant
            .last_looked
            .is_none_or(|last| last.elapsed() >= every);
        if !due {
            context.request_repaint_after(every);
            return;
        }
        self.assistant.last_looked = Some(Instant::now());
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let _ = sender.send(probe::probe());
        });
        self.assistant.looking = Some(receiver);
    }

    /// Carries out whichever action the step offered.
    fn perform(&mut self, action: Action, context: &egui::Context) {
        match action {
            Action::Install { plan } => self.get_runner(&plan, context),
            Action::Start => self.start_runner(),
            Action::Pull { profile } => self.pull_model(&profile),
            Action::PickFrom { .. } | Action::Verify { .. } | Action::None => {}
        }
    }

    /// Opens the runner's own download page through the operating system.
    ///
    /// Honest about what it is: Sindri cannot yet fetch and unpack the runner
    /// itself, because doing so means downloading over TLS and the editor has
    /// no HTTP stack — deliberately, since the engine is meant to stay free of
    /// one. So the platform's browser opens on a pinned page and the probe
    /// keeps watching, which still asks nobody to type anything and still moves
    /// the screen on by itself the moment the install finishes.
    fn get_runner(&mut self, plan: &InstallPlan, context: &egui::Context) {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(opener).arg(plan.source).spawn() {
            Ok(_) => {
                self.assistant.working = Some("Waiting for the runner to appear…".to_owned());
                self.console.info(format!(
                    "Opened {} to install the model runner",
                    plan.source
                ));
            }
            Err(error) => self.report(format!("Could not open {}: {error}", plan.source)),
        }
        context.request_repaint();
    }

    /// Starts the runner the machine already has.
    fn start_runner(&mut self) {
        match std::process::Command::new("ollama").arg("serve").spawn() {
            Ok(_) => {
                self.assistant.working = Some("Starting the runner…".to_owned());
                self.assistant.last_looked = None;
                self.console.info("Starting the local model runner");
            }
            Err(error) => self.report(format!("Could not start the model runner: {error}")),
        }
    }

    /// Downloads a model through the runner that is already answering.
    ///
    /// Over the loopback socket the probe already uses, so this needs no TLS
    /// and no client: the runner does the fetching, and Sindri asks it to.
    fn pull_model(&mut self, profile: &Profile) {
        self.assistant.working = Some(format!("Downloading {}…", profile.display_name));
        self.assistant.last_looked = None;
        self.console.info(format!(
            "Downloading {} at {}",
            profile.id, profile.quantisation
        ));
        let id = profile.id.clone();
        std::thread::spawn(move || {
            let _ = probe::pull(&id);
        });
    }
}

/// Draws the action as a control, returning it when it is chosen.
fn offer(ui: &mut egui::Ui, action: &Action) -> Option<Action> {
    match action {
        Action::Install { plan } => {
            let pressed =
                button::labelled(ui, "Get the model runner", Intent::Primary, plan.performs)
                    .clicked();
            ui.add_space(6.0);
            ui.label(
                RichText::new(plan.performs)
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            );
            pressed.then(|| action.clone())
        }
        Action::Start => button::labelled(
            ui,
            "Start it",
            Intent::Primary,
            "Starts the runner Sindri already found",
        )
        .clicked()
        .then(|| action.clone()),
        Action::Pull { profile } => button::labelled(
            ui,
            &format!("Download {}", profile.display_name),
            Intent::Primary,
            &profile.description,
        )
        .clicked()
        .then(|| action.clone()),
        Action::PickFrom { choices } => {
            let mut chosen = None;
            for (profile, tier) in choices {
                ui.horizontal(|ui| {
                    ui.add_space(metric::GUTTER);
                    if button::labelled(
                        ui,
                        &profile.display_name,
                        Intent::Primary,
                        &profile.description,
                    )
                    .clicked()
                    {
                        chosen = Some(Action::Pull {
                            profile: profile.clone(),
                        });
                    }
                    // How well it would run, said rather than implied by being
                    // present in or absent from the list.
                    ui.label(
                        RichText::new(format!(
                            "{} · {:.1} GB",
                            tier.label(),
                            profile.residency(crate::assistant::DEFAULT_CONTEXT)
                        ))
                        .size(text::NOTE)
                        .color(match tier {
                            Tier::Recommended | Tier::Supported => color::TEXT_FAINT,
                            Tier::BestEffort => color::TEXT_MUTED,
                            Tier::Unsupported => color::WARNING,
                        }),
                    );
                });
                ui.add_space(4.0);
            }
            chosen
        }
        Action::Verify { .. } => button::labelled(
            ui,
            "Check it",
            Intent::Primary,
            "Runs Sindri's own cases against the model",
        )
        .clicked()
        .then(|| action.clone()),
        Action::None => None,
    }
}

/// What a finished setup says: what was proved, not that a socket answered.
fn ready(ui: &mut egui::Ui, model: &str, verified: &[Feature]) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        panel::status_dot(ui, color::SUCCESS);
        ui.label(
            RichText::new(format!("{model} is ready"))
                .size(text::HEADING)
                .color(color::TEXT),
        );
    });
    ui.add_space(8.0);
    for feature in Feature::ALL {
        let proved = verified.contains(&feature);
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER);
            ui.label(
                RichText::new(feature.label())
                    .size(text::LABEL)
                    .color(if proved {
                        color::TEXT_MUTED
                    } else {
                        color::TEXT_FAINT
                    }),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(metric::GUTTER);
                ui.label(
                    RichText::new(if proved { "verified" } else { "unavailable" })
                        .size(text::NOTE)
                        .color(if proved {
                            color::SUCCESS
                        } else {
                            color::TEXT_FAINT
                        }),
                );
            });
        });
    }
}
