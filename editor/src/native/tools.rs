//! The tool strip over the scene view.
//!
//! Nine controls used to sit here in one undifferentiated row of identical
//! boxes, which says all nine are the same kind of thing. They are not: four
//! choose a manipulator, one chooses the axes it works in, one turns snapping
//! on, two move the scene camera, and one chooses the projection. Grouping is
//! the whole design — a reader should be able to find the rotate tool without
//! reading a tooltip.

use eframe::egui;

use crate::gizmo::{GizmoMode, GizmoSpace};
use crate::ui::icons;
use crate::ui::theme::{color, metric};
use crate::ui::widgets::{button, toolbar};

use super::EditorApp;
use super::chrome::projection_choice;

impl EditorApp {
    /// The four manipulators, and the keys that also choose them.
    fn manipulators(&mut self, ui: &mut egui::Ui) {
        let shortcut_mode = ui.input_mut(|input| {
            [
                (egui::Key::Q, GizmoMode::Select),
                (egui::Key::W, GizmoMode::Translate),
                (egui::Key::E, GizmoMode::Rotate),
                (egui::Key::R, GizmoMode::Scale),
            ]
            .into_iter()
            .find_map(|(key, mode)| {
                input
                    .consume_key(egui::Modifiers::NONE, key)
                    .then_some(mode)
            })
        });
        if let Some(mode) = shortcut_mode {
            self.choose_mode(mode);
        }
        toolbar::group(ui, |ui| {
            for (mode, icon, key) in [
                (GizmoMode::Select, icons::SELECT, "Q"),
                (GizmoMode::Translate, icons::TRANSLATE, "W"),
                (GizmoMode::Rotate, icons::ROTATE, "E"),
                (GizmoMode::Scale, icons::SCALE, "R"),
            ] {
                if button::icon(
                    ui,
                    icon,
                    self.gizmo_mode == mode,
                    &format!("{} ({key})", mode.label()),
                )
                .clicked()
                {
                    self.choose_mode(mode);
                }
            }
        });
    }

    /// Chooses a manipulator, ending whatever drag the last one was in.
    fn choose_mode(&mut self, mode: GizmoMode) {
        self.gizmo_mode = mode;
        self.gizmo_drag = None;
        self.history.break_merge_run();
    }

    /// Which axes the manipulators work in, and whether they snap.
    fn manipulator_options(&mut self, ui: &mut egui::Ui) {
        let mut space = self.gizmo_space;
        if button::Segmented::new(&mut space)
            .option(
                GizmoSpace::Local,
                "Local",
                "Move and rotate along the object's own axes",
            )
            .option(
                GizmoSpace::World,
                "World",
                "Move and rotate along the world's axes",
            )
            .show(ui)
        {
            self.gizmo_space = space;
            self.gizmo_drag = None;
        }
        let snap_tip = format!(
            "Snap to {} units · {}° · {} scale",
            self.gizmo_snapping.translation,
            self.gizmo_snapping.rotation_degrees,
            self.gizmo_snapping.scale
        );
        if button::icon(ui, icons::SNAP, self.gizmo_snapping.enabled, &snap_tip).clicked() {
            self.gizmo_snapping.enabled = !self.gizmo_snapping.enabled;
        }
    }

    /// Where the scene camera is looking, and the two ways to send it back.
    fn camera_controls(&mut self, ui: &mut egui::Ui) {
        toolbar::group(ui, |ui| {
            if button::icon(
                ui,
                icons::RESET_VIEW,
                self.view_moved(),
                "Put the scene camera back where it started",
            )
            .clicked()
            {
                self.reset_view();
            }
            let focusable = self.selection.is_some();
            if ui
                .add_enabled_ui(focusable, |ui| {
                    button::icon(
                        ui,
                        icons::FOCUS,
                        false,
                        if focusable {
                            "Frame the selection (F)"
                        } else {
                            "Select something to frame it"
                        },
                    )
                })
                .inner
                .clicked()
            {
                self.focus_selection();
            }
        });
    }

    /// The row of tools above the viewport.
    ///
    /// The game view has none of them: they change what the editor is looking
    /// at, and that view exists to show what the player would see.
    pub(super) fn scene_tools(&mut self, ui: &mut egui::Ui, editing: bool) {
        toolbar::strip(ui, metric::TOOLBAR_HEIGHT, |ui| {
            if !editing {
                toolbar::readout(ui, "camera", "Authored", true);
                ui.add_space(metric::GROUP_GAP);
                ui.label(
                    egui::RichText::new("Scene tools do not apply to the game view")
                        .size(crate::ui::theme::text::NOTE)
                        .color(color::TEXT_FAINT),
                );
                return;
            }
            // One ordered strip, left to right: what the pointer does, then
            // which axes it does it in, then where the scene camera is looking.
            // The projection pair used to claim its width from the right, which
            // meant a narrow viewport clipped the tools rather than the choice
            // nobody was reaching for.
            self.manipulators(ui);
            self.manipulator_options(ui);
            toolbar::divider(ui);
            self.camera_controls(ui);
            toolbar::divider(ui);
            projection_choice(ui, &mut self.preferences.projection);
        });
    }
}
