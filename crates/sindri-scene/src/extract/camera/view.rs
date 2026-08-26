//! The editor-side view of a scene: where it is looking from, and how.

use glam::{Mat4, Quat, Vec2, Vec3};
use sindri_core::Transform3D;
use sindri_render::OrthographicCamera;

use super::{ResolvedCamera, ResolvedCameras};

/// Which projection a world view uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldProjection {
    /// Render through the authored gameplay camera exactly as the scene defines it.
    #[default]
    Authored,
    /// Use the independent viewer camera with a perspective projection.
    Perspective,
    /// Use the independent viewer camera with an orthographic projection framed
    /// to match perspective, so toggling keeps the subject the same size.
    Orthographic,
}

/// A viewer's camera adjustment.
///
/// The default view is the authored gameplay camera. Explicit perspective or
/// orthographic projections describe the independent editor/viewer camera, so
/// Scene navigation never mutates or depends on a gameplay camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    /// Yaw and pitch in radians.
    pub orbit: Vec2,
    /// Multiplier on the eye-to-focus distance.
    pub distance_scale: f32,
    /// Sideways and upward shift across the view plane, in fractions of the
    /// framed half-height.
    ///
    /// Measured against what the camera currently frames rather than in world
    /// units, so dragging moves the picture the same distance whether the
    /// subject is a metre away or a kilometre, and the two projections agree.
    pub pan: Vec2,
    pub projection: WorldProjection,
}

impl Default for CameraView {
    fn default() -> Self {
        Self {
            orbit: Vec2::ZERO,
            distance_scale: 1.0,
            pan: Vec2::ZERO,
            projection: WorldProjection::Authored,
        }
    }
}

/// The screen overlay's visible half-size, which sprite anchors resolve against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OverlayExtent {
    pub(crate) center: Vec2,
    pub(crate) half_extent: Vec2,
}

/// The stable screen-space coordinate system used by HUD sprites and text.
///
/// It deliberately preserves the old overlay extent: vertical size 2, centered
/// at the viewport origin. The difference in format 7 is ownership, not the
/// authored coordinates — the viewport derives this projection itself, so no
/// camera entity is required to make UI exist.
pub(crate) fn resolved_screen_overlay(aspect: f32) -> ResolvedCameras {
    let vertical_size = 2.0;
    let near = 0.0;
    let far = 10.0;
    let camera = OrthographicCamera {
        center: Vec2::ZERO,
        vertical_size,
        near,
        far,
    };
    let half_height = vertical_size * 0.5;
    ResolvedCameras {
        world: None,
        overlay: Some(ResolvedCamera {
            view: camera.view(),
            view_projection: camera.view_projection(aspect),
            framed_half_height: half_height,
        }),
        overlay_extent: Some(OverlayExtent {
            center: Vec2::ZERO,
            half_extent: Vec2::new(half_height * aspect, half_height),
        }),
    }
}

pub(crate) fn safe_rotation(transform: Transform3D) -> Quat {
    let rotation = Quat::from_array(transform.rotation);
    if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

/// Turns a pan measured in framed units into a world-space shift.
///
/// The shift stays in the plane the camera faces, so panning slides the picture
/// rather than pushing the camera towards or away from what it is looking at.
pub(crate) fn panned_shift(offset: Vec3, up: Vec3, pan: Vec2) -> Vec3 {
    let forward = -offset.normalize_or_zero();
    let right = forward.cross(up).normalize_or_zero();
    let plane_up = right.cross(forward);
    right * -pan.x + plane_up * -pan.y
}

/// How close to straight up or straight down an orbit may take the camera.
///
/// At the pole the offset is parallel to `up`, and the pair no longer says
/// which way round the picture goes. `look_at` still returns a matrix there
/// rather than failing, which is worse than failing: the roll it picks is
/// decided by whatever rounding error survived, so dragging through straight
/// down whips the whole scene round to face the other way. Stopping a hundredth
/// of a radian short costs nothing anyone can see and removes the case.
pub(crate) const POLAR_LIMIT: f32 = 0.01;

/// Where the camera sits after the viewer's orbit, relative to its target.
///
/// Pitch turns the offset in the plane that holds it and `up`, so it adds
/// directly to the angle between them. That is what makes the guard here exact:
/// the limit is applied to the angle it actually produces, rather than to a
/// pitch that a caller would have to combine with an authored elevation it
/// cannot see to know whether it was safe.
pub(crate) fn orbited_offset(authored_offset: Vec3, up: Vec3, view: CameraView) -> Vec3 {
    let scaled = authored_offset * view.distance_scale;
    let yawed = Quat::from_axis_angle(up, view.orbit.x) * scaled;
    let right = up.cross(yawed).normalize_or_zero();
    if right == Vec3::ZERO {
        // Authored looking straight down its own up axis. There is no axis to
        // pitch about, and the scene chose this, so it is left alone.
        return yawed;
    }
    let polar = up.angle_between(yawed);
    let pitch = view.orbit.y.clamp(
        POLAR_LIMIT - polar,
        std::f32::consts::PI - POLAR_LIMIT - polar,
    );
    Quat::from_axis_angle(right, pitch) * yawed
}

/// How far in front of a camera a point is, which is what transparent draws
/// sort by.
///
/// Measured along the camera's forward axis rather than as a straight line to
/// the eye: two sprites side by side at the same depth have to sort as equally
/// far away, and a radial distance would call the one nearer the edge of the
/// screen further back. Nothing divides, so a sprite sitting exactly on the
/// camera plane produces a number rather than an infinity.
pub(crate) fn camera_distance(view: Mat4, position: Vec3) -> f32 {
    -(view * position.extend(1.0)).z
}
