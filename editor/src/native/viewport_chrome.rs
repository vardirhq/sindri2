//! Viewport UI helpers kept separate from GPU rendering and interaction.

use eframe::egui::{self, Rect};
use sindri_scene::{CameraView, UiCanvas};

use super::EditorApp;

impl EditorApp {
    /// Where this view puts the UI.
    ///
    /// The Scene view puts it in the world, where panning and zooming reach it.
    /// The Game view *is* the screen, so there the overlay is the viewport and
    /// no camera can move it — which is what makes a HUD a HUD.
    fn canvas_for(&self, editing: bool) -> UiCanvas {
        if editing {
            UiCanvas::InScene {
                aspect: self.canvas_aspect(),
            }
        } else {
            UiCanvas::OnViewport
        }
    }

    /// Draws the box the selected text element is laid out in.
    ///
    /// The rect the words actually occupy, which is the authored bounds where
    /// there are any and what the string came out as where there are not. It is
    /// the one way to see what a wrap width does without retyping it and
    /// looking.
    fn paint_text_rect(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        camera: CameraView,
        centre: [f32; 2],
        size: [f32; 2],
    ) {
        let aspect = rect.width() / rect.height().max(1.0);
        let Ok(Some(world)) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
        else {
            return;
        };
        let painter = ui.painter();
        let stroke = egui::Stroke::new(1.0, crate::ui::theme::color::FORGE_DIM);
        for [start, end] in
            super::camera::canvas_rect_outline(rect, world.view_projection, centre, size)
        {
            painter.line_segment([start, end], stroke);
        }
    }

    /// Draws the edge of the screen the UI is laid out on.
    ///
    /// Only in the Scene view, and only because the canvas is in the scene
    /// there: in the Game view the canvas *is* the viewport and its edge is the
    /// viewport's border, which is already drawn.
    fn paint_canvas_outline(&self, ui: &egui::Ui, rect: Rect, camera: CameraView) {
        let aspect = rect.width() / rect.height().max(1.0);
        let Ok(Some(world)) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
        else {
            return;
        };
        let painter = ui.painter();
        let stroke = egui::Stroke::new(1.0, crate::ui::theme::color::LINE);
        for [start, end] in
            super::camera::canvas_outline(rect, world.view_projection, self.canvas_aspect())
        {
            painter.line_segment([start, end], stroke);
        }
    }
}
