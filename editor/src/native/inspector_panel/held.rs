//! Inspector edits held back until they are due, and the Apply they wait on.
//!
//! [`crate::inspector::held`] decides what is due; this is where the editor
//! asks it, writes what is due through the history, and draws the parts a
//! person sees: the bar on a component with edits waiting for Apply, and
//! the notice over the views while an applied change is being rebuilt.

use std::collections::BTreeSet;
use std::time::Instant;

use eframe::egui;
use sindri_core::EntityId;

use crate::inspector::held::{Asked, Components, HeldEdits, SETTLE_AFTER};
use crate::ui::theme::{color, metric, text};

use super::super::EditorApp;

/// Edits made in the inspector and not yet in the world.
#[derive(Debug, Default)]
pub(in crate::native) struct HeldInspectorEdits {
    /// The stable ID being typed, and whose it is.
    ///
    /// Held across frames because a stable ID is not written as it is typed:
    /// renaming `orb-1` to `player` passes through `p`, `pl`, `pla`, each of
    /// which would be a real identity written into the world and into every
    /// component pointing at it.
    pub(in crate::native) id: Option<(EntityId, String)>,
    /// Field edits whose component says to apply them later.
    held: HeldEdits,
    /// A component whose Apply was pressed, and how far along it is: shown
    /// as rebuilding for a frame before the rebuild, so the notice is on
    /// screen while the editor is busy, and cleared the frame after.
    applying: Option<(EntityId, String, u8)>,
}

/// One frame of the inspector's held edits for the entity it shows.
pub(in crate::native) struct HeldFrame {
    /// What the world holds.
    stored: Components,
    /// What the inspector shows: that, with the held edits over it.
    pub(in crate::native) shown: Components,
    pub(in crate::native) apply: ApplyFrame,
}

/// Which components wait for Apply, and what their bars were asked.
#[derive(Default)]
pub(in crate::native) struct ApplyFrame {
    waiting: BTreeSet<String>,
    apply: Option<String>,
    revert: Option<String>,
}

impl EditorApp {
    /// What the inspector shows for `entity`: the world's components with
    /// the edits held for it laid over them. Settles, first, whatever was
    /// left typed into an entity no longer selected.
    pub(super) fn held_components(&mut self, entity: EntityId, stored: Components) -> HeldFrame {
        let world = &self.world;
        self.edits.held.retain(|entity| world.get(entity).is_some());
        let settled = self.edits.held.settle_others(Some(entity), |other| {
            world.get(other).map(|data| data.components.clone())
        });
        for (other, before, after) in settled {
            self.commit_components(other, &before, &after);
        }
        HeldFrame {
            shown: self.edits.held.shown(entity, &stored),
            stored,
            apply: ApplyFrame {
                waiting: self.edits.held.waiting(entity),
                ..ApplyFrame::default()
            },
        }
    }

    /// Writes what this frame's edits make due, holding the rest.
    pub(super) fn commit_held(
        &mut self,
        context: &egui::Context,
        entity: EntityId,
        frame: HeldFrame,
        edited: &Components,
    ) {
        let HeldFrame {
            stored,
            shown,
            apply: choice,
        } = frame;
        let now = Instant::now();
        if let Some(name) = choice.apply {
            self.edits.applying = Some((entity, name, 0));
        }
        let apply = match &mut self.edits.applying {
            Some((held, name, stage)) if *held == entity => {
                *stage += 1;
                context.request_repaint();
                // Applied on the second frame, once the notice has been drawn.
                (*stage == 2).then(|| name.clone())
            }
            _ => None,
        };
        // Finished, or left behind by a selection that moved on.
        if matches!(&self.edits.applying, Some((held, _, stage)) if *stage > 2 || *held != entity) {
            self.edits.applying = None;
        }
        let pointer_down = context.input(|input| input.pointer.any_down());
        let focused = context.memory(|memory| memory.focused().is_some());
        let settled = self.edits.held.settled(now, pointer_down, focused);
        let target = self.edits.held.sort(
            self.scene.components(),
            entity,
            &stored,
            &shown,
            edited,
            settled,
            Asked {
                apply: apply.as_deref(),
                revert: choice.revert.as_deref(),
            },
            now,
        );
        if self.edits.held.settling() {
            context.request_repaint_after(SETTLE_AFTER);
        }
        self.commit_components(entity, &stored, &target);
    }

    /// The notice over the views while an applied change is rebuilt.
    pub(in crate::native) fn paint_applying(&self, context: &egui::Context) {
        if self.edits.applying.is_none() {
            return;
        }
        let area = context.content_rect();
        egui::Area::new(egui::Id::new("inspector-applying"))
            .order(egui::Order::Foreground)
            .fixed_pos(area.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .interactable(false)
            .show(context, |ui| {
                egui::Frame::new()
                    .fill(color::FLOATING)
                    .stroke(egui::Stroke::new(1.0, color::FORGE))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(18, 12))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Regenerating…")
                                .size(text::HEADING)
                                .color(color::TEXT),
                        );
                    });
            });
    }
}

/// Apply and Revert, in the header of a component with edits waiting for
/// Apply. Laid out right to left, after the header's other actions.
pub(in crate::native) fn apply_actions(ui: &mut egui::Ui, name: &str, frame: &mut ApplyFrame) {
    if !frame.waiting.contains(name) {
        return;
    }
    let small = |label: &str, fill: egui::Color32, ink: egui::Color32| {
        egui::Button::new(egui::RichText::new(label).size(text::NOTE).color(ink))
            .fill(fill)
            .min_size(egui::vec2(0.0, 18.0))
    };
    ui.add_space(metric::GAP);
    if ui
        .add(small("Apply", color::FORGE, color::INK))
        .on_hover_text("Write these changes to the scene; it is rebuilt from them")
        .clicked()
    {
        frame.apply = Some(name.to_owned());
    }
    if ui
        .add(small("Revert", color::RAISED, color::TEXT_MUTED))
        .on_hover_text("Forget these changes and show what the scene holds")
        .clicked()
    {
        frame.revert = Some(name.to_owned());
    }
    ui.label(
        egui::RichText::new("Not applied")
            .size(text::NOTE)
            .color(color::FORGE_BRIGHT),
    );
}
