//! What the editor has said, and how it says it.
//!
//! Every failure the user should know about goes through `report`, so the
//! notice beside the viewport and the console listing cannot disagree about
//! what happened. The rest is the panel that shows the record.

use eframe::egui::{self, Align, Color32, Layout, RichText};
use sindri_core::EngineState;

use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{button, button::Intent, panel};
use crate::{
    console::{Console, Entry, Level},
    scripts::ScriptNote,
    textures::TextureNote,
};

use super::EditorApp;

impl EditorApp {
    /// Says that something the user asked for did not happen.
    ///
    /// The notice is the one line beside the viewport and is replaced by the
    /// next thing that goes wrong; the console keeps it. Every failure goes
    /// through here so the two cannot disagree about what happened.
    pub(super) fn report(&mut self, message: String) {
        self.console.error(&message);
        self.notice = Some(message);
    }

    pub(super) fn record_script_notes(&mut self, notes: Vec<ScriptNote>) {
        for note in notes {
            match note {
                ScriptNote::Loaded(message) | ScriptNote::Reloaded(message) => {
                    self.console.info(message);
                }
                ScriptNote::Failed(message) => self.console.warning(message),
            }
        }
    }

    pub(super) fn record_texture_notes(&mut self, notes: Vec<TextureNote>) {
        for note in notes {
            match note {
                TextureNote::Loaded(message) | TextureNote::Reloaded(message) => {
                    self.console.info(message);
                }
                TextureNote::Failed(message) => self.console.warning(message),
            }
        }
    }
}

/// The colour a line is written in, which is the only thing that distinguishes
/// three kinds of message in a list of forty.
const fn level_tint(level: Level) -> Color32 {
    match level {
        Level::Info => color::TEXT_MUTED,
        Level::Warning => color::WARNING,
        Level::Error => color::DANGER_TEXT,
    }
}

/// What the editor has said, newest at the bottom.
///
/// This used to be three fixed lines, two of them interpolating a real number,
/// which made it a status readout wearing a log's clothes. The engine's state
/// is still worth a line, so it is one — at the top, marked as the standing
/// state rather than something that just happened.
///
/// Returns true when the user asked to clear it.
pub(super) fn console_view(ui: &mut egui::Ui, console: &Console, state: EngineState) -> bool {
    let mut cleared = false;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        panel::status_dot(ui, color::FORGE);
        ui.label(
            RichText::new(format!("Engine {}", lifecycle_label(state)))
                .size(text::LABEL)
                .color(color::TEXT_MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(metric::GUTTER);
            if ui
                .add_enabled_ui(!console.is_empty(), |ui| {
                    button::labelled(ui, "Clear", Intent::Quiet, "Empty the console")
                })
                .inner
                .clicked()
            {
                cleared = true;
            }
        });
    });
    panel::rule_tight(ui);
    if console.is_empty() {
        panel::empty_state(
            ui,
            crate::ui::icons::CONSOLE,
            "Nothing to report",
            "Loads, script output, and anything that fails show up here.",
        );
        return cleared;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        // Pinned to the newest entry: a log you have to scroll to the bottom of
        // to see what just happened is a log nobody reads.
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(2.0);
            for entry in console.entries() {
                console_row(ui, entry);
            }
        });
    cleared
}

pub(super) fn console_row(ui: &mut egui::Ui, entry: &Entry) {
    let tint = level_tint(entry.level);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        ui.add_space(2.0);
        panel::status_dot(ui, tint);
        // Wrapped, not truncated: an asset failure names a path and an
        // operating system error, and a line that runs off the edge of the dock
        // is a line nobody can act on.
        ui.add(
            egui::Label::new(RichText::new(&entry.message).size(text::LABEL).color(tint)).wrap(),
        );
        // A message that repeated sixty times is one line with a count, not
        // sixty lines that scroll the useful one away.
        if entry.count > 1 {
            crate::ui::widgets::toolbar::chip(ui, &format!("x{}", entry.count), color::TEXT_FAINT);
        }
    });
}

pub(super) fn lifecycle_label(state: EngineState) -> &'static str {
    match state {
        EngineState::Created => "created",
        EngineState::Initialized => "ready",
        EngineState::Running => "running",
        EngineState::Paused => "paused",
        EngineState::Stopped => "stopped",
        EngineState::Destroyed => "destroyed",
    }
}
