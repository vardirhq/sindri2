//! Keyboard commands, read once a frame before anything consumes a key.

use eframe::egui::{self};

use super::EditorApp;

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
    pub(super) play: bool,
    pub(super) pause: bool,
    pub(super) duplicate: bool,
    pub(super) rename: bool,
    pub(super) delete: bool,
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
        save: input.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
        duplicate: input.consume_key(egui::Modifiers::COMMAND, egui::Key::D),
        rename: bare(input, egui::Key::F2),
        // Backspace as well as Delete, because the key a Mac keyboard labels
        // "delete" is Backspace.
        delete: bare(input, egui::Key::Delete) || bare(input, egui::Key::Backspace),
        // Unmodified, as it is everywhere else that frames a selection.
        focus: bare(input, egui::Key::F),
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
        if keys.save {
            self.save();
        }
        if authoring {
            if keys.redo {
                self.redo();
            } else if keys.undo {
                self.undo();
            }
            // One entity at a time, and exclusive: duplicating and then
            // deleting in the same frame would act on a selection the first
            // verb had already moved.
            if let Some(entity) = self.selection {
                if keys.duplicate {
                    self.duplicate_entity(entity);
                } else if keys.rename {
                    self.begin_rename(entity);
                } else if keys.delete {
                    self.delete_entity(entity);
                }
            }
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
}
