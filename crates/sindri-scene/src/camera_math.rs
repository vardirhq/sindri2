use glam::{Mat3, Quat, Vec3};

const DIRECTION_EPSILON: f32 = 1.0e-6;

/// Builds an authored camera rotation from the old eye/target/up model.
///
/// Sindri cameras face local -Z with local +Y as their up direction. This turns
/// the legacy look-at description into the quaternion a `Transform3D` needs so
/// migrating a scene does not change the picture the camera produces.
#[must_use]
pub fn camera_rotation_from_look_at(eye: Vec3, target: Vec3, authored_up: Vec3) -> Quat {
    let forward = (target - eye).normalize_or_zero();
    if forward.length_squared() <= DIRECTION_EPSILON {
        return Quat::IDENTITY;
    }

    let mut up = authored_up.normalize_or_zero();
    if up.length_squared() <= DIRECTION_EPSILON
        || up.cross(forward).length_squared() <= DIRECTION_EPSILON
    {
        up = fallback_up(forward);
    }

    let right = forward.cross(up).normalize_or_zero();
    let corrected_up = right.cross(forward).normalize_or_zero();
    let basis = Mat3::from_cols(right, corrected_up, -forward);
    Quat::from_mat3(&basis).normalize()
}

fn fallback_up(forward: Vec3) -> Vec3 {
    if forward.dot(Vec3::Y).abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: Vec3, expected: Vec3) {
        assert!(
            (actual - expected).length() < 1.0e-5,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn rotation_points_local_negative_z_at_the_old_target() {
        let eye = Vec3::new(3.0, 2.0, 4.0);
        let target = Vec3::ZERO;
        let rotation = camera_rotation_from_look_at(eye, target, Vec3::Y);

        assert_near(rotation * -Vec3::Z, (target - eye).normalize());
    }

    #[test]
    fn rotation_preserves_the_camera_up_plane() {
        let eye = Vec3::new(4.0, 3.0, 7.0);
        let target = Vec3::new(-2.0, 1.0, 0.5);
        let authored_up = Vec3::Y;
        let forward = (target - eye).normalize();
        let expected_right = forward.cross(authored_up).normalize();
        let expected_up = expected_right.cross(forward).normalize();
        let rotation = camera_rotation_from_look_at(eye, target, authored_up);

        assert_near(rotation * Vec3::Y, expected_up);
        assert_near(rotation * Vec3::X, expected_right);
    }

    #[test]
    fn looking_straight_up_still_produces_a_finite_rotation() {
        let rotation = camera_rotation_from_look_at(Vec3::ZERO, Vec3::Y, Vec3::Y);
        assert!(rotation.is_finite());
        assert_near(rotation * -Vec3::Z, Vec3::Y);
    }

    #[test]
    fn zero_length_look_direction_falls_back_to_identity() {
        assert_eq!(
            camera_rotation_from_look_at(Vec3::ONE, Vec3::ONE, Vec3::Y),
            Quat::IDENTITY
        );
    }
}
