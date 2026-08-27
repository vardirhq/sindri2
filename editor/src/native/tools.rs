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
use crate::ui::widgets::{button, menu, property, toolbar};

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
        self.snap_control(ui);
    }

    /// The snap toggle, and the increments it snaps to.
    ///
    /// The increments were constants: the tooltip named three numbers and there
    /// was nowhere to set any of them, so a board laid out on quarter units was
    /// a board laid out by hand. They live behind a right-click on the button
    /// that turns snapping on, because that is the control they belong to and a
    /// toolbar has no room for three number fields nobody usually touches.
    fn snap_control(&mut self, ui: &mut egui::Ui) {
        let snapping = self.preferences.snapping;
        let tip = format!(
            "Snap to {} units · {}° · {} scale\nRight-click to change the increments",
            snapping.translation, snapping.rotation_degrees, snapping.scale
        );
        let snap = button::icon(ui, icons::SNAP, snapping.enabled, &tip);
        if snap.clicked() {
            self.preferences.snapping.enabled = !snapping.enabled;
        }
        menu::on_right_click(&snap, |ui| {
            menu::subject(ui, "Snap to");
            // A step of zero is a real answer — "do not round this one" — so
            // the floor is zero rather than something arbitrarily small.
            snap_step(ui, "Units", &mut self.preferences.snapping.translation);
            snap_step(
                ui,
                "Degrees",
                &mut self.preferences.snapping.rotation_degrees,
            );
            snap_step(ui, "Scale", &mut self.preferences.snapping.scale);
        });
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

/// One increment, as a row in the snap menu.
///
/// Zero is allowed and means this one does not round: snapping a position while
/// leaving rotation free is an ordinary way to work, and it is what the gizmo
/// already did with a zero step.
fn snap_step(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    property::Property::new(label).show(ui, |ui| {
        ui.add(
            egui::DragValue::new(value)
                .speed(0.05)
                .range(0.0..=360.0)
                .max_decimals(3),
        );
    });
}
