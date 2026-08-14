use glam::{Mat4, Vec2, Vec3};

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
        Mat4::perspective_rh(self.vertical_fov_radians, aspect_ratio, self.near, self.far)
            * Mat4::look_at_rh(self.eye, self.target, self.up)
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
        Mat4::orthographic_rh(
            -half_width,
            half_width,
            -half_height,
            half_height,
            self.near,
            self.far.max(self.near + f32::EPSILON),
        ) * Mat4::look_at_rh(eye, target, Vec3::Y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
