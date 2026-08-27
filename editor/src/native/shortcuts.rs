//! Keyboard commands, read once a frame before anything consumes a key.

use eframe::egui::{self};

use super::unsaved::Discarding;
use super::{EditorApp, Focus};

/// The editing shortcuts pressed this frame.
///
/// A row of bools, which the pedantic lint reads as a struct that should have
/// been an enum. It should not: these are independent, a frame can carry more
/// than one, and each is exactly the yes-or-no its key asks.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Shortcuts {
    pub(super) focus: bool,
    pub(super) undo: bool,
    pub(super) redo: bool,
    pub(super) save: bool,
    pub(super) save_as: bool,
    pub(super) new_scene: bool,
    pub(super) play: bool,
    pub(super) pause: bool,
    pub(super) duplicate: bool,
    pub(super) rename: bool,
    pub(super) delete: bool,
    /// Which way a row was asked to move among its siblings, if it was.
    ///
    /// Not a bool, because the two are one question with two answers and a
    /// frame cannot carry both.
    pub(super) move_by: Option<isize>,
}

/// Reads the editing shortcuts, most specific first.
///
/// Order is the whole of it. egui matches modifiers logically, so an extra
/// Shift is ignored and a Ctrl+Shift+Z tested against Ctrl+Z matches it —
/// which meant the editor's redo shortcut was consumed by undo and performed
/// one. Redo is asked first so that it sees its own keys.
///
/// `typing` is whether something already has the keyboard. The unmodified keys
/// are only read when nothing does: renaming an entity to "Fence" must not
/// frame the camera on the "f", and Backspace inside a name must take back a
/// letter rather than the entity being named.
pub(super) fn pressed(input: &mut egui::InputState, typing: bool) -> Shortcuts {
    let redo = input.consume_key(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
    // Pause before play, for the same reason redo comes before undo: an extra
    // Shift is ignored by a logical match, so Ctrl+Shift+P tested against
    // Ctrl+P would play instead of pausing.
    let pause = input.consume_key(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::P,
    );
    // And Save As before Save, for the third time: Ctrl+Shift+S tested against
    // Ctrl+S matches, so asked the other way round a Save As would silently
    // save over the file it was trying to fork.
    let save_as = input.consume_key(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::S,
    );
    // Not consumed at all while typing, so the key reaches the field that is
    // being typed into instead of being eaten here and doing nothing.
    let bare = |input: &mut egui::InputState, key| {
        !typing && input.consume_key(egui::Modifiers::NONE, key)
    };
    Shortcuts {
        redo,
        pause,
        play: input.consume_key(egui::Modifiers::COMMAND, egui::Key::P),
        undo: input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
        save_as,
        save: input.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
        new_scene: input.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
        duplicate: input.consume_key(egui::Modifiers::COMMAND, egui::Key::D),
        rename: bare(input, egui::Key::F2),
        // Backspace as well as Delete, because the key a Mac keyboard labels
        // "delete" is Backspace.
        delete: bare(input, egui::Key::Delete) || bare(input, egui::Key::Backspace),
        // Unmodified, as it is everywhere else that frames a selection.
        focus: bare(input, egui::Key::F),
        // Alt rather than bare arrows: the arrows belong to whatever has the
        // keyboard, and a scene view will want them.
        move_by: if input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp) {
            Some(-1)
        } else if input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown) {
            Some(1)
        } else {
            None
        },
    }
}

impl EditorApp {
    pub(super) fn handle_shortcuts(&mut self, context: &egui::Context) {
        let typing = context.egui_wants_keyboard_input();
        let keys = context.input_mut(|input| pressed(input, typing));
        // Read whatever the transport is doing, so a key is consumed rather
        // than falling through to something else, and then acted on only where
        // acting is allowed. Save says why it refused; undo and redo do not,
        // because a running scene is not a thing they have anything to say
        // about.
        let authoring = self.authoring_enabled();
        if keys.save_as {
            self.save_as();
        } else if keys.save {
            self.save();
        }
        if keys.new_scene && authoring {
            self.discard_or_confirm(Discarding::NewScene, context);
        }
        if authoring {
            if keys.redo {
                self.redo();
            } else if keys.undo {
                self.undo();
            }
            self.act_on_selection(keys);
        }
        if keys.focus {
            self.focus_selection();
        }
        if keys.pause {
            self.toggle_pause();
        } else if keys.play {
            self.toggle_play_mode();
        }
    }

    /// Duplicate, rename and delete, on whichever selection the keys mean.
    ///
    /// The editor holds two — an entity and an asset — and these three verbs
    /// exist for both. Which one a key acts on is decided by what was chosen
    /// last, so a project row's menu can honestly print the keys beside its
    /// entries. Exclusive, and one thing at a time: duplicating and then
    /// deleting in the same frame would act on a selection the first verb had
    /// already moved.
    fn act_on_selection(&mut self, keys: Shortcuts) {
        match self.focus {
            Focus::Hierarchy => {
                let Some(entity) = self.selection.primary() else {
                    return;
                };
                // Duplicate and delete take the whole selection; rename takes
                // the primary, because five rows cannot become one field and
                // renaming five things to the same name is not a verb.
                if keys.duplicate {
                    self.duplicate_selection();
                } else if keys.rename {
                    self.begin_rename(entity);
                } else if keys.delete {
                    self.delete_selection();
                } else if let Some(offset) = keys.move_by {
                    // The primary, like rename: moving five rows one place at
                    // once has no single answer.
                    self.move_among_siblings(entity, offset);
                }
            }
            Focus::Project => {
                let Some(path) = self.browser.selected.clone() else {
                    return;
                };
                if keys.duplicate {
                    self.duplicate_asset(&path);
                } else if keys.rename {
                    self.begin_asset_rename(&path);
                } else if keys.delete {
                    // Asked rather than done: a disk write has no undo.
                    self.deleting = Some(path);
                }
            }
        }
    }
}
