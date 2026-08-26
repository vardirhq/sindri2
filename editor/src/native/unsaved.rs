//! The four ways to throw unsaved work away, and the question they ask first.
//!
//! Each of these used to happen the moment it was clicked. Routing them all
//! through one confirmation is why `Discarding` exists at all: the dialog has
//! to be able to say which action it is about to carry out.

use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText};

use super::EditorApp;
use super::theme::{ACCENT_BRIGHT, TEXT, TEXT_MUTED};

/// Something the user asked for that would throw unsaved work away.
///
/// Each of these used to happen the moment it was clicked. Two of them are in a
/// menu, one is the window's close button, and one was the Stop button, which
/// reset the scene rather than stopping anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Discarding {
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
        egui::Modal::new(egui::Id::new("sindri-discard-confirm")).show(context, |ui| {
            ui.set_width(360.0);
            ui.label(
                RichText::new("Unsaved changes")
                    .strong()
                    .size(13.0)
                    .color(TEXT),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(action.question())
                    .size(12.0)
                    .color(TEXT_MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    answered = Some(Answer::Cancel);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(action.verb()).color(ACCENT_BRIGHT))
                        .clicked()
                    {
                        answered = Some(Answer::Discard);
                    }
                    if ui
                        .add_enabled(saveable, egui::Button::new("Save first"))
                        .clicked()
                    {
                        answered = Some(Answer::Save);
                    }
                });
            });
        });
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
