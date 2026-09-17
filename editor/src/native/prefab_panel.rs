//! The prefab the browser is pointing at, and the tool attached to it.
//!
//! Its own panel rather than a text preview, which is what a `.prefab.json` got
//! before: readable, scrollable, and no use for the thing an author actually
//! wants to do with a prefab, which is put one somewhere.

use eframe::egui;

use super::EditorApp;
use crate::ui::icons;
use crate::ui::widgets::{panel, property, section};

impl EditorApp {
    pub(super) fn prefab_panel(&mut self, ui: &mut egui::Ui) {
        let Some(brush) = self.prefab_brush.as_mut() else {
            return;
        };
        section::group(ui, icons::PREFAB, &brush.name());

        if let Some(problem) = brush.problem() {
            panel::problem(ui, problem);
            return;
        }

        let entities = brush.entities();
        panel::note(
            ui,
            &if entities == 1 {
                "One entity.".to_owned()
            } else {
                format!("{entities} entities, as one undoable step.")
            },
        );

        property::toggle(ui, "Place", &mut brush.placing, "On a cell", "Off");
        if brush.placing {
            panel::note(
                ui,
                "Click a cell in the Scene view. The prefab stands on it, and \
                 keeps whatever shape it declares.",
            );
        } else {
            panel::note(ui, "Turn this on to build with it in the Scene view.");
        }
    }
}
