use glam::{Mat4, Vec2, Vec3};
use sindri_render::{look_at, orthographic_projection, perspective_projection};
use sindri_scene::ViewCamera;

const DEFAULT_POSITION: Vec3 = Vec3::new(3.0, 2.0, 4.0);
const DEFAULT_VERTICAL_FOV_RADIANS: f32 = 45.0_f32.to_radians();
const DEFAULT_ORTHOGRAPHIC_SIZE: f32 = 6.0;
const DEFAULT_NEAR: f32 = 0.1;
const DEFAULT_FAR: f32 = 1_000.0;
const MIN_FOCUS_DISTANCE: f32 = 0.01;
const MAX_FOCUS_DISTANCE: f32 = 100_000.0;
const PITCH_LIMIT: f32 = 1.553_343;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SceneProjection {
    #[default]
    Perspective,
    Orthographic,
}

/// Editor-only camera used by the Scene view.
///
/// This is deliberately not a scene component. It exists even when the open
/// scene has no authored camera, never serializes, and never changes what the
/// Game view renders. The editor observes the world through it rather than
/// borrowing an authored gameplay camera and applying offsets to that camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Distance from the eye to the point orbit, pan, and focus operate around.
    ///
    /// Keeping this explicit is what makes orbit a spatial operation instead of
    /// merely turning in place, and makes wheel zoom proportional at every
    /// scale rather than a fixed one-world-unit shove.
    pub focus_distance: f32,
    pub projection: SceneProjection,
    pub vertical_fov_radians: f32,
    pub orthographic_size: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for SceneCamera {
    fn default() -> Self {
        let focus_distance = DEFAULT_POSITION.length();
        let forward = -DEFAULT_POSITION / focus_distance;
        let pitch = forward.y.asin();
        let yaw = forward.x.atan2(-forward.z);
        Self {
            position: DEFAULT_POSITION,
            yaw,
            pitch,
            focus_distance,
            projection: SceneProjection::Perspective,
            vertical_fov_radians: DEFAULT_VERTICAL_FOV_RADIANS,
            orthographic_size: DEFAULT_ORTHOGRAPHIC_SIZE,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
        }
    }
}

impl SceneCamera {
    /// Rebuilds the Scene camera from the editor's persisted viewport controls.
    ///
    /// The old editor stored orbit, zoom, and pan as adjustments rather than an
    /// actual camera. Keeping the conversion here lets the native viewport switch
    /// to an independent camera without duplicating signs or projection maths in
    /// rendering, picking, gizmos, and tile painting.
    #[must_use]
    pub fn from_view_adjustment(
        orbit: Vec2,
        zoom: f32,
        pan: Vec2,
        projection: SceneProjection,
    ) -> Self {
        let mut camera = Self::default();
        camera.orbit_delta(orbit);

        // The existing editor value grows when zooming in, while `zoom` takes a
        // distance multiplier: larger editor zoom therefore means a smaller
        // eye-to-focus distance.
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        camera.zoom(1.0 / zoom);

        // Pan is stored in framed-half-height units and describes moving the
        // picture, while `SceneCamera::pan` moves the camera. The signs are
        // therefore opposite. Resolve this once here rather than at every
        // screen-to-world call site.
        let half_height = camera.framed_half_height();
        camera.pan(-pan, half_height);

        if projection == SceneProjection::Orthographic {
            // Projection toggles should not resize the subject. Match the
            // perspective camera's current framed height before switching.
            camera.orthographic_size = half_height * 2.0;
            camera.projection = SceneProjection::Orthographic;
        }
        camera
    }

    #[must_use]
    pub fn forward(self) -> Vec3 {
        let pitch = self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        Vec3::new(
            self.yaw.sin() * pitch.cos(),
            pitch.sin(),
            -self.yaw.cos() * pitch.cos(),
        )
        .normalize_or_zero()
    }

    #[must_use]
    pub fn right(self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    #[must_use]
    pub fn up(self) -> Vec3 {
        self.right().cross(self.forward()).normalize_or_zero()
    }

    #[must_use]
    pub fn focus_point(self) -> Vec3 {
        self.position + self.forward() * self.focus_distance
    }

    #[must_use]
    pub fn view(self) -> Mat4 {
        look_at(self.position, self.position + self.forward(), self.up())
    }

    #[must_use]
    pub fn projection_matrix(self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(f32::EPSILON);
        let near = self.near.max(f32::EPSILON);
        let far = self.far.max(near + f32::EPSILON);
        match self.projection {
            SceneProjection::Perspective => perspective_projection(
                self.vertical_fov_radians.max(f32::EPSILON),
                aspect,
                near,
                far,
            ),
            SceneProjection::Orthographic => {
                let half_height = self.orthographic_size.max(f32::EPSILON) * 0.5;
                let half_width = half_height * aspect;
                orthographic_projection(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    near,
                    far,
                )
            }
        }
    }

    #[must_use]
    pub fn view_projection(self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view()
    }

    #[must_use]
    pub fn framed_half_height(self) -> f32 {
        match self.projection {
            SceneProjection::Perspective => {
                self.focus_distance.max(f32::EPSILON) * (self.vertical_fov_radians * 0.5).tan()
            }
            SceneProjection::Orthographic => self.orthographic_size.max(f32::EPSILON) * 0.5,
        }
    }

    /// The fully resolved camera shared by Scene-view drawing, picking, gizmos,
    /// tile painting, and editor chrome.
    #[must_use]
    pub fn resolved(self, aspect: f32) -> ViewCamera {
        let view = self.view();
        ViewCamera {
            view,
            view_projection: self.projection_matrix(aspect) * view,
            framed_half_height: self.framed_half_height(),
        }
    }

    /// Orbits around the current focus point without changing the distance to it.
    pub fn orbit_delta(&mut self, delta: Vec2) {
        let focus = self.focus_point();
        self.yaw += delta.x;
        self.pitch = (self.pitch + delta.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        self.position = focus - self.forward() * self.focus_distance;
    }

    /// Moves the camera and its focus point together in the current image plane.
    pub fn pan(&mut self, delta: Vec2, world_units_per_point: f32) {
        let shift = self.right() * delta.x * world_units_per_point
            + self.up() * delta.y * world_units_per_point;
        self.position += shift;
    }

    /// Dollies proportionally toward/away from the focus point. Orthographic
    /// views change their visible extent by the same factor instead.
    pub fn zoom(&mut self, factor: f32) {
        let factor = factor.max(0.01);
        match self.projection {
            SceneProjection::Perspective => {
                let focus = self.focus_point();
                self.focus_distance =
                    (self.focus_distance * factor).clamp(MIN_FOCUS_DISTANCE, MAX_FOCUS_DISTANCE);
                self.position = focus - self.forward() * self.focus_distance;
            }
            SceneProjection::Orthographic => {
                self.orthographic_size =
                    (self.orthographic_size * factor).clamp(MIN_FOCUS_DISTANCE, MAX_FOCUS_DISTANCE);
            }
        }
    }

    /// Centres the Scene view on `point` while keeping the current orientation.
    pub fn focus(&mut self, point: Vec3, distance: f32) {
        self.focus_distance = distance.clamp(MIN_FOCUS_DISTANCE, MAX_FOCUS_DISTANCE);
        self.position = point - self.forward() * self.focus_distance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_position_does_not_change_orientation() {
        let camera = SceneCamera::default();
        let forward = camera.forward();
        let mut moved = camera;
        moved.position.x += 20.0;
        assert!((moved.forward() - forward).length() < 1.0e-6);
    }

    #[test]
    fn orbit_keeps_the_same_focus_point_and_distance() {
        let mut camera = SceneCamera::default();
        let focus = camera.focus_point();
        let distance = camera.focus_distance;
        camera.orbit_delta(Vec2::new(0.6, -0.25));
        assert!((camera.focus_point() - focus).length() < 1.0e-5);
        assert!((camera.focus_distance - distance).abs() < f32::EPSILON);
    }

    #[test]
    fn focus_keeps_orientation_and_places_target_in_front() {
        let mut camera = SceneCamera::default();
        let forward = camera.forward();
        let target = Vec3::new(10.0, -3.0, 8.0);
        camera.focus(target, 7.0);
        assert!((camera.forward() - forward).length() < 1.0e-6);
        assert!((camera.focus_point() - target).length() < 1.0e-5);
        assert!((camera.focus_distance - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn perspective_zoom_preserves_the_focus_point_and_scales_distance() {
        let mut camera = SceneCamera::default();
        let focus = camera.focus_point();
        let before = camera.focus_distance;
        camera.zoom(0.5);
        assert!((camera.focus_point() - focus).length() < 1.0e-5);
        assert!((camera.focus_distance - before * 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn panning_moves_the_focus_point_with_the_camera() {
        let mut camera = SceneCamera::default();
        let before_eye = camera.position;
        let before_focus = camera.focus_point();
        camera.pan(Vec2::new(3.0, -2.0), 0.25);
        let eye_shift = camera.position - before_eye;
        let focus_shift = camera.focus_point() - before_focus;
        assert!((eye_shift - focus_shift).length() < 1.0e-5);
        assert!(eye_shift.length() > 0.0);
    }

    #[test]
    fn view_adjustment_keeps_projection_toggle_at_the_same_scale() {
        let orbit = Vec2::new(0.4, -0.2);
        let pan = Vec2::new(0.25, -0.5);
        let perspective =
            SceneCamera::from_view_adjustment(orbit, 1.7, pan, SceneProjection::Perspective);
        let orthographic =
            SceneCamera::from_view_adjustment(orbit, 1.7, pan, SceneProjection::Orthographic);

        assert!(
            (perspective.framed_half_height() - orthographic.framed_half_height()).abs() < 1.0e-6
        );
        assert!((perspective.forward() - orthographic.forward()).length() < 1.0e-6);
        assert!((perspective.focus_point() - orthographic.focus_point()).length() < 1.0e-5);
    }

    #[test]
    fn view_adjustment_uses_editor_zoom_and_pan_conventions() {
        let base = SceneCamera::default();
        let adjusted = SceneCamera::from_view_adjustment(
            Vec2::ZERO,
            2.0,
            Vec2::new(0.5, -0.25),
            SceneProjection::Perspective,
        );

        assert!((adjusted.focus_distance - base.focus_distance * 0.5).abs() < 1.0e-6);
        let shift = adjusted.focus_point() - base.focus_point();
        let expected = base.right() * -0.5 * adjusted.framed_half_height()
            + base.up() * 0.25 * adjusted.framed_half_height();
        assert!((shift - expected).length() < 1.0e-5);
    }

    #[test]
    fn invalid_editor_zoom_falls_back_to_the_default_distance() {
        let camera = SceneCamera::from_view_adjustment(
            Vec2::ZERO,
            f32::NAN,
            Vec2::ZERO,
            SceneProjection::Perspective,
        );
        assert!(
            (camera.focus_distance - SceneCamera::default().focus_distance).abs() < f32::EPSILON
        );
    }

    #[test]
    fn scene_camera_needs_no_authored_camera() {
        let camera = SceneCamera::default();
        let matrix = camera.view_projection(16.0 / 9.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
        let resolved = camera.resolved(16.0 / 9.0);
        assert_eq!(resolved.view_projection, matrix);
        assert_eq!(resolved.view, camera.view());
    }

    #[test]
    fn orthographic_projection_is_independent_of_position() {
        let mut a = SceneCamera {
            projection: SceneProjection::Orthographic,
            ..SceneCamera::default()
        };
        let projection = a.projection_matrix(1.5);
        a.position += Vec3::new(100.0, -20.0, 7.0);
        assert_eq!(a.projection_matrix(1.5), projection);
    }

    #[test]
    fn orthographic_zoom_changes_extent_without_moving_the_eye() {
        let mut camera = SceneCamera {
            projection: SceneProjection::Orthographic,
            ..SceneCamera::default()
        };
        let position = camera.position;
        let size = camera.orthographic_size;
        camera.zoom(0.5);
        assert_eq!(camera.position, position);
        assert!((camera.orthographic_size - size * 0.5).abs() < f32::EPSILON);
    }
}
