//! Where the editor is looking from, and what moves it.
//!
//! The editor's view of a scene is not the scene's own camera. An orbit, a pan,
//! or a zoom here changes what the Scene view frames and touches nothing the
//! scene authored, which is what lets the Game view keep answering the only
//! question it exists to answer.

use std::f32::consts::TAU;

use eframe::egui::{self, Response};
use glam::{Vec2 as GlamVec2, Vec3};
use sindri_scene::{CameraView, ViewCamera, WorldProjection};

use crate::preferences::CameraProjection;

use super::{EditorApp, WorkspaceTab};

/// How the editor is looking at the scene, as opposed to what the scene says.
///
/// The authored camera lives in the world; this moves around it without
/// touching a single entity.
#[derive(Clone, Copy)]
pub(super) struct EditorCamera {
    pub(super) orbit: GlamVec2,
    pub(super) zoom: f32,
    pub(super) pan: GlamVec2,
    pub(super) projection: CameraProjection,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            orbit: GlamVec2::ZERO,
            zoom: 1.0,
            pan: GlamVec2::ZERO,
            projection: CameraProjection::Perspective,
        }
    }
}

/// The pan that would put `position` in the middle of what `camera` frames.
///
/// The pan's own definition, read backwards: a pan of one moves the picture by
/// exactly the framed half-height, so a subject that far from the middle is
/// exactly one pan away from it. Kept apart from the control that calls it
/// because the way this goes wrong is a sign, and a sign is only visible by
/// asking where the subject ended up.
pub(super) fn pan_to_centre(camera: ViewCamera, pan: GlamVec2, position: Vec3) -> GlamVec2 {
    if camera.framed_half_height <= 0.0 {
        return pan;
    }
    let offset = camera.view.transform_point3(position);
    pan - GlamVec2::new(offset.x, offset.y) / camera.framed_half_height
}

/// How far from the authored camera's own elevation a drag can pitch.
///
/// A little under a right angle either way. The orbit cannot reach the pole
/// whatever this says — the extractor guarantees that, where the authored
/// elevation is known — so this is only about how much drag is worth spending.
const PITCH_LIMIT: f32 = 1.5;

/// How far in and out the wheel can take the scene view.
///
/// The old pair, 0.65 to 1.8, could not frame anything much larger or smaller
/// than the demo cube: not quite twice as far out, and not quite twice as
/// close. A scene is whatever someone builds, so the range is a factor of four
/// hundred and the wheel moves through it proportionally.
pub(super) const MIN_ZOOM: f32 = 0.05;
pub(super) const MAX_ZOOM: f32 = 20.0;

/// The camera a workspace tab looks through.
///
/// The scene view is where the editor moves around; the game view is what the
/// player would see, which means the authored camera and nothing else. If an
/// orbit or a pan leaked into it, it would stop answering the only question it
/// exists to answer.
pub(super) fn camera_for(tab: WorkspaceTab, editor: EditorCamera) -> CameraView {
    match tab {
        WorkspaceTab::Scene => CameraView {
            orbit: editor.orbit,
            distance_scale: 1.0 / editor.zoom,
            pan: editor.pan,
            projection: match editor.projection {
                CameraProjection::Perspective => WorldProjection::Perspective,
                CameraProjection::Orthographic => WorldProjection::Orthographic,
            },
        },
        WorkspaceTab::Game => CameraView::default(),
    }
}

impl EditorApp {
    /// Turns pointer input over the viewport into camera movement.
    ///
    /// Left drag orbits, middle drag or shift-drag pans, and the wheel zooms.
    /// None of it touches the scene: the authored camera stays where it is and
    /// only the view of it moves.
    pub(super) fn move_camera(
        &mut self,
        context: &egui::Context,
        response: &Response,
        height: f32,
        painting: bool,
    ) {
        if response.dragged() {
            let delta = response.drag_motion();
            if response.dragged_by(egui::PointerButton::Middle)
                || context.input(|input| input.modifiers.shift)
            {
                // Panning drags the picture, so it is measured against the
                // height of the viewport: dragging halfway up moves the scene
                // halfway up, at any zoom and under either projection.
                let height = height.max(1.0);
                self.viewport_pan.x += delta.x * 2.0 / height;
                self.viewport_pan.y -= delta.y * 2.0 / height;
            } else if response.dragged_by(egui::PointerButton::Secondary)
                || (!painting && response.dragged_by(egui::PointerButton::Primary))
            {
                self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                // Most of a right angle either way, from wherever the scene
                // authored its camera. The extractor stops the orbit short of
                // the pole itself, because that is where it knows how far the
                // authored camera was already tilted; this only decides how far
                // a drag is worth carrying.
                self.viewport_pitch =
                    (self.viewport_pitch + delta.y * 0.008).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
        }
        if response.hovered() {
            let delta = context.input(|input| input.smooth_scroll_delta.y);
            // Multiplied rather than added: the range spans a factor of four
            // hundred, and a fixed step that moves the picture usefully at one
            // end does nothing at the other. A notch of the wheel is the same
            // proportion of the distance wherever the camera is.
            self.viewport_zoom =
                (self.viewport_zoom * (delta * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
        }
    }

    /// Puts the selected entity in the middle of the scene view.
    ///
    /// Centres rather than fits: fitting needs the bounds of what is selected,
    /// and an entity's bounds are a mesh's business, not a transform's. What
    /// this fixes is the ordinary way a subject is lost — panned off the edge,
    /// or never in frame because the authored camera was aimed elsewhere.
    ///
    /// The arithmetic is the pan's own definition read backwards. A pan of one
    /// moves the picture by exactly the framed half-height, so a subject sitting
    /// that far from the middle is exactly one pan away from it, and the
    /// extractor is asked for both numbers rather than the editor keeping its
    /// own copy of either.
    pub(super) fn focus_selection(&mut self) {
        let Some(position) = self
            .selection
            .and_then(|entity| self.world.get(entity))
            .and_then(|data| data.transform_3d)
            .map(|transform| Vec3::from_array(transform.position))
        else {
            return;
        };
        let Ok(Some(camera)) = self.scene.world_camera(&self.world, self.scene_camera()) else {
            return;
        };
        self.viewport_pan = pan_to_centre(camera, self.viewport_pan, position);
    }

    /// The camera the scene view is looking through.
    pub(super) fn scene_camera(&self) -> CameraView {
        camera_for(
            WorkspaceTab::Scene,
            EditorCamera {
                orbit: GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                zoom: self.viewport_zoom,
                pan: self.viewport_pan,
                projection: self.preferences.projection,
            },
        )
    }

    /// Whether the viewer has moved away from the authored camera.
    pub(super) fn view_moved(&self) -> bool {
        self.viewport_yaw != 0.0
            || self.viewport_pitch != 0.0
            || self.viewport_pan != GlamVec2::ZERO
            || (self.viewport_zoom - 1.0).abs() > f32::EPSILON
    }

    /// Returns to the camera the scene authored, without touching the scene.
    pub(super) fn reset_view(&mut self) {
        self.viewport_yaw = 0.0;
        self.viewport_pitch = 0.0;
        self.viewport_pan = GlamVec2::ZERO;
        self.viewport_zoom = 1.0;
    }
}
