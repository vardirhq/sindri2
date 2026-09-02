//! One frame of the editor: docks, panels, and the arrangement they take.
//!
//! `eframe` calls `update` once a frame, and everything the editor draws hangs
//! off it. The work each region does lives in the module that owns that
//! region; what is here is only the order and the layout.

use eframe::egui::{self};

use crate::preferences::Layout as WorkspaceLayout;
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{
    panel,
    tabs::{self, Weight},
};

use super::chrome::workspace_label;
use super::{EditorApp, WorkspaceTab};

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

/// The icon a workspace is known by, in the tab and anywhere else it is named.
pub(super) const fn workspace_icon(tab: WorkspaceTab) -> egui_material_icons::MaterialIcon {
    match tab {
        WorkspaceTab::Scene => icons::WORLD,
        WorkspaceTab::Game => icons::CAMERA,
    }
}

/// The name strip above a view that is already on screen.
///
/// Drawn as a lit tab rather than as a plain label: in the two-by-three
/// arrangement both views are visible at once, so a control that selected one
/// would do nothing — but it is the same workspace it would be in the tabbed
/// arrangement, and it should be recognisably that.
fn view_banner(ui: &mut egui::Ui, tab: WorkspaceTab) {
    tabs::strip(ui, |ui| {
        tabs::tab(
            ui,
            Weight::Primary,
            true,
            Some(workspace_icon(tab)),
            workspace_label(tab),
        );
        if tab == WorkspaceTab::Game {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(metric::GUTTER);
                ui.label(
                    egui::RichText::new("What the player would see")
                        .size(text::NOTE)
                        .color(color::TEXT_FAINT),
                );
            });
        }
    });
}

impl EditorApp {
    /// The 2 by 3 workspace: Scene above Game, with the panels beside them.
    fn two_by_three_views(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(panel::viewport_frame())
            .show(ui, |ui| {
                // The Game view is a bottom panel so it keeps its height while
                // the Scene view takes whatever is left.
                egui::Panel::bottom("game-view")
                    .default_size(300.0)
                    .min_size(120.0)
                    .resizable(true)
                    .frame(panel::viewport_frame())
                    .show(ui, |ui| {
                        view_banner(ui, WorkspaceTab::Game);
                        self.render_view(ui, WorkspaceTab::Game);
                    });
                view_banner(ui, WorkspaceTab::Scene);
                self.scene_tools(ui, true);
                self.render_view(ui, WorkspaceTab::Scene);
            });
    }

    /// The wide workspace: one view at a time, chosen by a tab.
    fn tabbed_view(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(panel::viewport_frame())
            .show(ui, |ui| {
                let mut chosen = self.workspace_tab;
                tabs::strip(ui, |ui| {
                    for tab in [WorkspaceTab::Scene, WorkspaceTab::Game] {
                        if tabs::tab(
                            ui,
                            Weight::Primary,
                            self.workspace_tab == tab,
                            Some(workspace_icon(tab)),
                            workspace_label(tab),
                        )
                        .clicked()
                        {
                            chosen = tab;
                        }
                    }
                });
                self.workspace_tab = chosen;
                let tab = self.workspace_tab;
                self.scene_tools(ui, tab == WorkspaceTab::Scene);
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
        // Before anything else: the welcome window is a window of its own, and
        // while it is the only one open there is no scene to draw, no viewport
        // to render into, and a hidden window to not spend a frame on.
        if self.welcome.is_some() {
            self.show_welcome(ui.ctx());
            if self.awaiting_welcome() {
                return;
            }
        }
        self.show_window(ui.ctx());
        // Before anything is drawn, so a texture that arrived since the last
        // frame is bound by the time this one extracts.
        self.refresh_textures();
        let state = self.render_state.clone();
        let arrived = self
            .textures
            .poll(&state.device, &state.queue, &mut self.renderers.text);
        self.record_texture_notes(arrived);
        self.advance_play(ui.ctx());
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
        // Drawn last so they sit over everything, and asked before Escape is
        // read as clearing the selection.
        if self.confirm_dialog(ui.ctx()) || self.confirm_delete(ui.ctx()) {
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
