//! What a grid refuses, and why.

use std::fmt;

use crate::{GridPoint, PlanePoint};

/// A projection input that cannot be represented safely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridError {
    InvalidCellWidth(f64),
    InvalidCellHeight(f64),
    NonFinitePoint(PlanePoint),
    NonFiniteGridPoint(GridPoint),
    CoordinateOutsideIntegerRange(f64),
    BoundsOutsideIntegerRange { width: u32, height: u32 },
    ProjectionOverflow,
}

impl fmt::Display for GridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCellWidth(width) => {
                write!(
                    formatter,
                    "grid cell width must be finite and positive, got {width}"
                )
            }
            Self::InvalidCellHeight(height) => {
                write!(
                    formatter,
                    "grid cell height must be finite and positive, got {height}"
                )
            }
            Self::NonFinitePoint(point) => write!(
                formatter,
                "plane point must be finite, got ({}, {})",
                point.x, point.y
            ),
            Self::NonFiniteGridPoint(point) => write!(
                formatter,
                "grid point must be finite, got ({}, {})",
                point.x, point.y
            ),
            Self::CoordinateOutsideIntegerRange(value) => {
                write!(
                    formatter,
                    "grid coordinate {value} is outside the i32 range"
                )
            }
            Self::BoundsOutsideIntegerRange { width, height } => write!(
                formatter,
                "grid bounds {width}x{height} exceed the signed coordinate range"
            ),
            Self::ProjectionOverflow => formatter.write_str("grid projection overflowed f64"),
        }
    }
}

impl std::error::Error for GridError {}
