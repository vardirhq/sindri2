//! One frame of the editor: what happens, and in what order.
//!
//! `eframe` calls `update` once a frame, and everything the editor draws hangs
//! off it. The work each region does lives in the module that owns that region;
//! the arrangement those regions are drawn in is no longer here at all — it is
//! the [`Workspace`](crate::dock::Workspace) the user dragged into shape, and
//! `workspace.rs` walks it.

use eframe::egui;

use super::EditorApp;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

impl EditorApp {
    /// Picks up saved Weave changes before either viewport resolves presentation.
    ///
    /// `ProjectStyles` throttles the file-system poll itself. Keeping the call
    /// here makes hot reload part of the editor frame rather than part of a
    /// particular panel, so it works whether the stylesheet is being edited in
    /// another application or merely sitting in the project browser.
    fn refresh_styles(&mut self) {
        match self.styles.poll_reload() {
            Ok(true) => self.console.info("Reloaded Weave styles"),
            Ok(false) => {}
            Err(error) => {
                let message = format!("Weave reload: {error}");
                self.console.error(&message);
                self.notice = Some(message);
            }
        }
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
        // Presentation and textures both refresh before extraction, so a saved
        // asset change is visible in the frame that first notices it.
        self.refresh_styles();
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
        self.render_error = None;
        // Order is the arrangement. Docked furniture claims its rows before the
        // workspace divides what is left, so the bars go first. Floating
        // furniture is drawn over a scene that has already taken the whole
        // window, so the workspace goes first — and the centre's tabs, which
        // the bar draws in that mode, need the centre's drop zone to exist
        // before they can point it at themselves.
        if self.preferences.workspace.chrome().floats() {
            self.workspace(ui);
            self.top_bar(ui);
            self.status_bar(ui);
        } else {
            self.top_bar(ui);
            self.status_bar(ui);
            self.workspace(ui);
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
