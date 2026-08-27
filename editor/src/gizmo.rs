//! Transform gizmo geometry and drag math.
//!
//! The viewport owns drawing and input policy; this module owns the answer to
//! the harder question underneath both: which world-space axis a screen-space
//! handle represents, and what transform a pointer ray produces along it.

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use sindri_core::{EntityId, Transform3D};

#[cfg(test)]
mod tests;

const HANDLE_PIXELS: f32 = 72.0;
const HIT_RADIUS: f32 = 9.0;
const RING_SEGMENTS: u16 = 48;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GizmoMode {
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Translate => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GizmoSpace {
    World,
    #[default]
    Local,
}

impl GizmoSpace {
    /// What this space is called wherever the editor names it.
    ///
    /// The toolbar's switch and the viewport's status plate both say it, and
    /// two spellings of one state is how an interface teaches nobody anything.
    pub const fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Local => "Local",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    pub const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

/// What a drag rounds to, and whether it rounds at all.
///
/// Remembered rather than reset every launch: someone laying out a grid at half
/// a unit wants half a unit tomorrow too, and the increments were constants
/// nothing could change — the snap button's tooltip named three numbers and
/// there was nowhere to set any of them.
///
/// A step of zero means "do not round this one", which is what
/// `snapped` already did with it and is a useful answer: snapping a position
/// while leaving rotation free is an ordinary way to work.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Snapping {
    pub enabled: bool,
    pub translation: f32,
    pub rotation_degrees: f32,
    pub scale: f32,
}

impl Default for Snapping {
    fn default() -> Self {
        Self {
            enabled: false,
            translation: 0.5,
            rotation_degrees: 15.0,
            scale: 0.1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HandleVisual {
    pub axis: Axis,
    pub points: Vec<Vec2>,
}

#[derive(Clone, Debug)]
pub struct GizmoVisual {
    pub origin: Vec2,
    pub handles: Vec<HandleVisual>,
}

#[derive(Clone, Copy, Debug)]
pub struct GizmoDrag {
    pub entity: EntityId,
    pub mode: GizmoMode,
    pub axis: Axis,
    anchoring: Anchoring,
    start: Transform3D,
    direction: Vec3,
    start_parameter: f32,
    start_vector: Vec3,
    space: GizmoSpace,
}

#[derive(Clone, Copy)]
struct Ray {
    origin: Vec3,
    direction: Vec3,
}

/// Where a gizmo's handles belong, and what its axes mean there.
///
/// A world entity is placed by its transform, so the handle goes at the
/// transform and the two are the same thing. A UI element is not placed by its
/// transform: an anchor picks a point on the viewport and the transform is an
/// offset from that point, so drawing the handle at the transform puts it
/// wherever that offset happens to point in the world. Confirmed on Gather's
/// title: one red arm in the bottom-left corner of the Scene view, mostly off
/// screen, while the text it belonged to was at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchoring {
    /// The point the handles radiate from, in whatever space they are drawn in.
    origin: Vec3,
    /// Whether the third axis places the thing or only orders it.
    flat: bool,
}

impl Anchoring {
    /// A world entity, drawn where its transform says it is.
    #[must_use]
    pub fn in_world(transform: Transform3D) -> Self {
        Self {
            origin: Vec3::from_array(transform.position),
            flat: false,
        }
    }

    /// A UI element, drawn where the overlay puts it.
    ///
    /// `flat`, because a UI element's Z orders it within the overlay rather
    /// than placing it: a third arm would be a handle pointing straight at the
    /// viewer that changes a draw order when dragged, which is not what a
    /// translate handle promises.
    #[must_use]
    pub const fn on_overlay(origin: Vec2) -> Self {
        Self {
            origin: Vec3::new(origin.x, origin.y, 0.0),
            flat: true,
        }
    }
}

/// Builds the exact screen-space paths that are painted and hit-tested.
pub fn visual(
    mode: GizmoMode,
    anchoring: Anchoring,
    transform: Transform3D,
    space: GizmoSpace,
    view_projection: Mat4,
    viewport: Vec2,
    framed_half_height: f32,
) -> Option<GizmoVisual> {
    if mode == GizmoMode::Select || viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }
    let origin_world = anchoring.origin;
    let origin = project(view_projection, origin_world, viewport)?;
    let world_per_point = 2.0 * framed_half_height / viewport.y.max(1.0);
    let radius = (HANDLE_PIXELS * world_per_point).max(0.001);
    let rotation = normalized_rotation(transform);
    let mut handles = Vec::new();

    for axis in Axis::ALL {
        let Some(direction) = direction(axis, rotation, space, anchoring.locks_z(transform), mode)
        else {
            continue;
        };
        let points = if mode == GizmoMode::Rotate {
            ring(origin_world, direction, radius, view_projection, viewport)
        } else {
            let end = project(view_projection, origin_world + direction * radius, viewport);
            end.map_or_else(Vec::new, |end| vec![origin, end])
        };
        if points.len() >= 2 {
            handles.push(HandleVisual { axis, points });
        }
    }
    Some(GizmoVisual { origin, handles })
}

pub fn hit_test(visual: &GizmoVisual, pointer: Vec2) -> Option<Axis> {
    visual
        .handles
        .iter()
        .filter_map(|handle| {
            let distance = handle
                .points
                .windows(2)
                .map(|segment| segment_distance(pointer, segment[0], segment[1]))
                .fold(f32::INFINITY, f32::min);
            (distance <= HIT_RADIUS).then_some((handle.axis, distance))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(axis, _)| axis)
}

#[allow(clippy::too_many_arguments)]
pub fn begin_drag(
    entity: EntityId,
    mode: GizmoMode,
    axis: Axis,
    anchoring: Anchoring,
    transform: Transform3D,
    space: GizmoSpace,
    view_projection: Mat4,
    pointer: Vec2,
    viewport: Vec2,
) -> Option<GizmoDrag> {
    let rotation = normalized_rotation(transform);
    let direction = direction(axis, rotation, space, anchoring.locks_z(transform), mode)?;
    let ray = ray(view_projection, pointer, viewport)?;
    let origin = anchoring.origin;
    let (start_parameter, start_vector) = if mode == GizmoMode::Rotate {
        let point = ray_plane(ray, origin, direction)?;
        (0.0, (point - origin).normalize_or_zero())
    } else {
        (axis_parameter(ray, origin, direction)?, Vec3::ZERO)
    };
    Some(GizmoDrag {
        entity,
        mode,
        axis,
        anchoring,
        start: transform,
        direction,
        start_parameter,
        start_vector,
        space,
    })
}

pub fn update_drag(
    drag: GizmoDrag,
    view_projection: Mat4,
    pointer: Vec2,
    viewport: Vec2,
    snapping: Snapping,
) -> Option<Transform3D> {
    let ray = ray(view_projection, pointer, viewport)?;
    // Where the handle is, which is not always where the transform is: the
    // pointer maths is done against the drawn origin, and the answer is applied
    // to the authored value.
    let origin = drag.anchoring.origin;
    let mut next = drag.start;
    match drag.mode {
        GizmoMode::Select => return None,
        GizmoMode::Translate => {
            let current = axis_parameter(ray, origin, drag.direction)?;
            let delta = snapped(
                current - drag.start_parameter,
                snapping.translation,
                snapping,
            );
            let authored = Vec3::from_array(drag.start.position);
            let mut position = authored + drag.direction * delta;
            if drag.anchoring.locks_z(drag.start) {
                position.z = authored.z;
            }
            next.position = position.to_array();
        }
        GizmoMode::Scale => {
            let current = axis_parameter(ray, origin, drag.direction)?;
            let delta = current - drag.start_parameter;
            let index = drag.axis.index();
            next.scale[index] = snapped(drag.start.scale[index] + delta, snapping.scale, snapping);
        }
        GizmoMode::Rotate => {
            let point = ray_plane(ray, origin, drag.direction)?;
            let current = (point - origin).normalize_or_zero();
            let sin = drag.direction.dot(drag.start_vector.cross(current));
            let cos = drag.start_vector.dot(current);
            let angle = snapped(
                sin.atan2(cos),
                snapping.rotation_degrees.to_radians(),
                snapping,
            );
            let delta = Quat::from_axis_angle(drag.axis.vector(), angle);
            let start = normalized_rotation(drag.start);
            let rotation = match drag.space {
                GizmoSpace::Local => start * delta,
                GizmoSpace::World => Quat::from_axis_angle(drag.direction, angle) * start,
            };
            next.rotation = rotation.normalize().to_array();
        }
    }
    Some(next)
}

impl Anchoring {
    /// Whether a translate handle may move this thing off its layer.
    ///
    /// Either because the entity says so, or because the space it is drawn in
    /// has no third dimension to move through.
    const fn locks_z(self, transform: Transform3D) -> bool {
        self.flat || transform.z_locked
    }
}

fn normalized_rotation(transform: Transform3D) -> Quat {
    let rotation = Quat::from_array(transform.rotation);
    if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn direction(
    axis: Axis,
    rotation: Quat,
    space: GizmoSpace,
    z_locked: bool,
    mode: GizmoMode,
) -> Option<Vec3> {
    let mut direction = match (mode, space) {
        // Scale is stored on the transform's local axes. Drawing a world axis
        // while changing a local component would make the visible promise and
        // the authored result disagree.
        (GizmoMode::Scale, _) | (_, GizmoSpace::Local) => rotation * axis.vector(),
        (_, GizmoSpace::World) => axis.vector(),
    };
    if z_locked && mode == GizmoMode::Translate {
        direction.z = 0.0;
    }
    (direction.length_squared() > 0.000_001).then(|| direction.normalize())
}

fn snapped(value: f32, step: f32, snapping: Snapping) -> f32 {
    if snapping.enabled && step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

fn project(view_projection: Mat4, point: Vec3, viewport: Vec2) -> Option<Vec2> {
    let clip = view_projection * point.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.000_001 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport.x,
        (1.0 - ndc.y) * 0.5 * viewport.y,
    ))
}

fn ray(view_projection: Mat4, pointer: Vec2, viewport: Vec2) -> Option<Ray> {
    let inverse = view_projection.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let x = pointer.x / viewport.x.max(1.0) * 2.0 - 1.0;
    let y = 1.0 - pointer.y / viewport.y.max(1.0) * 2.0;
    // Sindri cameras use WebGPU's zero-to-one depth convention.
    let near = unproject(inverse, Vec4::new(x, y, 0.0, 1.0))?;
    let far = unproject(inverse, Vec4::new(x, y, 1.0, 1.0))?;
    let direction = (far - near).normalize_or_zero();
    (direction.length_squared() > 0.0).then_some(Ray {
        origin: near,
        direction,
    })
}

fn unproject(inverse: Mat4, clip: Vec4) -> Option<Vec3> {
    let world = inverse * clip;
    (world.is_finite() && world.w.abs() > 0.000_001).then(|| world.truncate() / world.w)
}

fn axis_parameter(ray: Ray, origin: Vec3, axis: Vec3) -> Option<f32> {
    let dot = ray.direction.dot(axis);
    let denominator = 1.0 - dot * dot;
    if denominator.abs() < 0.000_1 {
        return None;
    }
    let offset = origin - ray.origin;
    Some((ray.direction.dot(offset) * dot - axis.dot(offset)) / denominator)
}

fn ray_plane(ray: Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 0.000_1 {
        return None;
    }
    let distance = (origin - ray.origin).dot(normal) / denominator;
    Some(ray.origin + ray.direction * distance)
}

fn ring(
    origin: Vec3,
    normal: Vec3,
    radius: f32,
    view_projection: Mat4,
    viewport: Vec2,
) -> Vec<Vec2> {
    let reference = if normal.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let tangent = normal.cross(reference).normalize();
    let bitangent = normal.cross(tangent).normalize();
    (0..=RING_SEGMENTS)
        .filter_map(|index| {
            let angle = f32::from(index) / f32::from(RING_SEGMENTS) * std::f32::consts::TAU;
            let point = origin + (tangent * angle.cos() + bitangent * angle.sin()) * radius;
            project(view_projection, point, viewport)
        })
        .collect()
}

fn segment_distance(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let amount = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * amount)
}
