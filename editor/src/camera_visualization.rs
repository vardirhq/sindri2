use glam::{Mat4, Vec3, Vec4};

/// A world-space line segment used to draw an authored camera in Scene view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrustumLine {
    pub start: Vec3,
    pub end: Vec3,
}

/// Reconstructs an authored camera frustum from the exact matrix it renders with.
///
/// Keeping this matrix-driven means the editor does not need a second copy of
/// perspective/orthographic camera maths. It also works before and after the
/// authored-camera format migration: if the renderer can produce the camera's
/// view-projection matrix, the Scene view can visualize it.
#[must_use]
pub fn frustum_lines(view_projection: Mat4) -> Option<[FrustumLine; 12]> {
    let inverse = view_projection.inverse();
    if !inverse.is_finite() {
        return None;
    }

    // WebGPU's clip-space depth is 0..1. The four near corners come first,
    // followed by the corresponding far corners.
    let clip = [
        Vec4::new(-1.0, -1.0, 0.0, 1.0),
        Vec4::new(1.0, -1.0, 0.0, 1.0),
        Vec4::new(1.0, 1.0, 0.0, 1.0),
        Vec4::new(-1.0, 1.0, 0.0, 1.0),
        Vec4::new(-1.0, -1.0, 1.0, 1.0),
        Vec4::new(1.0, -1.0, 1.0, 1.0),
        Vec4::new(1.0, 1.0, 1.0, 1.0),
        Vec4::new(-1.0, 1.0, 1.0, 1.0),
    ];
    let mut world = [Vec3::ZERO; 8];
    for (index, corner) in clip.into_iter().enumerate() {
        let homogeneous = inverse * corner;
        if !homogeneous.is_finite() || homogeneous.w.abs() <= f32::EPSILON {
            return None;
        }
        world[index] = homogeneous.truncate() / homogeneous.w;
        if !world[index].is_finite() {
            return None;
        }
    }

    let line = |a: usize, b: usize| FrustumLine {
        start: world[a],
        end: world[b],
    };
    Some([
        // Near plane.
        line(0, 1),
        line(1, 2),
        line(2, 3),
        line(3, 0),
        // Far plane.
        line(4, 5),
        line(5, 6),
        line(6, 7),
        line(7, 4),
        // Near-to-far edges.
        line(0, 4),
        line(1, 5),
        line(2, 6),
        line(3, 7),
    ])
}

#[cfg(test)]
mod tests {
    use sindri_render::{look_at, orthographic_projection, perspective_projection};

    use super::*;

    fn endpoints(lines: &[FrustumLine; 12]) -> impl Iterator<Item = Vec3> + '_ {
        lines.iter().flat_map(|line| [line.start, line.end])
    }

    #[test]
    fn perspective_frustum_is_finite_and_has_near_and_far_planes() {
        let view = look_at(Vec3::new(3.0, 2.0, 4.0), Vec3::ZERO, Vec3::Y);
        let projection = perspective_projection(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        let lines = frustum_lines(projection * view).expect("a valid camera has a frustum");

        assert!(endpoints(&lines).all(Vec3::is_finite));
        let near_width = (lines[0].end - lines[0].start).length();
        let far_width = (lines[4].end - lines[4].start).length();
        assert!(far_width > near_width, "a perspective frustum widens with distance");
    }

    #[test]
    fn orthographic_frustum_keeps_the_same_width() {
        let view = look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let projection = orthographic_projection(-4.0, 4.0, -3.0, 3.0, 0.1, 100.0);
        let lines = frustum_lines(projection * view).expect("a valid camera has a frustum");

        let near_width = (lines[0].end - lines[0].start).length();
        let far_width = (lines[4].end - lines[4].start).length();
        assert!((near_width - far_width).abs() < 1.0e-4);
    }

    #[test]
    fn singular_matrix_has_no_visualization() {
        assert!(frustum_lines(Mat4::ZERO).is_none());
    }
}
