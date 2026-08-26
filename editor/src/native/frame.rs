//! One frame of the editor: docks, panels, and the arrangement they take.
//!
//! `eframe` calls `update` once a frame, and everything the editor draws hangs
//! off it. The work each region does lives in the module that owns that
//! region; what is here is only the order and the layout.

use eframe::egui::{self};

use crate::preferences::Layout as WorkspaceLayout;

use super::chrome::workspace_tab;
use super::theme::{APP_BG, view_title};
use super::{EditorApp, WorkspaceTab};

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

impl EditorApp {
    /// The 2 by 3 workspace: Scene above Game, with the panels beside them.
    fn two_by_three_views(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                // The Game view is a bottom panel so it keeps its height while
                // the Scene view takes whatever is left.
                egui::Panel::bottom("game-view")
                    .default_size(300.0)
                    .min_size(120.0)
                    .resizable(true)
                    .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
                    .show(ui, |ui| {
                        view_title(ui, "Game");
                        ui.separator();
                        self.render_view(ui, WorkspaceTab::Game);
                    });
                view_title(ui, "Scene");
                ui.separator();
                self.scene_tools(ui, true);
                ui.separator();
                self.render_view(ui, WorkspaceTab::Scene);
            });
    }

    /// The wide workspace: one view at a time, chosen by a tab.
    fn tabbed_view(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Scene, "Scene");
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Game, "Game");
                });
                ui.separator();
                let tab = self.workspace_tab;
                self.scene_tools(ui, tab == WorkspaceTab::Scene);
                ui.separator();
                // Only the visible view is drawn: rendering the hidden one would
                // spend a frame's GPU work on something nobody is looking at.
                self.render_view(ui, tab);
            });
    }
}
impl eframe::App for EditorApp {
    /// Settings are written when eframe decides to, which includes shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.preferences.save(storage);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Before anything is drawn, so a texture that arrived since the last
        // frame is bound by the time this one extracts.
        self.refresh_textures();
        let state = self.render_state.clone();
        let arrived = self
            .textures
            .poll(&state.device, &state.queue, &mut self.renderers.text);
        self.record_texture_notes(arrived);
        self.advance_animations(ui.ctx());
        self.advance_scripts(ui.ctx());
        self.update_title(ui.ctx());
        self.handle_close_request(ui.ctx());
        self.handle_shortcuts(ui.ctx());
        self.top_bar(ui);
        self.status_bar(ui);
        // Panels claim space in the order they are shown, so this order is the
        // arrangement: each right panel sits to the left of the one before it.
        match self.preferences.layout {
            WorkspaceLayout::TwoByThree => {
                self.inspector_panel(ui);
                self.asset_panel(ui);
                self.hierarchy_panel(ui);
                self.render_error = None;
                self.two_by_three_views(ui);
            }
            WorkspaceLayout::Wide => {
                self.hierarchy_panel(ui);
                self.inspector_panel(ui);
                self.asset_panel(ui);
                self.render_error = None;
                self.tabbed_view(ui);
            }
        }
        // Releasing the pointer ends a drag, so the next one is its own step.
        if ui.ctx().input(|input| input.pointer.any_released()) {
            self.history.break_merge_run();
        }
        // Drawn last so it sits over everything, and asked before Escape is
        // read as clearing the selection.
        if self.confirm_dialog(ui.ctx()) {
            return;
        }
        // Escape clears the selection wherever the pointer happens to be. The
        // hierarchy's empty space does the same, but only while it has empty
        // space to click.
        if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
            self.select(None);
        }
    }
}
