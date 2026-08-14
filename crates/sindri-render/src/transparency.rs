use std::cmp::Ordering;

use thiserror::Error;

/// Deterministic ordering key for transparent draws.
///
/// Lower layers render first. Within a layer, greater depth renders first
/// (back-to-front). Submission order breaks exact ties deterministically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransparentOrder {
    layer: i32,
    depth: f32,
    submission_index: u32,
}

impl TransparentOrder {
    pub fn new(
        layer: i32,
        depth: f32,
        submission_index: u32,
    ) -> Result<Self, TransparentOrderError> {
        if !depth.is_finite() {
            return Err(TransparentOrderError::NonFiniteDepth);
        }
        let depth = if depth == 0.0 { 0.0 } else { depth };
        Ok(Self {
            layer,
            depth,
            submission_index,
        })
    }

    pub const fn layer(self) -> i32 {
        self.layer
    }

    pub const fn depth(self) -> f32 {
        self.depth
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
            .then_with(|| other.depth.total_cmp(&self.depth))
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
    #[error("transparent draw depth must be finite")]
    NonFiniteDepth,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(layer: i32, depth: f32, submission_index: u32) -> TransparentOrder {
        TransparentOrder::new(layer, depth, submission_index).unwrap()
    }

    #[test]
    fn layers_render_in_ascending_order() {
        let mut draws = [order(2, 100.0, 0), order(-1, 0.0, 1), order(1, -100.0, 2)];
        draws.sort();
        assert_eq!(draws.map(TransparentOrder::layer), [-1, 1, 2]);
    }

    #[test]
    fn equal_layer_depths_render_back_to_front() {
        let mut draws = [order(0, 1.0, 0), order(0, 8.0, 1), order(0, 3.0, 2)];
        draws.sort();
        assert_eq!(
            draws.map(|draw| draw.depth().to_bits()),
            [8.0_f32.to_bits(), 3.0_f32.to_bits(), 1.0_f32.to_bits()]
        );
    }

    #[test]
    fn submission_index_stabilizes_exact_ties() {
        let mut draws = [order(0, 2.0, 9), order(0, 2.0, 2), order(0, 2.0, 5)];
        draws.sort();
        assert_eq!(draws.map(TransparentOrder::submission_index), [2, 5, 9]);
    }

    #[test]
    fn rejects_non_finite_depths() {
        for depth in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                TransparentOrder::new(0, depth, 0),
                Err(TransparentOrderError::NonFiniteDepth)
            );
        }
    }

    #[test]
    fn signed_zero_has_one_canonical_order() {
        assert_eq!(order(0, -0.0, 0), order(0, 0.0, 0));
        assert_eq!(order(0, -0.0, 0).cmp(&order(0, 0.0, 0)), Ordering::Equal);
    }
}
