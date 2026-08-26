//! Which keystroke means which command.

use eframe::egui::{self};

use super::super::shortcuts::{Shortcuts, pressed};

/// Which shortcuts a key press produces, read through a real egui frame.
fn shortcuts_for(modifiers: egui::Modifiers, key: egui::Key) -> Shortcuts {
    let context = egui::Context::default();
    let read = std::cell::Cell::new(Shortcuts::default());
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    context
        .run_ui(input, |ui| {
            read.set(ui.ctx().input_mut(pressed));
        })
        .drop_without_applying_deltas();
    read.get()
}

/// Redo must be asked for before undo, because egui ignores an extra Shift
/// when matching: Ctrl+Shift+Z tested against Ctrl+Z matches, so the
/// editor's redo shortcut used to be consumed by undo and perform one.
#[test]
fn redo_is_not_swallowed_by_undo() {
    let redo = shortcuts_for(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    assert!(redo.redo, "Ctrl+Shift+Z must redo");
    assert!(!redo.undo, "and must not also undo");

    let undo = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::Z);
    assert!(undo.undo && !undo.redo, "Ctrl+Z is still undo");

    let also_redo = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::Y);
    assert!(
        also_redo.redo && !also_redo.undo,
        "and Ctrl+Y is still redo"
    );

    let save = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::S);
    assert!(save.save && !save.undo && !save.redo);
}
