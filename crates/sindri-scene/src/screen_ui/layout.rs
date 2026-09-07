//! `sindri.ui.layout`: a parent that places its children in a line.
//!
//! Anchors already put a single element where it belongs, and a row of three
//! buttons can be authored as three offsets. What cannot be authored is what
//! happens when one of them is switched off: a hand-placed row leaves a hole,
//! and every entry below a hidden one is in the wrong place. Re-flowing over
//! the children that are actually there is the whole reason this exists.
//!
//! Layout spacing is measured between child **edges**, not their centres. That
//! makes the component useful for real boxes: a 0.1 gap between two 0.5-high
//! buttons leaves 0.1 of air instead of putting their centres 0.1 apart and
//! stacking most of one button on top of the other.

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
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct UiLayoutComponent {
    #[serde(default)]
    pub direction: UiDirection,
    /// Empty space between adjacent child edges, in overlay units.
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
    /// Placement for point-like children. With zero-sized boxes edge spacing is
    /// identical to the historical centre-spacing result.
    #[must_use]
    pub fn offset(self, index: usize, count: usize) -> [f32; 2] {
        self.offset_in_box(index, count, [0.0, 0.0], [0.0, 0.0])
    }

    /// Placement for equally sized children inside one parent box.
    #[must_use]
    pub fn offset_in_box(
        self,
        index: usize,
        count: usize,
        parent_size: [f32; 2],
        child_size: [f32; 2],
    ) -> [f32; 2] {
        if index >= count {
            return [0.0, 0.0];
        }
        let sizes = vec![child_size; count];
        self.offsets_in_box(parent_size, &sizes)
            .get(index)
            .copied()
            .unwrap_or([0.0, 0.0])
    }

    /// Resolve the whole sibling line from the parent and every child box.
    ///
    /// Resolving siblings together is what makes spacing mean edge-to-edge air
    /// for mixed sizes and lets `space_between` put the outside child edges on
    /// the parent edges without guessing a centre distance.
    #[must_use]
    pub fn offsets_in_box(self, parent_size: [f32; 2], child_sizes: &[[f32; 2]]) -> Vec<[f32; 2]> {
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
        let requested_gap = self.spacing.max(0.0);
        #[allow(clippy::cast_precision_loss)]
        let packed_span = children_span
            + requested_gap * child_sizes.len().saturating_sub(1) as f32;

        let actual_gap = if self.justify == UiJustify::SpaceBetween && child_sizes.len() > 1 {
            #[allow(clippy::cast_precision_loss)]
            let distributed = (parent_main - children_span).max(0.0)
                / child_sizes.len().saturating_sub(1) as f32;
            distributed.max(requested_gap)
        } else {
            requested_gap
        };
        #[allow(clippy::cast_precision_loss)]
        let actual_span = children_span
            + actual_gap * child_sizes.len().saturating_sub(1) as f32;

        let mut cursor = match self.justify {
            UiJustify::Start | UiJustify::SpaceBetween => -parent_main / 2.0,
            UiJustify::Center => -actual_span / 2.0,
            UiJustify::End => parent_main / 2.0 - actual_span,
        };

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
            justify: UiJustify::Center,
            align: UiAlign::Center,
        }
    }

    #[test]
    fn one_child_sits_on_its_parent() {
        assert_at(layout(UiDirection::Column).offset(0, 1), [0.0, 0.0]);
    }

    #[test]
    fn point_like_children_keep_the_old_spacing_geometry() {
        let column = layout(UiDirection::Column);
        assert_at(column.offset(0, 3), [0.0, 0.5]);
        assert_at(column.offset(1, 3), [0.0, 0.0]);
        assert_at(column.offset(2, 3), [0.0, -0.5]);

        let row = layout(UiDirection::Row);
        assert_at(row.offset(0, 3), [-0.5, 0.0]);
        assert_at(row.offset(2, 3), [0.5, 0.0]);
    }

    #[test]
    fn packed_row_spacing_is_between_edges() {
        let mut row = layout(UiDirection::Row);
        row.spacing = 0.25;
        let offsets = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [2.0, 0.5]]);
        assert_at(offsets[0], [-1.125, 0.0]);
        assert_at(offsets[1], [0.625, 0.0]);
        let first_right = offsets[0][0] + 0.5;
        let second_left = offsets[1][0] - 1.0;
        assert!((second_left - first_right - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn packed_column_spacing_prevents_overlap() {
        let mut column = layout(UiDirection::Column);
        column.spacing = 0.2;
        let offsets = column.offsets_in_box([2.0, 3.0], &[[1.0, 0.5], [1.0, 1.0]]);
        assert_at(offsets[0], [0.0, 0.6]);
        assert_at(offsets[1], [0.0, -0.35]);
        let first_bottom = offsets[0][1] - 0.25;
        let second_top = offsets[1][1] + 0.5;
        assert!((first_bottom - second_top - 0.2).abs() < 1.0e-5);
    }

    #[test]
    fn start_and_end_use_parent_edges_and_box_spacing() {
        let mut row = layout(UiDirection::Row);
        row.justify = UiJustify::Start;
        let start = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [1.0, 0.5]]);
        assert_at(start[0], [-1.5, 0.0]);
        assert_at(start[1], [0.0, 0.0]);

        row.justify = UiJustify::End;
        let end = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [1.0, 0.5]]);
        assert_at(end[0], [0.0, 0.0]);
        assert_at(end[1], [1.5, 0.0]);
    }

    #[test]
    fn space_between_spans_parent_edges_for_mixed_boxes() {
        let mut row = layout(UiDirection::Row);
        row.spacing = 0.1;
        row.justify = UiJustify::SpaceBetween;
        let offsets = row.offsets_in_box(
            [4.0, 2.0],
            &[[1.0, 0.5], [0.5, 0.5], [1.0, 0.5]],
        );
        assert_at(offsets[0], [-1.5, 0.0]);
        assert_at(offsets[2], [1.5, 0.0]);
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
}
