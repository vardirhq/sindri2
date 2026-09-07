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

/// Where a line of children sits along the layout's main axis.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiJustify {
    Start,
    #[default]
    Center,
    End,
    SpaceBetween,
}

impl UiJustify {
    pub const ALL: [Self; 4] = [Self::Start, Self::Center, Self::End, Self::SpaceBetween];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::SpaceBetween => "space_between",
        }
    }
}

/// Where children sit on the axis perpendicular to the layout direction.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UiAlign {
    Start,
    #[default]
    Center,
    End,
}

impl UiAlign {
    pub const ALL: [Self; 3] = [Self::Start, Self::Center, Self::End];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

/// Places an entity's active children along one axis.
///
/// Existing scenes default to centred placement, preserving the original layout
/// behavior. The optional box-aware path lets responsive UI place that same line
/// against the start/end of a parent or spread it across the available span.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiLayoutComponent {
    #[serde(default)]
    pub direction: UiDirection,
    /// The distance between child centres in overlay units for packed layouts.
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    #[serde(default)]
    pub justify: UiJustify,
    #[serde(default)]
    pub align: UiAlign,
}

const fn default_spacing() -> f32 {
    0.25
}

impl SceneComponent for UiLayoutComponent {
    const TYPE_NAME: &'static str = "sindri.ui.layout";
}

impl UiLayoutComponent {
    /// Legacy centred placement, independent of parent/child bounds.
    #[must_use]
    pub fn offset(self, index: usize, count: usize) -> [f32; 2] {
        self.offset_in_box(index, count, [0.0, 0.0], [0.0, 0.0])
    }

    /// Placement relative to a parent box, using the child's own size at the
    /// edges so `start`, `end`, and `space_between` do not push it outside.
    #[must_use]
    pub fn offset_in_box(
        self,
        index: usize,
        count: usize,
        parent_size: [f32; 2],
        child_size: [f32; 2],
    ) -> [f32; 2] {
        let main_axis = usize::from(self.direction == UiDirection::Column);
        let cross_axis = 1 - main_axis;
        let parent_main = parent_size[main_axis].abs();
        let child_main = child_size[main_axis].abs();
        let parent_cross = parent_size[cross_axis].abs();
        let child_cross = child_size[cross_axis].abs();

        let packed = if count <= 1 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                (index as f32 - (count - 1) as f32 / 2.0) * self.spacing
            }
        };
        let edge = ((parent_main - child_main).max(0.0)) / 2.0;

        let logical_main = match self.justify {
            UiJustify::Center => packed,
            UiJustify::Start => {
                #[allow(clippy::cast_precision_loss)]
                {
                    -edge + index as f32 * self.spacing
                }
            }
            UiJustify::End => {
                #[allow(clippy::cast_precision_loss)]
                {
                    edge - (count.saturating_sub(1) - index) as f32 * self.spacing
                }
            }
            UiJustify::SpaceBetween if count > 1 => {
                #[allow(clippy::cast_precision_loss)]
                let t = index as f32 / (count - 1) as f32;
                -edge + t * edge * 2.0
            }
            UiJustify::SpaceBetween => 0.0,
        };

        let cross_edge = ((parent_cross - child_cross).max(0.0)) / 2.0;
        let logical_cross = match self.align {
            UiAlign::Start => -cross_edge,
            UiAlign::Center => 0.0,
            UiAlign::End => cross_edge,
        };

        match self.direction {
            UiDirection::Row => [logical_main, -logical_cross],
            // Main-axis start for a column is visually at the top.
            UiDirection::Column => [logical_cross, -logical_main],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UiAlign, UiDirection, UiJustify, UiLayoutComponent};

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
            justify: UiJustify::Center,
            align: UiAlign::Center,
        }
    }

    #[test]
    fn one_child_sits_on_its_parent() {
        assert_at(layout(UiDirection::Column).offset(0, 1), [0.0, 0.0]);
    }

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

    #[test]
    fn start_and_end_use_parent_edges() {
        let mut row = layout(UiDirection::Row);
        row.justify = UiJustify::Start;
        assert_at(row.offset_in_box(0, 2, [4.0, 2.0], [1.0, 0.5]), [-1.5, 0.0]);
        assert_at(row.offset_in_box(1, 2, [4.0, 2.0], [1.0, 0.5]), [-1.0, 0.0]);

        row.justify = UiJustify::End;
        assert_at(row.offset_in_box(0, 2, [4.0, 2.0], [1.0, 0.5]), [1.0, 0.0]);
        assert_at(row.offset_in_box(1, 2, [4.0, 2.0], [1.0, 0.5]), [1.5, 0.0]);
    }

    #[test]
    fn space_between_spans_the_available_box() {
        let mut row = layout(UiDirection::Row);
        row.justify = UiJustify::SpaceBetween;
        assert_at(row.offset_in_box(0, 3, [4.0, 2.0], [1.0, 0.5]), [-1.5, 0.0]);
        assert_at(row.offset_in_box(1, 3, [4.0, 2.0], [1.0, 0.5]), [0.0, 0.0]);
        assert_at(row.offset_in_box(2, 3, [4.0, 2.0], [1.0, 0.5]), [1.5, 0.0]);
    }

    #[test]
    fn cross_axis_alignment_uses_child_bounds() {
        let mut column = layout(UiDirection::Column);
        column.align = UiAlign::Start;
        assert_at(column.offset_in_box(0, 1, [4.0, 2.0], [1.0, 0.5]), [-1.5, 0.0]);
        column.align = UiAlign::End;
        assert_at(column.offset_in_box(0, 1, [4.0, 2.0], [1.0, 0.5]), [1.5, 0.0]);
    }

    #[test]
    fn an_even_count_straddles_the_middle() {
        let column = layout(UiDirection::Column);
        assert_at(column.offset(0, 2), [0.0, 0.25]);
        assert_at(column.offset(1, 2), [0.0, -0.25]);
    }
}