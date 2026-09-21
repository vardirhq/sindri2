//! Conservative bounds tests shared by cached scene geometry.

use glam::{Mat4, Vec3};

/// Whether any part of an axis-aligned box could be inside the camera frustum.
///
/// A box is rejected only when all eight corners lie beyond the same clip
/// plane. That can retain a box which does not really intersect the frustum,
/// but it cannot cut a visible piece out of the world.
pub(super) fn aabb_in_view(min: Vec3, max: Vec3, view_projection: Mat4) -> bool {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ];
    let clip = corners.map(|corner| view_projection * corner.extend(1.0));
    !(clip.iter().all(|point| point.x < -point.w)
        || clip.iter().all(|point| point.x > point.w)
        || clip.iter().all(|point| point.y < -point.w)
        || clip.iter().all(|point| point.y > point.w)
        || clip.iter().all(|point| point.z < 0.0)
        || clip.iter().all(|point| point.z > point.w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_only_boxes_wholly_beyond_one_clip_plane() {
        assert!(aabb_in_view(
            Vec3::new(-0.5, -0.5, 0.25),
            Vec3::new(0.5, 0.5, 0.75),
            Mat4::IDENTITY,
        ));
        assert!(!aabb_in_view(
            Vec3::new(2.0, -0.5, 0.25),
            Vec3::new(3.0, 0.5, 0.75),
            Mat4::IDENTITY,
        ));
        assert!(aabb_in_view(
            Vec3::new(0.5, -0.5, 0.25),
            Vec3::new(2.0, 0.5, 0.75),
            Mat4::IDENTITY,
        ));
    }
}
