use std::cmp::Ordering;

use thiserror::Error;

/// Deterministic ordering key for transparent draws.
///
/// Lower layers render first. Within a layer, whatever is further from the
/// camera renders first (back-to-front). Submission order breaks exact ties
/// deterministically.
///
/// The distance is measured along the camera's forward axis rather than as a
/// straight line to the eye, so two things side by side at the same depth sort
/// as equally far away. It is geometry rather than an authored number: a layer
/// is the explicit override, and it wins, which is the one thing about this
/// order that surprises people.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransparentOrder {
    layer: i32,
    camera_distance: f32,
    submission_index: u32,
}

impl TransparentOrder {
    pub fn new(
        layer: i32,
        camera_distance: f32,
        submission_index: u32,
    ) -> Result<Self, TransparentOrderError> {
        if !camera_distance.is_finite() {
            return Err(TransparentOrderError::NonFiniteCameraDistance);
        }
        // Both zeroes are the same distance, and total_cmp would otherwise
        // order them apart.
        let camera_distance = if camera_distance == 0.0 {
            0.0
        } else {
            camera_distance
        };
        Ok(Self {
            layer,
            camera_distance,
            submission_index,
        })
    }

    pub const fn layer(self) -> i32 {
        self.layer
    }

    pub const fn camera_distance(self) -> f32 {
        self.camera_distance
    }

    pub const fn submission_index(self) -> u32 {
        self.submission_index
    }
}

impl Eq for TransparentOrder {}

impl Ord for TransparentOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        self.layer
            .cmp(&other.layer)
            .then_with(|| other.camera_distance.total_cmp(&self.camera_distance))
            .then_with(|| self.submission_index.cmp(&other.submission_index))
    }
}

impl PartialOrd for TransparentOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TransparentOrderError {
    #[error("the distance from the camera to a transparent draw must be finite")]
    NonFiniteCameraDistance,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(layer: i32, camera_distance: f32, submission_index: u32) -> TransparentOrder {
        TransparentOrder::new(layer, camera_distance, submission_index).unwrap()
    }

    #[test]
    fn layers_render_in_ascending_order() {
        let mut draws = [order(2, 100.0, 0), order(-1, 0.0, 1), order(1, -100.0, 2)];
        draws.sort();
        assert_eq!(draws.map(TransparentOrder::layer), [-1, 1, 2]);
    }

    #[test]
    fn a_shared_layer_renders_furthest_from_the_camera_first() {
        let mut draws = [order(0, 1.0, 0), order(0, 8.0, 1), order(0, 3.0, 2)];
        draws.sort();
        assert_eq!(
            draws.map(|draw| draw.camera_distance().to_bits()),
            [8.0_f32.to_bits(), 3.0_f32.to_bits(), 1.0_f32.to_bits()]
        );
    }

    /// The rule people are surprised by, held in place: a layer is an authored
    /// override, so it beats where things actually are.
    #[test]
    fn a_layer_overrides_the_geometry() {
        let mut draws = [order(1, 100.0, 0), order(0, 0.1, 1)];
        draws.sort();
        assert_eq!(draws.map(TransparentOrder::layer), [0, 1]);
    }

    #[test]
    fn submission_index_stabilizes_exact_ties() {
        let mut draws = [order(0, 2.0, 9), order(0, 2.0, 2), order(0, 2.0, 5)];
        draws.sort();
        assert_eq!(draws.map(TransparentOrder::submission_index), [2, 5, 9]);
    }

    #[test]
    fn rejects_non_finite_distances() {
        for distance in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                TransparentOrder::new(0, distance, 0),
                Err(TransparentOrderError::NonFiniteCameraDistance)
            );
        }
    }

    #[test]
    fn signed_zero_has_one_canonical_order() {
        assert_eq!(order(0, -0.0, 0), order(0, 0.0, 0));
        assert_eq!(order(0, -0.0, 0).cmp(&order(0, 0.0, 0)), Ordering::Equal);
    }
}
