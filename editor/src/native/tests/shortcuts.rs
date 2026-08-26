//! Which keystroke means which command.

use eframe::egui::{self};

use super::super::shortcuts::{Shortcuts, pressed};

/// Which shortcuts a key press produces, read through a real egui frame.
fn shortcuts_for(modifiers: egui::Modifiers, key: egui::Key) -> Shortcuts {
    shortcuts_while(modifiers, key, false)
}

/// The same, with something already holding the keyboard.
fn shortcuts_while(modifiers: egui::Modifiers, key: egui::Key, typing: bool) -> Shortcuts {
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
            read.set(ui.ctx().input_mut(|input| pressed(input, typing)));
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

/// The three verbs a row's menu offers have keys, and the keys mean the same
/// thing the menu entries say they do.
#[test]
fn the_row_verbs_have_the_keys_their_menu_advertises() {
    let duplicate = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::D);
    assert!(duplicate.duplicate, "Ctrl+D must duplicate");

    let rename = shortcuts_for(egui::Modifiers::NONE, egui::Key::F2);
    assert!(
        rename.rename && !rename.focus,
        "F2 must rename, and only that"
    );

    for key in [egui::Key::Delete, egui::Key::Backspace] {
        assert!(
            shortcuts_for(egui::Modifiers::NONE, key).delete,
            "{key:?} must delete"
        );
    }
}

/// A field with the keyboard keeps the unmodified keys.
///
/// The reason this is asserted rather than assumed: renaming is a text field
/// inside the hierarchy, and the shortcuts are read before the panels draw. If
/// the bare keys were taken anyway, typing an "f" into a name would frame the
/// camera and Backspace would delete the entity being renamed instead of a
/// letter of its name.
#[test]
fn typing_keeps_the_unmodified_keys() {
    for key in [
        egui::Key::F,
        egui::Key::F2,
        egui::Key::Delete,
        egui::Key::Backspace,
    ] {
        let keys = shortcuts_while(egui::Modifiers::NONE, key, true);
        assert_eq!(
            keys,
            Shortcuts::default(),
            "{key:?} belongs to the field being typed into"
        );
    }
    // A modified key is still the editor's: Ctrl+S saves from inside a name.
    assert!(shortcuts_while(egui::Modifiers::COMMAND, egui::Key::S, true).save);
}

/// Save As must be asked for before Save, for the same reason redo is asked
/// before undo: egui ignores an extra Shift when matching, so Ctrl+Shift+S
/// tested against Ctrl+S matches it — and the shortcut for "fork this scene"
/// would silently save over the file it was trying to fork.
#[test]
fn save_as_is_not_swallowed_by_save() {
    let save_as = shortcuts_for(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::S,
    );
    assert!(save_as.save_as, "Ctrl+Shift+S must save as");
    assert!(!save_as.save, "and must not also save");

    let save = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::S);
    assert!(save.save && !save.save_as, "Ctrl+S is still save");

    let new_scene = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::N);
    assert!(new_scene.new_scene);
}
