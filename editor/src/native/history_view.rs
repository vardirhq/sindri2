//! What Ctrl+Z will do, and everything else it could do after that.
//!
//! The history was answerable one step at a time, from the label on a menu
//! entry nobody opens mid-edit. So "how far back can I go" had no answer, and
//! an edit made twenty steps ago that turned out to be wrong was undone by
//! pressing a key twenty times and watching the viewport to see where you were.
//!
//! The panel is the stack, drawn: every step in the order it happened, the
//! current position marked, and the steps that have been undone still listed
//! below it because they are still reachable. Clicking one travels there.

use eframe::egui::{self, RichText};
use sindri_core::CommandHistory;

use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{panel, tree};

use super::EditorApp;

/// How far to travel through the history: negative undoes, positive redoes.
///
/// One number rather than a target index, because that is what the caller can
/// act on — the history has no "go to step N", and giving it one would mean a
/// second way to move the world.
pub(super) type Travel = isize;

/// The list of steps, and the one that was clicked.
pub(super) fn history_panel(ui: &mut egui::Ui, history: &CommandHistory) -> Option<Travel> {
    let done = history.undo_steps().len();
    let mut travel = None;
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        ui.label(
            RichText::new(format!(
                "{done} step{} back, {} forward",
                if done == 1 { "" } else { "s" },
                history.redo_steps().len()
            ))
            .size(text::LABEL)
            .color(color::TEXT_MUTED),
        );
    });
    panel::rule_tight(ui);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(2.0);
            // The bottom of the undone stack is a state too, and it is the one
            // someone reaching for a history panel most often wants: the scene
            // as it was opened. Without a row for it there is no way to click
            // back past the first edit.
            if step(ui, "Scene opened", Stage::Base, done == 0) {
                travel = Some(-(isize::try_from(done).unwrap_or(isize::MAX)));
            }
            for (index, label) in history.undo_steps().enumerate() {
                let behind = done - index - 1;
                if step(ui, label, Stage::Done, behind == 0) {
                    travel = Some(-(isize::try_from(behind).unwrap_or(isize::MAX)));
                }
            }
            for (index, label) in history.redo_steps().enumerate() {
                if step(ui, label, Stage::Undone, false) {
                    travel = Some(isize::try_from(index + 1).unwrap_or(isize::MAX));
                }
            }
        });
    travel
}

/// Where a step sits relative to where the world is.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Stage {
    /// The state before any edit.
    Base,
    /// Applied, and undoable.
    Done,
    /// Undone, and redoable — still listed, because it is still reachable.
    Undone,
}

/// One step as a row, answering whether it was clicked.
fn step(ui: &mut egui::Ui, label: &str, stage: Stage, current: bool) -> bool {
    let icon = match stage {
        Stage::Base => icons::SCENE,
        Stage::Done => icons::UNDO,
        Stage::Undone => icons::REDO,
    };
    let row = tree::row(
        ui,
        icon,
        label,
        tree::RowStyle {
            selected: current,
            depth: 0,
            children: tree::Children::None,
            // An undone step is listed but is not where the world is, and it
            // has to read as the difference rather than as another step back.
            dimmed: stage == Stage::Undone,
        },
    );
    row.select
        .on_hover_text(if current {
            "Where the scene is now"
        } else {
            "Take the scene back to here"
        })
        .clicked()
        && !current
}

impl EditorApp {
    /// Undoes or redoes until the world is at the step that was clicked.
    ///
    /// Step by step through the same undo and redo the keys use rather than
    /// through a jump of its own: a second way to move the world is a second
    /// thing that can disagree with the first, and each step here is a
    /// transaction that already knows how to reverse itself.
    pub(super) fn travel_history(&mut self, travel: Travel) {
        self.history.break_merge_run();
        for _ in 0..travel.unsigned_abs() {
            let moved = if travel < 0 {
                self.history.undo(&mut self.world)
            } else {
                self.history.redo(&mut self.world)
            };
            match moved {
                // The stack ran out, which means the panel was drawn from a
                // history that has since changed. Stopping is the answer, not
                // reporting: nothing went wrong.
                Ok(None) => break,
                Ok(Some(_)) => {}
                Err(error) => {
                    self.report(error.to_string());
                    break;
                }
            }
        }
        self.selection.retain_live(&self.world);
        self.refresh_textures();
    }
}
