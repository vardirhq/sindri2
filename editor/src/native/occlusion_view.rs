//! The ordering sweep, as something an author can see.
//!
//! Two small halves of one feature that would otherwise sit in the middle of
//! the viewport's frame path: run the sweep while authoring, and paint what it
//! found onto the grid it is about.

use eframe::egui::{self, Rect};
use sindri_scene::{CameraView, OcclusionProbe};

use super::EditorApp;
use super::overlay::paint_occlusion_faults;

impl EditorApp {
    /// Re-sweeps the scene when its volumes have changed.
    ///
    /// Only while authoring. The marks describe a scene at rest, and a running
    /// game moves things the sweep does not model, so showing them over a live
    /// game would be describing a world that is no longer there.
    pub(super) fn sweep_occlusion_overlay(&mut self) {
        self.occlusion.ensure(
            &self.world,
            self.scene.components(),
            self.textures.tile_sets(),
            OcclusionProbe::default(),
        );
    }

    /// Paints every fault through the grid that produced it.
    pub(super) fn paint_occlusion_overlay(&self, ui: &egui::Ui, rect: Rect, camera: CameraView) {
        if !self.occlusion.enabled {
            return;
        }
        // The same camera the frame under it was drawn through, resolved the way
        // block picking resolves it, so a mark cannot land somewhere the cell it
        // describes is not.
        let aspect = rect.width() / rect.height().max(1.0);
        let Ok(Some(camera)) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
        else {
            return;
        };
        let marks =
            self.occlusion
                .marks(&self.world, self.scene.components(), camera.view_projection);
        paint_occlusion_faults(ui.painter(), rect, &marks);
    }
}
