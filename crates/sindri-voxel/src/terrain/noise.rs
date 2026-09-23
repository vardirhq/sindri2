//! Value noise hashed from its own coordinates.
//!
//! Hashed rather than drawn from a random stream, so what a coordinate holds is
//! a property of where it is: the same seed gives the same world, and asking
//! about one column never depends on having asked about another first. That is
//! what lets a section be generated on its own, in any order, on any machine.

/// A number from zero to one for a lattice point in two dimensions.
pub(super) fn hash2(seed: u64, x: i32, z: i32) -> f32 {
    unit(mix(seed
        ^ i64::from(x)
            .cast_unsigned()
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ i64::from(z)
            .cast_unsigned()
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)))
}

/// A number from zero to one for a lattice point in three dimensions.
pub(super) fn hash3(seed: u64, x: i32, y: i32, z: i32) -> f32 {
    unit(mix(seed
        ^ i64::from(x)
            .cast_unsigned()
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ i64::from(y)
            .cast_unsigned()
            .wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ i64::from(z)
            .cast_unsigned()
            .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)))
}

const fn mix(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

/// The top 24 bits as a fraction, which an `f32` holds exactly.
#[allow(clippy::cast_precision_loss)]
fn unit(value: u64) -> f32 {
    (value >> 40) as f32 / 16_777_216.0
}

/// Smoothstep, so the lattice the noise is built on does not show as straight
/// creases running through every hillside.
fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// The lattice cell a coordinate falls in, and how far across it.
#[allow(clippy::cast_possible_truncation)]
fn cell(at: f32) -> (i32, f32) {
    let floor = at.floor();
    (floor as i32, at - floor)
}

/// Smoothly interpolated value noise at one frequency, from zero to one.
pub(super) fn noise2(seed: u64, x: f32, z: f32) -> f32 {
    let ((ix, fx), (iz, fz)) = (cell(x), cell(z));
    let (sx, sz) = (ease(fx), ease(fz));
    let near = lerp(hash2(seed, ix, iz), hash2(seed, ix + 1, iz), sx);
    let far = lerp(hash2(seed, ix, iz + 1), hash2(seed, ix + 1, iz + 1), sx);
    lerp(near, far, sz)
}

/// The same in three dimensions, for what varies with height as well: caves,
/// and the undercut faces of cliffs.
pub(super) fn noise3(seed: u64, x: f32, y: f32, z: f32) -> f32 {
    let ((ix, fx), (iy, fy), (iz, fz)) = (cell(x), cell(y), cell(z));
    let (sx, sy, sz) = (ease(fx), ease(fy), ease(fz));
    let plane = |iy: i32| {
        let near = lerp(hash3(seed, ix, iy, iz), hash3(seed, ix + 1, iy, iz), sx);
        let far = lerp(
            hash3(seed, ix, iy, iz + 1),
            hash3(seed, ix + 1, iy, iz + 1),
            sx,
        );
        lerp(near, far, sz)
    };
    lerp(plane(iy), plane(iy + 1), sy)
}

/// Several octaves of noise, which is what makes a coastline ragged at every
/// scale rather than smooth with a wobble. `scale` is the width of the largest
/// feature, in voxels. From zero to one, clustered around a half.
pub(super) fn fbm2(seed: u64, x: f32, z: f32, octaves: u32, scale: f32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut sum = 0.0;
    let mut frequency = 1.0 / scale;
    for octave in 0..octaves {
        let salt = seed ^ (u64::from(octave) << 17);
        total += noise2(salt, x * frequency, z * frequency) * amplitude;
        sum += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    total / sum
}

/// A field from `fbm2` pulled out towards its ends.
///
/// Averaging octaves bunches the answer around a half, so a climate read from
/// it straight would be temperate nearly everywhere and no threshold near the
/// ends would ever be crossed. Stretched, the extremes are regions rather than
/// rarities.
pub(super) fn spread(value: f32, by: f32) -> f32 {
    ((value - 0.5) * by + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_a_property_of_the_coordinate() {
        for step in 0..50 {
            #[allow(clippy::cast_precision_loss)]
            let at = step as f32 * 0.37 - 9.0;
            let once = noise2(7, at, -at);
            assert!((once - noise2(7, at, -at)).abs() < f32::EPSILON);
            assert!((0.0..=1.0).contains(&once));
            let deep = noise3(7, at, at * 0.5, -at);
            assert!((0.0..=1.0).contains(&deep));
        }
    }

    #[test]
    fn noise_is_continuous_across_lattice_lines() {
        // Either side of a lattice line the value barely changes: a step here
        // would be a cliff running dead straight through the world.
        for line in -4..4 {
            #[allow(clippy::cast_precision_loss)]
            let x = line as f32;
            let before = noise2(3, x - 1.0e-3, 0.4);
            let after = noise2(3, x + 1.0e-3, 0.4);
            assert!((before - after).abs() < 0.01, "{before} then {after}");
        }
    }

    #[test]
    fn negative_coordinates_are_not_a_mirror_of_positive_ones() {
        let mirrored = (1..40)
            .filter(|x| (hash2(1, *x, 3) - hash2(1, -x, 3)).abs() < f32::EPSILON)
            .count();
        assert!(mirrored < 2, "{mirrored} columns mirrored about the origin");
    }
}
