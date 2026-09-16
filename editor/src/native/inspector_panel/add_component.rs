//! The Add Component menu, and the families it groups its offers into.
//!
//! Split from `rows` because it is a different job: `rows` draws a value that
//! already exists, and this offers one that does not yet. They shared a file
//! only because both are drawn by the inspector.

use eframe::egui::{self, RichText};

use crate::components::{self, Family};
use crate::ui::theme::{color, metric, text};

use super::draft::Offer;

/// The Add Component menu, offering only what can actually be added.
///
/// Absent entirely when there is nothing to add, rather than shown disabled: an
/// entity that already has everything is not a state worth drawing a greyed-out
/// control for.
pub(crate) fn add_component_button(ui: &mut egui::Ui, addable: &[Offer]) -> Option<String> {
    if addable.is_empty() {
        return None;
    }
    let mut chosen = None;
    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        // Words rather than a bare "+", because an inspector has several things
        // it could plausibly be adding. Given the panel's width so it reads as
        // the one thing left to do at the bottom of the list. The label is
        // plain text: the bundled Inter subset carries 192 glyphs, and a
        // decorative plus sign outside it draws as a missing-glyph box.
        let width = (ui.available_width() - 2.0 * metric::GUTTER).max(120.0);
        ui.allocate_ui(egui::vec2(width, metric::CONTROL_HEIGHT + 6.0), |ui| {
            ui.menu_button(
                RichText::new("Add Component")
                    .size(text::BODY)
                    .color(color::TEXT),
                |ui| {
                    ui.set_min_width(200.0);
                    chosen = component_families(ui, addable);
                },
            );
        });
    });
    ui.add_space(10.0);
    chosen
}

/// The offers, under the families they belong to.
///
/// A flat list of thirteen is a list you read rather than a menu you use, and
/// it only grows. Grouping is by [`crate::components::family`] rather than by
/// splitting the type name on its dots: the namespace is a naming scheme, not a
/// taxonomy, and splitting it yields two one-entry submenus and five components
/// with no family at all.
///
/// A family holding one offer here is *not* given a submenu — it is listed at
/// the top level with the ungrouped ones. Which family holds one depends on the
/// entity: a UI element is offered no Rendering components at all, and hiding a
/// lone entry behind a heading is a click that buys nothing.
fn component_families(ui: &mut egui::Ui, addable: &[Offer]) -> Option<String> {
    let mut chosen = None;
    let mut loose: Vec<&Offer> = Vec::new();
    for family in Family::ALL {
        let held = held_by(addable, Some(family));
        match held.len() {
            0 => {}
            1 => loose.extend(held),
            _ => {
                ui.menu_button(
                    RichText::new(family.label())
                        .size(text::BODY)
                        .color(color::TEXT),
                    |ui| {
                        ui.set_min_width(200.0);
                        for offer in held {
                            if component_entry(ui, offer) {
                                chosen = Some(offer.metadata.type_name.clone());
                            }
                        }
                    },
                );
            }
        }
    }
    // Anything the table has never heard of, which a scene from a newer tool
    // can carry. Listed rather than dropped, and at the top level rather than
    // under a family invented to hold it.
    loose.extend(held_by(addable, None));
    for offer in loose {
        if component_entry(ui, offer) {
            chosen = Some(offer.metadata.type_name.clone());
        }
    }
    chosen
}

/// The offers in one family, in the order they are listed.
///
/// By display name, because that is what the menu shows: sorted by type name a
/// reader sees Collider 2D under Rigid Body 2D for a reason nothing on screen
/// explains.
fn held_by(addable: &[Offer], family: Option<Family>) -> Vec<&Offer> {
    let mut held: Vec<&Offer> = addable
        .iter()
        .filter(|offer| components::family(&offer.metadata.type_name) == family)
        .collect();
    held.sort_by(|left, right| left.metadata.display_name.cmp(&right.metadata.display_name));
    held
}

/// One offer as an entry, answering whether it was chosen.
///
/// Listed either way. An entry that cannot be used says what to go and make;
/// absent, it said nothing, and the menu was simply shorter than the
/// documentation.
fn component_entry(ui: &mut egui::Ui, offer: &Offer) -> bool {
    let entry = ui.add_enabled(
        offer.withheld.is_none(),
        egui::Button::new(
            RichText::new(&offer.metadata.display_name)
                .size(text::BODY)
                .color(color::TEXT),
        ),
    );
    if let Some(reason) = offer.withheld {
        entry.on_disabled_hover_text(reason);
        return false;
    }
    if entry.clicked() {
        ui.close();
        return true;
    }
    false
}
