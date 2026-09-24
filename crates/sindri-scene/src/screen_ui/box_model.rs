//! `sindri.ui.box`: the space around an element and inside it.
//!
//! The CSS box model, for the part of it a layout needs. **Padding** is room
//! kept free inside an element's edge, so a layout's children start inside it
//! rather than at the border. **Margin** is room kept free outside it, so a
//! layout leaves that much air between this element and its neighbours.
//!
//! Both are four numbers in CSS order — top, right, bottom, left — in overlay
//! units, like every other UI distance. An element with no box has none of
//! either, which is what every element had before this existed.

use serde::Deserialize;
use sindri_core::SceneComponent;

/// Four distances, one per side, in CSS order: top, right, bottom, left.
pub type UiSides = [f32; 4];

/// Top, right, bottom, left, as indices into [`UiSides`].
const TOP: usize = 0;
const RIGHT: usize = 1;
const BOTTOM: usize = 2;
const LEFT: usize = 3;

/// An element's margin and padding.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct UiBoxComponent {
    /// Room kept free outside the element, between it and its neighbours.
    #[serde(default)]
    pub margin: UiSides,
    /// Room kept free inside the element, between its edge and its children.
    #[serde(default)]
    pub padding: UiSides,
}

impl SceneComponent for UiBoxComponent {
    const TYPE_NAME: &'static str = "sindri.ui.box";
}

/// How much `sides` adds across and down: left plus right, top plus bottom.
#[must_use]
pub fn span(sides: UiSides) -> [f32; 2] {
    let sides = clean(sides);
    [sides[LEFT] + sides[RIGHT], sides[TOP] + sides[BOTTOM]]
}

/// Where the middle of what `sides` leaves moves to, from the middle of the
/// whole. Up is positive, as everywhere in the overlay.
#[must_use]
pub fn shift(sides: UiSides) -> [f32; 2] {
    let sides = clean(sides);
    [
        (sides[LEFT] - sides[RIGHT]) / 2.0,
        (sides[BOTTOM] - sides[TOP]) / 2.0,
    ]
}

/// Sides with nothing negative or not a number in them. A negative padding
/// has no meaning, and a negative margin (which CSS allows) is not supported
/// yet, so both are read as none rather than as a box turned inside out.
fn clean(sides: UiSides) -> UiSides {
    sides.map(|side| if side.is_finite() { side.max(0.0) } else { 0.0 })
}

#[cfg(test)]
// Every number here is exact in binary, so an exact comparison is the honest one.
#[allow(clippy::float_cmp)]
mod tests {
    use super::{UiBoxComponent, shift, span};

    #[test]
    fn sides_run_top_right_bottom_left() {
        let sides = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(span(sides), [6.0, 4.0]);
        // More on the left pushes the middle right; more below pushes it up.
        assert_eq!(shift(sides), [1.0, 1.0]);
    }

    #[test]
    fn a_box_with_nothing_said_has_no_room_either_side() {
        let empty: UiBoxComponent = serde_json::from_str("{}").expect("all defaulted");
        assert_eq!(empty, UiBoxComponent::default());
        assert_eq!(span(empty.padding), [0.0, 0.0]);
    }

    #[test]
    fn negative_and_broken_sides_count_as_none() {
        assert_eq!(span([-1.0, f32::NAN, 0.5, f32::INFINITY]), [0.0, 0.5]);
    }
}
