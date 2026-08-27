//! The menus that hang off a control, and the entries in them.
//!
//! The editor had no context menu at all — not one `context_menu` call — so
//! every action that belongs to *a specific thing* had nowhere to live, and
//! most of them therefore did not exist. Duplicate, rename and delete were
//! missing partly because there was no surface to put them on.
//!
//! What this centralises is the two things a menu has to get right and a call
//! site would get wrong on its own: a width that does not jump between menus,
//! and a destructive entry that reads as destructive rather than as one more
//! neutral line of text.

use eframe::egui::{self, Response, RichText};

use crate::ui::theme::{color, text};

/// How wide a menu opens.
///
/// Fixed rather than fitted, so a menu does not change width with the length of
/// the entity someone happens to have selected.
const MENU_WIDTH: f32 = 200.0;

/// The menu a right-click opens, reporting what was chosen.
///
/// `context` cannot return a value through egui's closure, so the chosen action
/// is written into a slot the caller owns. Every menu in the editor is shaped
/// this way: draw, record what was pressed, act after the panel is finished
/// with the borrow it was drawing from.
pub fn on_right_click(response: &Response, add: impl FnOnce(&mut egui::Ui)) {
    response.context_menu(|ui| {
        ui.set_min_width(MENU_WIDTH);
        add(ui);
    });
}

/// One entry: a verb, and what it does to.
pub fn item(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(egui::Button::new(
        RichText::new(label).size(text::BODY).color(color::TEXT),
    ))
}

/// One entry with the key that also does it.
pub fn item_with_key(ui: &mut egui::Ui, label: &str, key: &str) -> Response {
    ui.add(entry(label, key))
}

/// The same entry, built rather than added.
///
/// Handed back so a caller can offer it disabled: the first row has nowhere to
/// move up to, and saying so before the entry is chosen is better than
/// accepting the choice and doing nothing.
pub fn entry(label: &str, key: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label).size(text::BODY).color(color::TEXT))
        .shortcut_text(RichText::new(key).size(text::NOTE).color(color::TEXT_FAINT))
}

/// One entry that throws something away.
pub fn danger(ui: &mut egui::Ui, label: &str, key: &str) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(text::BODY)
                .color(color::DANGER_TEXT),
        )
        .shortcut_text(RichText::new(key).size(text::NOTE).color(color::TEXT_FAINT)),
    )
}

/// A label naming what the entries below it act on.
pub fn subject(ui: &mut egui::Ui, name: &str) {
    ui.label(
        RichText::new(name)
            .size(text::NOTE)
            .color(color::TEXT_FAINT),
    );
    ui.separator();
}
