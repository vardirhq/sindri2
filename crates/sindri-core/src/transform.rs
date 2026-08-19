use serde::{Deserialize, Serialize};

/// Where a thing is, how it is turned, and how big it is.
///
/// There is one transform, and it has three dimensions. A 2D game is one that
/// keeps everything on a plane and points a camera at it, rather than one whose
/// entities carry a flatter transform — see `docs/2d-model.md`.
///
/// That leaves a hazard worth naming. Z is doing presentation work in a 2D
/// scene: it is what layers a background behind a player and what a perspective
/// camera turns into parallax. The classic way such a scene collapses is a line
/// like `position = [x, y, 0.0]`, written by someone who was only thinking
/// about X and Y and had no reason to look up what Z was. The [2D
/// accessors](Self::set_position_2d) exist so that person has a call that reads
/// and writes exactly the two numbers they are thinking about, and cannot
/// express the third.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform3D {
    pub position: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// Whether this transform declares that it stays on the layer it is on.
    ///
    /// See [`Transform3D::z_lock_rejects`] for what respects it. Omitted from a
    /// saved scene when false, so declaring nothing writes nothing and every
    /// scene that predates the lock is byte for byte what it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub z_locked: bool,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            z_locked: false,
        }
    }
}

/// The 2D shape of a 3D transform: two numbers in, two numbers out, and no way
/// to say anything about the third.
///
/// This is the strong half of keeping a layered scene layered. The other half
/// is a Z lock, which is a check that a write path can respect; this is not a
/// check at all. Anyone thinking in two dimensions reaches for the 2D call, and
/// the 2D call is incapable of the mistake, so there is nothing to remember and
/// nothing to enforce.
impl Transform3D {
    /// Where the transform is in the plane.
    pub const fn position_2d(self) -> [f32; 2] {
        [self.position[0], self.position[1]]
    }

    /// Moves it in the plane, leaving Z exactly where it was.
    pub const fn set_position_2d(&mut self, position: [f32; 2]) {
        self.position[0] = position[0];
        self.position[1] = position[1];
    }

    /// Moves it by an offset in the plane, leaving Z exactly where it was.
    pub const fn translate_2d(&mut self, offset: [f32; 2]) {
        self.position[0] += offset[0];
        self.position[1] += offset[1];
    }

    /// How big it is in the plane.
    pub const fn scale_2d(self) -> [f32; 2] {
        [self.scale[0], self.scale[1]]
    }

    /// Resizes it in the plane. A flat thing has no thickness to scale, so Z
    /// keeps whatever it had — normally the identity 1.
    pub const fn set_scale_2d(&mut self, scale: [f32; 2]) {
        self.scale[0] = scale[0];
        self.scale[1] = scale[1];
    }

    /// The turn about Z, which is the only turn a flat thing facing the camera
    /// has.
    ///
    /// A transform tilted about X or Y is not a 2D thing, and this reads only
    /// the Z part of it rather than inventing a single angle that describes the
    /// whole rotation, because no single angle does.
    pub fn rotation_z_radians(self) -> f32 {
        2.0 * f32::atan2(self.rotation[2], self.rotation[3])
    }

    /// Turns it about Z.
    ///
    /// The rotation becomes a turn about Z alone: an angle is the whole of a 2D
    /// rotation, so there is nothing left for a tilt to mean. Written out
    /// rather than composed with whatever was there, so setting the same angle
    /// twice is the same transform both times.
    pub fn set_rotation_z_radians(&mut self, radians: f32) {
        let (sin, cos) = (radians * 0.5).sin_cos();
        self.rotation = [0.0, 0.0, sin, cos];
    }

    /// Whether replacing this transform with `next` would move it off the layer
    /// it declared it stays on.
    ///
    /// This is the weaker of the two guards the 2D model describes, and it is
    /// weak on purpose: it holds for writes that go through a path that can ask,
    /// and a direct field assignment walks straight past it. What it buys is
    /// that the author said something — "this stays on its layer" — which is
    /// visible in the inspector, saved with the scene, and checked by the
    /// command layer every tool writes through.
    ///
    /// Removing the transform counts as moving it. An entity without one is at
    /// Z = 0, so dropping a locked transform lands a parallax layer in the play
    /// plane exactly as writing a different number would.
    pub fn z_lock_rejects(self, next: Option<Self>) -> bool {
        if !self.z_locked {
            return false;
        }
        // An exact comparison is the point rather than a hazard: any change to
        // Z at all is a change of layer, and there is no tolerance within which
        // one is the other.
        #[allow(clippy::float_cmp)]
        next.is_none_or(|next| next.position[2] != self.position[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact comparison, because these numbers are exact: every value here is
    /// representable, and "close enough" would hide a Z that moved a little.
    fn bits<const N: usize>(values: [f32; N]) -> [u32; N] {
        values.map(f32::to_bits)
    }

    /// The property the whole 2D shape exists for.
    #[test]
    fn a_two_dimensional_write_cannot_touch_the_layer_it_sits_on() {
        let mut background = Transform3D {
            position: [1.0, 2.0, -50.0],
            ..Transform3D::default()
        };

        background.set_position_2d([8.0, -3.0]);
        background.translate_2d([0.5, 0.5]);
        background.set_scale_2d([2.0, 4.0]);
        background.set_rotation_z_radians(0.75);

        assert_eq!(bits(background.position_2d()), bits([8.5, -2.5]));
        assert_eq!(
            background.position[2].to_bits(),
            (-50.0_f32).to_bits(),
            "a parallax layer must survive being moved around in the plane"
        );
        assert_eq!(bits(background.scale_2d()), bits([2.0, 4.0]));
        assert_eq!(
            background.scale[2].to_bits(),
            1.0_f32.to_bits(),
            "a flat thing has no thickness"
        );
    }

    #[test]
    fn a_two_dimensional_turn_round_trips_through_the_quaternion() {
        let mut transform = Transform3D::default();
        for angle in [0.0, 0.5, -1.25, std::f32::consts::FRAC_PI_2, 3.0] {
            transform.set_rotation_z_radians(angle);
            assert!(
                (transform.rotation_z_radians() - angle).abs() < 1.0e-5,
                "{angle} came back as {}",
                transform.rotation_z_radians()
            );
            assert_eq!(
                bits([transform.rotation[0], transform.rotation[1]]),
                bits([0.0, 0.0]),
                "a 2D turn is about Z alone"
            );
        }
    }

    /// The lock is about the layer and nothing else: a locked transform can
    /// still be moved around in its plane, resized, and turned.
    #[test]
    fn a_locked_transform_refuses_only_a_change_of_layer() {
        let locked = Transform3D {
            position: [1.0, 2.0, -50.0],
            z_locked: true,
            ..Transform3D::default()
        };

        let mut moved_in_plane = locked;
        moved_in_plane.set_position_2d([9.0, 9.0]);
        moved_in_plane.set_rotation_z_radians(1.0);
        moved_in_plane.set_scale_2d([3.0, 3.0]);
        assert!(!locked.z_lock_rejects(Some(moved_in_plane)));

        let mut moved_off_layer = locked;
        moved_off_layer.position[2] = 0.0;
        assert!(locked.z_lock_rejects(Some(moved_off_layer)));

        // Dropping the transform puts the entity at Z = 0 just as surely.
        assert!(locked.z_lock_rejects(None));
    }

    /// A lock declared by the incoming transform does not apply to the write
    /// that declares it, so locking something is never a thing you have to do
    /// before you can put it where it goes.
    #[test]
    fn a_lock_only_binds_once_it_has_been_declared() {
        let free = Transform3D::default();
        let locked_elsewhere = Transform3D {
            position: [0.0, 0.0, -12.0],
            z_locked: true,
            ..Transform3D::default()
        };
        assert!(!free.z_lock_rejects(Some(locked_elsewhere)));

        // And unlocking in place is allowed, which is how you get permission to
        // move afterwards.
        let unlocked = Transform3D {
            z_locked: false,
            ..locked_elsewhere
        };
        assert!(!locked_elsewhere.z_lock_rejects(Some(unlocked)));
    }

    /// Signed zero is the same layer, whatever the bits say.
    #[test]
    fn a_negative_zero_layer_is_the_layer_it_already_was() {
        let locked = Transform3D {
            position: [0.0, 0.0, 0.0],
            z_locked: true,
            ..Transform3D::default()
        };
        let same_place = Transform3D {
            position: [0.0, 0.0, -0.0],
            ..locked
        };
        assert!(!locked.z_lock_rejects(Some(same_place)));
    }

    /// The rotation written is the quaternion the renderer expects, rather than
    /// merely something that reads back the same way.
    #[test]
    fn a_quarter_turn_is_the_quaternion_a_quarter_turn_should_be() {
        let mut transform = Transform3D::default();
        transform.set_rotation_z_radians(std::f32::consts::FRAC_PI_2);
        let eighth = std::f32::consts::FRAC_PI_4.sin();
        assert!((transform.rotation[2] - eighth).abs() < 1.0e-6);
        assert!((transform.rotation[3] - eighth).abs() < 1.0e-6);
    }
}
