use glam::{Mat4, Vec3};

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
        Mat4::perspective_rh(
            self.vertical_fov_radians,
            aspect_ratio,
            self.near,
            self.far,
        ) * Mat4::look_at_rh(self.eye, self.target, self.up)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_matrix_is_finite() {
        let matrix = PerspectiveCamera::default().view_projection(16.0 / 9.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
    }

    #[test]
    fn aspect_ratio_changes_horizontal_projection() {
        let camera = PerspectiveCamera::default();
        let square = camera.view_projection(1.0);
        let wide = camera.view_projection(2.0);
        assert!(wide.x_axis.x.abs() < square.x_axis.x.abs());
        assert_eq!(wide.y_axis.y, square.y_axis.y);
    }

    #[test]
    fn zero_aspect_ratio_does_not_produce_non_finite_values() {
        let matrix = PerspectiveCamera::default().view_projection(0.0);
        assert!(matrix.to_cols_array().into_iter().all(f32::is_finite));
    }
}
