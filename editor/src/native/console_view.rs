//! What the editor has said, and how it says it.
//!
//! Every failure the user should know about goes through `report`, so the
//! notice beside the viewport and the console listing cannot disagree about
//! what happened. The rest is the panel that shows the record.

use eframe::egui::{self, Align, Color32, Layout, RichText};
use sindri_core::{EngineState, EntityId};

use crate::preferences::ConsoleFilter;
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
/// What a frame of the console asked for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ConsoleAction {
    pub(super) cleared: bool,
    /// The entity a row was asked to go to.
    pub(super) go_to: Option<EntityId>,
}

pub(super) fn console_view(
    ui: &mut egui::Ui,
    console: &Console,
    state: EngineState,
    filter: &mut ConsoleFilter,
    named: &dyn Fn(EntityId) -> Option<String>,
) -> ConsoleAction {
    let mut action = ConsoleAction::default();
    let mut cleared = false;
    ui.add_space(4.0);
    // One row when there is width for both, two when there is not. The console
    // is a tall column in one arrangement and a wide dock in the other, and a
    // right-aligned group that wants more width than it has grows leftwards —
    // which is how the engine line came to read "Engine rea".
    let stacked = ui.available_width() < CONTROLS_WIDTH + ENGINE_WIDTH;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        panel::status_dot(ui, color::FORGE);
        ui.label(
            RichText::new(format!("Engine {}", lifecycle_label(state)))
                .size(text::LABEL)
                .color(color::TEXT_MUTED),
        );
        if !stacked {
            console_tools(ui, console, filter, &mut cleared);
        }
    });
    if stacked {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER);
            console_tools(ui, console, filter, &mut cleared);
        });
    }
    panel::rule_tight(ui);
    action.cleared = cleared;
    if console.is_empty() {
        panel::empty_state(
            ui,
            crate::ui::icons::CONSOLE,
            "Nothing to report",
            "Loads, script output, and anything that fails show up here.",
        );
        return action;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        // Pinned to the newest entry: a log you have to scroll to the bottom of
        // to see what just happened is a log nobody reads.
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add_space(2.0);
            let mut shown = 0_usize;
            for entry in console.at_least(filter.floor()) {
                shown += 1;
                if let Some(entity) = console_row(ui, entry, named) {
                    action.go_to = Some(entity);
                }
            }
            // A filter that hides everything has to say that it did, or an
            // empty panel reads as a console that stopped working.
            if shown == 0 {
                ui.add_space(6.0);
                panel::note(ui, "Nothing at this level. The rest is filtered out.");
            }
        });
    action
}

/// How much room the filter and Clear take together.
///
/// A measured constant rather than a guess, for the reason the browser's
/// toolbar has one: the label beside them is given the rest, and getting it
/// wrong is how a header overflows its panel.
const CONTROLS_WIDTH: f32 = 218.0;

/// How much the engine line needs to read as a sentence rather than a stub.
const ENGINE_WIDTH: f32 = 108.0;

/// What the console is filtered to, and the way to empty it.
fn console_tools(
    ui: &mut egui::Ui,
    console: &Console,
    filter: &mut ConsoleFilter,
    cleared: &mut bool,
) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(metric::GUTTER);
        if ui
            .add_enabled_ui(!console.is_empty(), |ui| {
                button::labelled(ui, "Clear", Intent::Quiet, "Empty the console")
            })
            .inner
            .clicked()
        {
            *cleared = true;
        }
        // A console left open for an hour is mostly loads and script output;
        // the line worth reading is the one that went wrong.
        let mut showing = *filter;
        if button::Segmented::new(&mut showing)
            .option(ConsoleFilter::All, "All", "Everything the editor said")
            .option(
                ConsoleFilter::Problems,
                "Problems",
                "Only what went wrong, and what might have",
            )
            .option(ConsoleFilter::Errors, "Errors", "Only what did not happen")
            .show(ui)
        {
            *filter = showing;
        }
    });
}

/// One line, reporting the entity it was asked to go to.
pub(super) fn console_row(
    ui: &mut egui::Ui,
    entry: &Entry,
    named: &dyn Fn(EntityId) -> Option<String>,
) -> Option<EntityId> {
    let tint = level_tint(entry.level);
    let mut go_to = None;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        // Nudged onto the first line's centre: a dot allocated at the top of a
        // wrapping row otherwise floats above the text it belongs to.
        ui.add_space(4.0);
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
        // The entity the line is about, as the way to it. An error naming an
        // entity you cannot reach is a dead end, and the runtime can only name
        // a handle — which is not something anyone can look for in a list.
        if let Some(entity) = entry.subject
            && let Some(name) = named(entity)
            && button::labelled(ui, &name, Intent::Quiet, "Select the entity this is about")
                .clicked()
        {
            go_to = Some(entity);
        }
    });
    go_to
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
