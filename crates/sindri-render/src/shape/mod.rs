//! Shapes drawn as distance fields: the vector half of what a scene can draw.
//!
//! A sprite is a picture someone made. That is right for a character and wrong
//! for a great deal of what a game actually puts on screen: a ring whose radius
//! is an upgrade, a cooldown arc that fills, a shockwave that expands, a dashed
//! marker that turns, a grid under everything. Each of those is a *number* the
//! game already has, and drawing it from a texture means re-exporting art every
//! time the number's range changes — or scaling one picture until its lines go
//! soft.
//!
//! So these are evaluated rather than sampled. The quad carries what the shape
//! is and the fragment shader measures the distance to its edge, which means one
//! antialiased line at any size, no atlas, no import step, and modifiers that
//! compose: a ring is a circle with no fill, an arc is a ring with a sweep, a
//! dashed ring is an arc with a duty cycle. It is the same technique the glyphs
//! use, and for the same reason.
//!
//! What it is not: a general vector renderer. There are no paths, no béziers and
//! no fills with holes. The kinds here are the ones a game HUD and a
//! twin-stick's cast of shapes are actually built from, and a shape outside them
//! is still a sprite.

mod instance;
mod render;

pub use instance::ShapeInstance;
pub use render::{ShapeBlend, ShapeDrawError, ShapeRenderer};

/// What a shape is, before anything is done to it.
///
/// Deliberately few. Rings, arcs, dashes and progress are *modifiers* on these
/// rather than kinds of their own, because they compose — and a kind per
/// combination is how a list like this reaches thirty entries and stops being
/// readable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle filling the quad, optionally with rounded corners.
    Rect,
    /// An ellipse inscribed in the quad — a circle in a square one.
    Ellipse,
    /// A regular polygon of this many sides, inscribed in the quad, with a
    /// vertex at the top.
    Polygon { sides: f32 },
    /// A lattice of this many cells across the quad, drawn as lines.
    ///
    /// Lines rather than an enclosure: it has no inside, so it takes a stroke
    /// and ignores a fill.
    Grid { cells: f32 },
}

impl Shape {
    /// Which kind the shader should evaluate.
    pub(super) fn tag(self) -> f32 {
        match self {
            Self::Rect => 0.0,
            Self::Ellipse => 1.0,
            Self::Polygon { .. } => 2.0,
            Self::Grid { .. } => 3.0,
        }
    }

    /// The one number that kind needs, if it needs one.
    pub(super) fn parameter(self) -> f32 {
        match self {
            // A polygon below three sides encloses nothing and a grid below one
            // cell has no lines in it. Clamped here rather than in the shader so
            // that what was asked for and what is drawn differ in one place.
            Self::Polygon { sides } => sides.max(3.0),
            Self::Grid { cells } => cells.max(1.0),
            Self::Rect | Self::Ellipse => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Shape;

    /// A polygon needs three sides to enclose anything and a grid needs one
    /// cell to have a line in it.
    ///
    /// Clamped where the shape is described rather than in the shader, so that
    /// what was asked for and what is drawn differ in one place — and so a
    /// two-sided polygon is a flat degenerate rather than a shape whose SDF
    /// divides by a segment of a whole turn.
    #[test]
    fn a_degenerate_count_is_clamped_to_the_smallest_real_one() {
        for (asked, drawn) in [
            (Shape::Polygon { sides: 0.0 }, 3.0),
            (Shape::Polygon { sides: 2.0 }, 3.0),
            (Shape::Polygon { sides: 7.0 }, 7.0),
            (Shape::Grid { cells: 0.0 }, 1.0),
            (Shape::Grid { cells: 32.0 }, 32.0),
        ] {
            assert!(
                (asked.parameter() - drawn).abs() < 1.0e-6,
                "{asked:?} should draw as {drawn}"
            );
        }
    }

    /// Every kind has its own tag, or two of them would draw as one.
    #[test]
    fn the_kinds_are_told_apart() {
        let tags = [
            Shape::Rect.tag(),
            Shape::Ellipse.tag(),
            Shape::Polygon { sides: 6.0 }.tag(),
            Shape::Grid { cells: 8.0 }.tag(),
        ];
        for (index, tag) in tags.iter().enumerate() {
            for other in &tags[index + 1..] {
                assert!((tag - other).abs() > 0.5, "{tag} and {other} collide");
            }
        }
    }
}
