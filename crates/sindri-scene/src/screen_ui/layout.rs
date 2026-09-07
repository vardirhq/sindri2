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
/// `spacing` is the original centre-to-centre distance and remains the default
/// for existing scenes. `gap` is an edge-to-edge distance: when present the
/// layout resolves the whole line from every child's box so differently sized
/// children cannot overlap merely because the requested gap is smaller than
/// either child.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiLayoutComponent {
    #[serde(default)]
    pub direction: UiDirection,
    /// Legacy distance between child centres in overlay units.
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    /// Optional edge-to-edge distance between adjacent child boxes.
    #[serde(default)]
    pub gap: Option<f32>,
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

    /// Legacy placement relative to a parent box.
    ///
    /// This keeps the historical centre-spacing behavior for authored scenes
    /// that do not opt into box-aware `gap` layout.
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

        self.finish_offset(logical_main, parent_cross, child_cross)
    }

    /// Resolve every child together, using their actual boxes when `gap` is set.
    ///
    /// A CSS-like gap is space *between edges*, not between centres. Resolving
    /// siblings as a group is therefore required for mixed child sizes and for
    /// correct start/end/space-between placement.
    #[must_use]
    pub fn offsets_in_box(self, parent_size: [f32; 2], child_sizes: &[[f32; 2]]) -> Vec<[f32; 2]> {
        let Some(gap) = self.gap else {
            return child_sizes
                .iter()
                .enumerate()
                .map(|(index, child_size)| {
                    self.offset_in_box(index, child_sizes.len(), parent_size, *child_size)
                })
                .collect();
        };
        if child_sizes.is_empty() {
            return Vec::new();
        }

        let main_axis = usize::from(self.direction == UiDirection::Column);
        let cross_axis = 1 - main_axis;
        let parent_main = parent_size[main_axis].abs();
        let parent_cross = parent_size[cross_axis].abs();
        let main_sizes: Vec<f32> = child_sizes
            .iter()
            .map(|size| size[main_axis].abs())
            .collect();
        let children_span: f32 = main_sizes.iter().sum();
        let requested_gap = gap.max(0.0);
        #[allow(clippy::cast_precision_loss)]
        let gap_span = requested_gap * child_sizes.len().saturating_sub(1) as f32;
        let packed_span = children_span + gap_span;

        let actual_gap = if self.justify == UiJustify::SpaceBetween && child_sizes.len() > 1 {
            #[allow(clippy::cast_precision_loss)]
            let distributed = (parent_main - children_span).max(0.0)
                / child_sizes.len().saturating_sub(1) as f32;
            distributed.max(requested_gap)
        } else {
            requested_gap
        };
        #[allow(clippy::cast_precision_loss)]
        let actual_span = children_span + actual_gap * child_sizes.len().saturating_sub(1) as f32;

        let mut cursor = match self.justify {
            UiJustify::Start | UiJustify::SpaceBetween => -parent_main / 2.0,
            UiJustify::Center => -packed_span / 2.0,
            UiJustify::End => parent_main / 2.0 - packed_span,
        };
        if self.justify == UiJustify::Center && actual_span != packed_span {
            cursor = -actual_span / 2.0;
        }
        if self.justify == UiJustify::End && actual_span != packed_span {
            cursor = parent_main / 2.0 - actual_span;
        }

        child_sizes
            .iter()
            .zip(main_sizes)
            .map(|(child_size, child_main)| {
                let logical_main = cursor + child_main / 2.0;
                cursor += child_main + actual_gap;
                self.finish_offset(
                    logical_main,
                    parent_cross,
                    child_size[cross_axis].abs(),
                )
            })
            .collect()
    }

    fn finish_offset(self, logical_main: f32, parent_cross: f32, child_cross: f32) -> [f32; 2] {
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
            gap: None,
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
        assert_at(
            column.offset_in_box(0, 1, [4.0, 2.0], [1.0, 0.5]),
            [-1.5, 0.0],
        );
        column.align = UiAlign::End;
        assert_at(
            column.offset_in_box(0, 1, [4.0, 2.0], [1.0, 0.5]),
            [1.5, 0.0],
        );
    }

    #[test]
    fn an_even_count_straddles_the_middle() {
        let column = layout(UiDirection::Column);
        assert_at(column.offset(0, 2), [0.0, 0.25]);
        assert_at(column.offset(1, 2), [0.0, -0.25]);
    }

    #[test]
    fn box_gap_keeps_edges_apart_for_mixed_child_sizes() {
        let mut row = layout(UiDirection::Row);
        row.gap = Some(0.25);
        let offsets = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [2.0, 0.5]]);
        assert_at(offsets[0], [-1.125, 0.0]);
        assert_at(offsets[1], [0.625, 0.0]);
        let first_right = offsets[0][0] + 0.5;
        let second_left = offsets[1][0] - 1.0;
        assert!((second_left - first_right - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn box_gap_column_stacks_children_without_overlap() {
        let mut column = layout(UiDirection::Column);
        column.gap = Some(0.2);
        let offsets = column.offsets_in_box([2.0, 3.0], &[[1.0, 0.5], [1.0, 1.0]]);
        assert_at(offsets[0], [0.0, 0.6]);
        assert_at(offsets[1], [0.0, -0.35]);
        let first_bottom = offsets[0][1] - 0.25;
        let second_top = offsets[1][1] + 0.5;
        assert!((first_bottom - second_top - 0.2).abs() < 1.0e-5);
    }

    #[test]
    fn box_gap_space_between_uses_parent_edges_and_child_boxes() {
        let mut row = layout(UiDirection::Row);
        row.gap = Some(0.1);
        row.justify = UiJustify::SpaceBetween;
        let offsets = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [0.5, 0.5], [1.0, 0.5]]);
        assert_at(offsets[0], [-1.5, 0.0]);
        assert_at(offsets[2], [1.5, 0.0]);
    }
}
