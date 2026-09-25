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
//!
//! The rest is how the element behaves as an item in its parent's layout,
//! CSS flexbox's item properties: how it grows into spare room and shrinks
//! when there is too little, the size it starts from, where it comes in the
//! order, how it aligns across the line, and the limits on its size.

use serde::Deserialize;
use sindri_core::SceneComponent;

/// Four distances, one per side, in CSS order: top, right, bottom, left.
pub type UiSides = [f32; 4];

/// How one item sits across its line, overriding its layout's `align`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiAlignSelf {
    /// Whatever the layout says, which is what an item says nothing about.
    #[default]
    Auto,
    Start,
    Center,
    End,
    Stretch,
}

impl UiAlignSelf {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Start,
        Self::Center,
        Self::End,
        Self::Stretch,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }
}

/// An element's margin and padding, and how it behaves in a layout.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiBoxComponent {
    /// Room kept free outside the element, between it and its neighbours.
    #[serde(default)]
    pub margin: UiSides,
    /// Room kept free inside the element, between its edge and its children.
    #[serde(default)]
    pub padding: UiSides,
    /// How much of a line's spare room this element takes, against its
    /// neighbours' shares. Zero keeps its size.
    #[serde(default)]
    pub grow: f32,
    /// How much of a line's shortfall this element gives up, weighted by its
    /// size. One by default, as in CSS; zero keeps its size.
    #[serde(default = "one")]
    pub shrink: f32,
    /// The size along the line it starts from before growing or shrinking.
    /// Negative is `auto`: its own size.
    #[serde(default = "auto")]
    pub basis: f32,
    /// Where it comes in its layout: lower first, ties in scene order.
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub align_self: UiAlignSelf,
    /// The smallest it may be made, across and down.
    #[serde(default)]
    pub min_size: [f32; 2],
    /// The largest it may be made, across and down. Zero is no limit.
    #[serde(default)]
    pub max_size: [f32; 2],
    /// Whether the element sizes itself to what it holds, across and down:
    /// CSS's `width: auto` on something with content of its own. For a text
    /// element that is its measured words, and its padding; a layout says the
    /// same for its children with its own `fit_content`.
    #[serde(default)]
    pub fit_content: [bool; 2],
}

const fn one() -> f32 {
    1.0
}

const fn auto() -> f32 {
    -1.0
}

impl Default for UiBoxComponent {
    fn default() -> Self {
        Self {
            margin: [0.0; 4],
            padding: [0.0; 4],
            grow: 0.0,
            shrink: one(),
            basis: auto(),
            order: 0,
            align_self: UiAlignSelf::Auto,
            min_size: [0.0; 2],
            max_size: [0.0; 2],
            fit_content: [false; 2],
        }
    }
}

impl UiBoxComponent {
    /// `size` held within this element's limits on `axis`.
    #[must_use]
    pub fn clamp(&self, axis: usize, size: f32) -> f32 {
        let minimum = finite_or_zero(self.min_size[axis]).max(0.0);
        let maximum = finite_or_zero(self.max_size[axis]);
        let mut size = finite_or_zero(size).max(minimum);
        if maximum > 0.0 {
            // As in CSS, the minimum wins a conflict.
            size = size.min(maximum.max(minimum));
        }
        size
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

impl SceneComponent for UiBoxComponent {
    const TYPE_NAME: &'static str = "sindri.ui.box";
}

/// Sides with nothing negative or not a number in them. A negative padding
/// has no meaning, and a negative margin (which CSS allows) is not supported
/// yet, so both are read as none rather than as a box turned inside out.
pub(crate) fn clean(sides: UiSides) -> UiSides {
    sides.map(|side| if side.is_finite() { side.max(0.0) } else { 0.0 })
}

#[cfg(test)]
// Every number here is exact in binary, so an exact comparison is the honest one.
#[allow(clippy::float_cmp)]
mod tests {
    use super::{UiBoxComponent, clean};

    #[test]
    fn limits_hold_a_size_and_the_minimum_wins() {
        let limited = UiBoxComponent {
            min_size: [0.5, 0.0],
            max_size: [1.0, 0.25],
            ..UiBoxComponent::default()
        };
        assert_eq!(limited.clamp(0, 2.0), 1.0);
        assert_eq!(limited.clamp(0, 0.1), 0.5);
        assert_eq!(limited.clamp(1, 3.0), 0.25);
        let conflicted = UiBoxComponent {
            min_size: [2.0, 0.0],
            max_size: [1.0, 0.0],
            ..UiBoxComponent::default()
        };
        assert_eq!(conflicted.clamp(0, 0.0), 2.0);
    }

    #[test]
    fn a_box_with_nothing_said_has_no_room_either_side() {
        let empty: UiBoxComponent = serde_json::from_str("{}").expect("all defaulted");
        assert_eq!(empty, UiBoxComponent::default());
        assert_eq!(empty.padding, [0.0; 4]);
    }

    #[test]
    fn negative_and_broken_sides_count_as_none() {
        assert_eq!(
            clean([-1.0, f32::NAN, 0.5, f32::INFINITY]),
            [0.0, 0.0, 0.5, 0.0]
        );
    }
}
