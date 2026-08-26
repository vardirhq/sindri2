//! The two tool strips over the scene view.

use eframe::egui::{self, Align, Layout, RichText, Stroke};
use egui_material_icons::icons::{
    ICON_CAMERA_ALT, ICON_CENTER_FOCUS_STRONG, ICON_GRID_4X4, ICON_MOVE, ICON_ROTATE_RIGHT,
    ICON_SCALE, ICON_SELECT,
};

use crate::{
    gizmo::{GizmoMode, GizmoSpace},
    // `egui::Layout` is a different thing entirely and is already in scope.
    preferences::CameraProjection,
};

use super::EditorApp;
use super::chrome::projection_button;
use super::theme::{BORDER_SUBTLE, PANEL_RAISED, TEXT_FAINT, TEXT_MUTED, icon_button};

impl EditorApp {
    fn transform_tools(&mut self, ui: &mut egui::Ui) {
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
            self.gizmo_mode = mode;
            self.gizmo_drag = None;
            self.history.break_merge_run();
        }
        for (mode, icon, key) in [
            (GizmoMode::Select, ICON_SELECT, "Q"),
            (GizmoMode::Translate, ICON_MOVE, "W"),
            (GizmoMode::Rotate, ICON_ROTATE_RIGHT, "E"),
            (GizmoMode::Scale, ICON_SCALE, "R"),
        ] {
            if icon_button(
                ui,
                icon,
                self.gizmo_mode == mode,
                &format!("{} ({key})", mode.label()),
            )
            .clicked()
            {
                self.gizmo_mode = mode;
                self.gizmo_drag = None;
                self.history.break_merge_run();
            }
        }
        if ui
            .add_sized(
                [48.0, 28.0],
                egui::Button::new(
                    RichText::new(match self.gizmo_space {
                        GizmoSpace::World => "World",
                        GizmoSpace::Local => "Local",
                    })
                    .size(10.0)
                    .color(TEXT_MUTED),
                )
                .fill(PANEL_RAISED)
                .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .on_hover_text("Toggle world/local movement and rotation axes")
            .clicked()
        {
            self.gizmo_space = match self.gizmo_space {
                GizmoSpace::World => GizmoSpace::Local,
                GizmoSpace::Local => GizmoSpace::World,
            };
            self.gizmo_drag = None;
        }
        let snap_tip = format!(
            "Snap: {} units · {}° · {} scale",
            self.gizmo_snapping.translation,
            self.gizmo_snapping.rotation_degrees,
            self.gizmo_snapping.scale
        );
        if icon_button(ui, ICON_GRID_4X4, self.gizmo_snapping.enabled, &snap_tip).clicked() {
            self.gizmo_snapping.enabled = !self.gizmo_snapping.enabled;
        }
    }

    /// The row of tools above the viewport.
    ///
    /// The game view has none of them: they change what the editor is looking
    /// at, and that view exists to show what the player would see.
    pub(super) fn scene_tools(&mut self, ui: &mut egui::Ui, editing: bool) {
        ui.horizontal(|ui| {
            if !editing {
                // The game view is what the player sees, so the tools
                // for changing what they are looking at do not apply.
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Rendering through the authored camera")
                        .size(11.0)
                        .color(TEXT_FAINT),
                );
                return;
            }
            // The projection pair claims its width first so the icon row
            // shrinks beside it. Laid out the other way round, a narrow
            // viewport drew the icons straight over the buttons.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                projection_button(
                    ui,
                    &mut self.preferences.projection,
                    CameraProjection::Orthographic,
                    "Ortho",
                );
                projection_button(
                    ui,
                    &mut self.preferences.projection,
                    CameraProjection::Perspective,
                    "Perspective",
                );
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(8.0);
                    self.transform_tools(ui);
                    // Panning can carry the subject off screen entirely, so
                    // the way back is a control rather than a remembered
                    // number.
                    if icon_button(ui, ICON_CAMERA_ALT, self.view_moved(), "Reset view").clicked() {
                        self.reset_view();
                    }
                    if ui
                        .add_enabled_ui(self.selection.is_some(), |ui| {
                            icon_button(ui, ICON_CENTER_FOCUS_STRONG, false, "Focus selection (F)")
                        })
                        .inner
                        .clicked()
                    {
                        self.focus_selection();
                    }
                });
            });
        });
    }
}
