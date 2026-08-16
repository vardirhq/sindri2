use glam::{Mat4, Vec2, Vec3};

/// The projection convention every Sindri camera uses.
///
/// WebGPU clips depth to `[0, 1]` with Y up, which glam names after DirectX.
/// The choice lives here once because the alternatives are not loud when they
/// are wrong: OpenGL's `[-1, 1]` range and Vulkan's Y-down variant both
/// compile, render, and pass every "is this finite" test while quietly
/// halving depth precision or flipping the image. A caller that picks a
/// module per call site is three chances to pick differently.
///
/// `sindri-scene` builds a camera or two of its own, so these are public
/// rather than private helpers — reaching past them into `glam::camera` is
/// how the convention stops being one decision.
pub fn perspective_projection(
    vertical_fov_radians: f32,
    aspect_ratio: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    glam::camera::rh::proj::directx::perspective(vertical_fov_radians, aspect_ratio, near, far)
}

/// The orthographic half of [`perspective_projection`]'s convention.
pub fn orthographic_projection(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    glam::camera::rh::proj::directx::orthographic(left, right, bottom, top, near, far)
}

/// A right-handed view matrix, paired with the projections above.
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    glam::camera::rh::view::look_at_mat4(eye, target, up)
}

/// Perspective camera using WebGPU's zero-to-one depth range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub vertical_fov_radians: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for PerspectiveCamera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(3.0, 2.0, 4.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            vertical_fov_radians: 45.0_f32.to_radians(),
            near: 0.1,
            far: 100.0,
        }
    }
}

impl PerspectiveCamera {
    pub fn view_projection(self, aspect_ratio: f32) -> Mat4 {
        let aspect_ratio = aspect_ratio.max(f32::EPSILON);
        perspective_projection(self.vertical_fov_radians, aspect_ratio, self.near, self.far)
            * look_at(self.eye, self.target, self.up)
    }
}

/// Centered orthographic camera for 2D worlds and screen-space overlays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrthographicCamera {
    pub center: Vec2,
    pub vertical_size: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for OrthographicCamera {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            vertical_size: 2.0,
            near: 0.0,
            far: 10.0,
        }
    }
}

impl OrthographicCamera {
    pub fn view_projection(self, aspect_ratio: f32) -> Mat4 {
        let aspect_ratio = aspect_ratio.max(f32::EPSILON);
        let half_height = self.vertical_size.max(f32::EPSILON) * 0.5;
        let half_width = half_height * aspect_ratio;
        let eye = self.center.extend(1.0);
        let target = self.center.extend(0.0);
        orthographic_projection(
            -half_width,
            half_width,
            -half_height,
            half_height,
            self.near,
            self.far.max(self.near + f32::EPSILON),
        ) * look_at(eye, target, Vec3::Y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The depth convention, pinned by what it actually does rather than by
    /// which library function produced it.
    ///
    /// Every other test here passes just as well under OpenGL's `[-1, 1]`
    /// range, so none of them would notice a projection swapped for the wrong
    /// one. A wrong range renders — it just spends half the depth buffer on
    /// nothing.
    #[test]
    fn the_near_plane_is_depth_zero_and_the_far_plane_is_depth_one() {
        let camera = PerspectiveCamera {
            eye: Vec3::ZERO,
            target: Vec3::NEG_Z,
            ..PerspectiveCamera::default()
        };
        let matrix = camera.view_projection(1.0);

        let depth_at = |distance: f32| {
            let clip = matrix * Vec3::new(0.0, 0.0, -distance).extend(1.0);
            clip.z / clip.w
        };

        assert!(depth_at(camera.near).abs() < 1.0e-4, "near should map to 0");
        assert!(
            (depth_at(camera.far) - 1.0).abs() < 1.0e-4,
            "far should map to 1"
        );
    }

    /// The same claim for the overlay camera, which sprites are positioned by.
    #[test]
    fn the_orthographic_near_and_far_planes_span_zero_to_one() {
        let camera = OrthographicCamera::default();
        let matrix = camera.view_projection(1.0);
        // The camera sits at z = 1 looking towards z = 0, so `near` and `far`
        // are distances along that direction.
        let depth_at = |distance: f32| (matrix * Vec3::new(0.0, 0.0, 1.0 - distance).extend(1.0)).z;

        assert!(depth_at(camera.near).abs() < 1.0e-4, "near should map to 0");
        assert!(
            (depth_at(camera.far) - 1.0).abs() < 1.0e-4,
            "far should map to 1"
        );
    }

    /// Y-up, not Vulkan's Y-down: a point above the target projects to
    /// positive Y. A flipped image is the other quiet way to get this wrong.
    #[test]
    fn a_point_above_the_camera_target_projects_upwards() {
        let camera = PerspectiveCamera {
            eye: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            ..PerspectiveCamera::default()
        };
        let clip = camera.view_projection(1.0) * Vec3::new(0.0, 1.0, 0.0).extend(1.0);
        assert!(
            clip.y / clip.w > 0.0,
            "up in the world must be up in clip space"
        );
    }

    #[test]
    fn default_perspective_camera_matrix_is_finite() {
        let matrix = PerspectiveCamera::default().view_projection(16.0 / 9.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
    }

    #[test]
    fn perspective_aspect_ratio_changes_horizontal_projection() {
        let camera = PerspectiveCamera::default();
        let square = camera.view_projection(1.0);
        let wide = camera.view_projection(2.0);
        assert!(wide.x_axis.x.abs() < square.x_axis.x.abs());
        assert!((wide.y_axis.y - square.y_axis.y).abs() <= f32::EPSILON);
    }

    #[test]
    fn zero_perspective_aspect_ratio_does_not_produce_non_finite_values() {
        let matrix = PerspectiveCamera::default().view_projection(0.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
    }

    #[test]
    fn default_orthographic_camera_matrix_is_finite() {
        let matrix = OrthographicCamera::default().view_projection(16.0 / 9.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
    }

    #[test]
    fn orthographic_aspect_ratio_preserves_vertical_scale() {
        let camera = OrthographicCamera::default();
        let square = camera.view_projection(1.0);
        let wide = camera.view_projection(2.0);
        assert!(wide.x_axis.x.abs() < square.x_axis.x.abs());
        assert!((wide.y_axis.y - square.y_axis.y).abs() <= f32::EPSILON);
    }

    #[test]
    fn orthographic_center_maps_to_view_center() {
        let camera = OrthographicCamera {
            center: Vec2::new(12.0, -4.0),
            ..OrthographicCamera::default()
        };
        let projected = camera.view_projection(1.0) * camera.center.extend(0.0).extend(1.0);
        assert!(projected.x.abs() < 0.000_01);
        assert!(projected.y.abs() < 0.000_01);
    }
}
