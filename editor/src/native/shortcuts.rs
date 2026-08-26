//! Keyboard commands, read once a frame before anything consumes a key.

use eframe::egui::{self};

use super::EditorApp;

/// The editing shortcuts pressed this frame.
///
/// Four bools, which the pedantic lint reads as a struct that should have been
/// an enum. It should not: these are independent, a frame can carry more than
/// one, and each is exactly the yes-or-no its key asks.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Shortcuts {
    pub(super) focus: bool,
    pub(super) undo: bool,
    pub(super) redo: bool,
    pub(super) save: bool,
    pub(super) play: bool,
    pub(super) pause: bool,
}

/// Reads the editing shortcuts, most specific first.
///
/// Order is the whole of it. egui matches modifiers logically, so an extra
/// Shift is ignored and a Ctrl+Shift+Z tested against Ctrl+Z matches it —
/// which meant the editor's redo shortcut was consumed by undo and performed
/// one. Redo is asked first so that it sees its own keys.
pub(super) fn pressed(input: &mut egui::InputState) -> Shortcuts {
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
    Shortcuts {
        redo,
        pause,
        play: input.consume_key(egui::Modifiers::COMMAND, egui::Key::P),
        undo: input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
        save: input.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
        // Unmodified, as it is everywhere else that frames a selection. A text
        // field with focus consumes the key before this sees it, so typing an
        // "f" into a name does not move the camera.
        focus: input.consume_key(egui::Modifiers::NONE, egui::Key::F),
    }
}

impl EditorApp {
    pub(super) fn handle_shortcuts(&mut self, context: &egui::Context) {
        let keys = context.input_mut(pressed);
        if keys.save {
            self.save();
        }
        if keys.redo {
            self.redo();
        } else if keys.undo {
            self.undo();
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
