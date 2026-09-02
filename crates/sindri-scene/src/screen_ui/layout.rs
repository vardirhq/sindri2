//! `sindri.ui.layout`: a parent that places its children in a line.
//!
//! Anchors already put a single element where it belongs, and a row of three
//! buttons can be authored as three offsets. What cannot be authored is what
//! happens when one of them is switched off: a hand-placed row leaves a hole,
//! and every entry below a hidden one is in the wrong place. Re-flowing over
//! the children that are actually there is the whole reason this exists.
//!
//! There is no scroll. Nothing in the games this engine is being built against
//! has a list longer than a screen, and a scroll region invented before
//! something needs one is a shape chosen by guesswork — it would have to decide
//! about clipping, momentum, and where a pointer drag stops being a press,
//! none of which has an answer yet.

use serde::Deserialize;
use sindri_core::SceneComponent;

/// Which way a layout runs.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiDirection {
    /// Left to right.
    Row,
    /// Top to bottom, which is the order a menu reads in.
    #[default]
    Column,
}

impl UiDirection {
    /// Every direction, in the order a chooser should offer them.
    pub const ALL: [Self; 2] = [Self::Column, Self::Row];

    /// The name this direction is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
        }
    }
}

/// Places an entity's active children evenly along one axis.
///
/// The children keep their own anchors and their own sizes; what the layout
/// owns is their offset along its axis. A child's other axis is left alone, so
/// a row of differently-raised buttons stays that way.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiLayoutComponent {
    #[serde(default)]
    pub direction: UiDirection,
    /// The gap between one child's centre and the next, in overlay units.
    ///
    /// Centre to centre rather than edge to edge, because an element's drawn
    /// size is its texture's business and a layout that measured it would move
    /// when an artist re-exported a sprite.
    #[serde(default = "default_spacing")]
    pub spacing: f32,
}

const fn default_spacing() -> f32 {
    0.25
}

impl SceneComponent for UiLayoutComponent {
    const TYPE_NAME: &'static str = "sindri.ui.layout";
}

impl UiLayoutComponent {
    /// Where the `index`th of `count` children sits, relative to the parent.
    ///
    /// Centred on the parent as a whole, so a menu losing an entry closes up
    /// around its middle rather than growing downwards from its top — which is
    /// what a person reading a centred menu expects to see.
    #[must_use]
    pub fn offset(self, index: usize, count: usize) -> [f32; 2] {
        if count <= 1 {
            return [0.0, 0.0];
        }
        // `count - 1` gaps, so the first and last child sit half a span either
        // side of the parent.
        #[allow(clippy::cast_precision_loss)]
        let along = (index as f32 - (count - 1) as f32 / 2.0) * self.spacing;
        match self.direction {
            UiDirection::Row => [along, 0.0],
            // Negated: the overlay runs up and a menu reads down.
            UiDirection::Column => [0.0, -along],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UiDirection, UiLayoutComponent};

    /// Arrays of `f32` do not compare exactly, and should not: these are the
    /// results of arithmetic.
    #[track_caller]
    fn assert_at(got: [f32; 2], want: [f32; 2]) {
        assert!(
            (got[0] - want[0]).abs() < 1.0e-5 && (got[1] - want[1]).abs() < 1.0e-5,
            "{got:?} is not {want:?}"
        );
    }

    fn layout(direction: UiDirection) -> UiLayoutComponent {
        UiLayoutComponent {
            direction,
            spacing: 0.5,
        }
    }

    #[test]
    fn one_child_sits_on_its_parent() {
        assert_at(layout(UiDirection::Column).offset(0, 1), [0.0, 0.0]);
    }

    /// A menu reads downwards, so the first entry is the highest.
    #[test]
    fn a_column_runs_down_the_screen() {
        let column = layout(UiDirection::Column);
        assert_at(column.offset(0, 3), [0.0, 0.5]);
        assert_at(column.offset(1, 3), [0.0, 0.0]);
        assert_at(column.offset(2, 3), [0.0, -0.5]);
    }

    #[test]
    fn a_row_runs_across_it() {
        let row = layout(UiDirection::Row);
        assert_at(row.offset(0, 3), [-0.5, 0.0]);
        assert_at(row.offset(2, 3), [0.5, 0.0]);
    }

    /// The whole point: a menu that loses an entry closes up around its middle
    /// rather than leaving a hole.
    #[test]
    fn a_shorter_list_closes_up_around_the_same_middle() {
        let column = layout(UiDirection::Column);
        let three: Vec<f32> = (0..3).map(|i| column.offset(i, 3)[1]).collect();
        let two: Vec<f32> = (0..2).map(|i| column.offset(i, 2)[1]).collect();
        // Summing is enough: a list centred on nothing sums to nothing.
        let middle = |offsets: &[f32]| offsets.iter().sum::<f32>();
        assert!(middle(&three).abs() < 1.0e-5);
        assert!(middle(&two).abs() < 1.0e-5);
        assert!(two[0] < three[0], "two entries sit closer in");
    }

    #[test]
    fn an_even_count_straddles_the_middle() {
        let column = layout(UiDirection::Column);
        assert_at(column.offset(0, 2), [0.0, 0.25]);
        assert_at(column.offset(1, 2), [0.0, -0.25]);
    }
}
