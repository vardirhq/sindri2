//! The four ways to throw unsaved work away, and the question they ask first.
//!
//! Each of these used to happen the moment it was clicked. Routing them all
//! through one confirmation is why `Discarding` exists at all: the dialog has
//! to be able to say which action it is about to carry out.

use std::path::PathBuf;

use eframe::egui::{self, Align, Layout};

use crate::ui::widgets::button::{self, Intent};
use crate::ui::widgets::dialog;

use super::EditorApp;

/// Something the user asked for that would throw unsaved work away.
///
/// Each of these used to happen the moment it was clicked. Two of them are in a
/// menu, one is the window's close button, and one was the Stop button, which
/// reset the scene rather than stopping anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Discarding {
    /// Making a scene replaces the open one, so it costs the same work
    /// opening another does.
    NewScene,
    OpenAnother,
    /// A scene chosen in the project browser, which knows the path already and
    /// so has no dialog to open.
    OpenPath(PathBuf),
    Reload,
    Reset,
    Close,
}

impl Discarding {
    /// What the user is about to lose the work to, in the words of the control
    /// they pressed.
    pub(super) const fn question(&self) -> &'static str {
        match self {
            Self::NewScene => "Make a new scene and discard the changes to this one?",
            Self::OpenAnother | Self::OpenPath(_) => {
                "Open another scene and discard the changes to this one?"
            }
            Self::Reload => "Re-read this scene from disk and discard the changes?",
            Self::Reset => "Discard the changes and go back to the scene as it was saved?",
            Self::Close => "Close the editor and discard the changes?",
        }
    }

    pub(super) const fn verb(&self) -> &'static str {
        match self {
            Self::NewScene => "Make one anyway",
            Self::OpenAnother | Self::OpenPath(_) => "Open anyway",
            Self::Reload => "Reload anyway",
            Self::Reset => "Discard",
            Self::Close => "Close anyway",
        }
    }
}

/// What the confirm dialog came back with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Answer {
    Cancel,
    Discard,
    Save,
}

impl EditorApp {
    /// Whether the world has moved away from what the file holds.
    pub(super) fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    /// Does what was asked, or asks first when it would cost unsaved work.
    pub(super) fn discard_or_confirm(&mut self, action: Discarding, context: &egui::Context) {
        if self.unsaved() {
            self.confirming = Some(action);
        } else {
            self.discard(action, context);
        }
    }

    /// Carries out an action that throws away whatever is unsaved.
    fn discard(&mut self, action: Discarding, context: &egui::Context) {
        self.confirming = None;
        match action {
            Discarding::NewScene => self.new_scene(),
            Discarding::OpenAnother => self.open_scene(),
            Discarding::OpenPath(path) => self.open_path(&path),
            Discarding::Reload => self.reload(),
            Discarding::Reset => self.reset_to_authored(),
            // Agreeing to close is not closing. The request that raised the
            // question was cancelled, so nothing is asking the window to go any
            // more; this asks again, and the flag lets that one through.
            Discarding::Close => {
                self.closing = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Catches the window's close button while there is unsaved work.
    ///
    /// The close is cancelled and the question asked; answering it either lets
    /// the next request through or leaves the editor open. Without this, the
    /// most ordinary way to leave the editor is also the one way to lose an
    /// afternoon without being asked.
    pub(super) fn handle_close_request(&mut self, context: &egui::Context) {
        if !context.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.closing || !self.unsaved() {
            return;
        }
        context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.confirming = Some(Discarding::Close);
    }

    /// Asks before throwing work away, and reports whether it is asking.
    ///
    /// Returns `true` while the question is on screen, so the frame's remaining
    /// input handling stands down rather than acting on keys aimed at the
    /// dialog.
    pub(super) fn confirm_dialog(&mut self, context: &egui::Context) -> bool {
        let Some(action) = self.confirming.clone() else {
            return false;
        };
        let saveable = self.file.path().is_some();
        let mut answered = None;
        dialog::ask(
            context,
            "sindri-discard-confirm",
            "Unsaved changes",
            action.question(),
            |ui| {
                if button::labelled(ui, "Cancel", Intent::Quiet, "Leave everything as it is")
                    .clicked()
                {
                    answered = Some(Answer::Cancel);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // The destructive answer is the one drawn as destructive,
                    // and saving first is the one drawn as the way out.
                    if button::labelled(ui, action.verb(), Intent::Danger, "Lose the changes")
                        .clicked()
                    {
                        answered = Some(Answer::Discard);
                    }
                    if ui
                        .add_enabled_ui(saveable, |ui| {
                            button::labelled(
                                ui,
                                "Save first",
                                Intent::Primary,
                                "Write the scene to disk, then carry on",
                            )
                        })
                        .inner
                        .clicked()
                    {
                        answered = Some(Answer::Save);
                    }
                });
            },
        );
        match answered {
            None => {}
            Some(Answer::Cancel) => self.confirming = None,
            Some(Answer::Discard) => self.discard(action, context),
            Some(Answer::Save) => {
                self.save();
                // A failed save leaves the question standing rather than
                // discarding the work it could not write.
                if !self.unsaved() {
                    self.discard(action, context);
                }
            }
        }
        self.confirming.is_some()
    }
}
