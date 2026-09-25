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

use super::UiBoxComponent;
use super::box_model::UiSides;
use super::flex;

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
    /// Equal room around each child: half as much at the ends as between.
    SpaceAround,
    /// Equal room between children and at both ends.
    SpaceEvenly,
}

impl UiJustify {
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Center,
        Self::End,
        Self::SpaceBetween,
        Self::SpaceAround,
        Self::SpaceEvenly,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::SpaceBetween => "space_between",
            Self::SpaceAround => "space_around",
            Self::SpaceEvenly => "space_evenly",
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
    /// Fill the parent's cross axis while preserving main-axis size.
    Stretch,
}

impl UiAlign {
    pub const ALL: [Self; 4] = [Self::Start, Self::Center, Self::End, Self::Stretch];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }
}

/// One child's resolved box inside a parent layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiLayoutBox {
    pub offset: [f32; 2],
    pub size: [f32; 2],
}

/// One child as its parent's layout sees it: its own size, and its box —
/// the margin it keeps and how it grows, shrinks, orders and aligns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiLayoutChild {
    pub size: [f32; 2],
    pub item: UiBoxComponent,
    /// The least it can shrink to without cutting into its own content,
    /// CSS's automatic minimum. A layout's is its children and padding;
    /// anything else's is zero, because nothing here measures text yet.
    pub min_content: [f32; 2],
}

impl UiLayoutChild {
    /// A child with nothing to say about itself but its size.
    #[must_use]
    pub fn sized(size: [f32; 2]) -> Self {
        Self {
            size,
            item: UiBoxComponent::default(),
            min_content: [0.0; 2],
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
    /// Whether children that do not fit start a new line, as `flex-wrap`.
    #[serde(default)]
    pub wrap: bool,
    /// Whether the element sizes itself to its children, across and down,
    /// rather than keeping its own size: `width: auto` and `height: auto`.
    #[serde(default)]
    pub fit_content: [bool; 2],
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
    #[must_use]
    pub fn resolve_in_box(
        self,
        parent_size: [f32; 2],
        child_sizes: &[[f32; 2]],
    ) -> Vec<UiLayoutBox> {
        let children: Vec<UiLayoutChild> = child_sizes
            .iter()
            .copied()
            .map(UiLayoutChild::sized)
            .collect();
        self.resolve_boxes(parent_size, [0.0; 4], &children)
    }

    /// Resolve the children with the box model and flexbox: they flow inside
    /// the parent's `padding`, keep their margins, and grow, shrink, wrap,
    /// order and align as their boxes say. See [`flex`].
    #[must_use]
    pub fn resolve_boxes(
        self,
        parent_size: [f32; 2],
        padding: UiSides,
        children: &[UiLayoutChild],
    ) -> Vec<UiLayoutBox> {
        flex::resolve(&self, parent_size, padding, children)
    }

    /// The size this layout would be to hold `children` exactly, padding
    /// included, for an element that fits its content.
    #[must_use]
    pub fn content_size(self, padding: UiSides, children: &[UiLayoutChild]) -> [f32; 2] {
        flex::content_size(&self, padding, children)
    }

    /// Resolve only the child offsets for callers that do not need sizing.
    #[must_use]
    pub fn offsets_in_box(self, parent_size: [f32; 2], child_sizes: &[[f32; 2]]) -> Vec<[f32; 2]> {
        self.resolve_in_box(parent_size, child_sizes)
            .into_iter()
            .map(|child| child.offset)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{UiAlign, UiDirection, UiJustify, UiLayoutChild, UiLayoutComponent};

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
            wrap: false,
            fit_content: [false; 2],
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
        let offsets = row.offsets_in_box([4.0, 2.0], &[[1.0, 0.5], [0.5, 0.5], [1.0, 0.5]]);
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

    #[test]
    fn stretch_fills_only_the_cross_axis() {
        let mut row = layout(UiDirection::Row);
        row.align = UiAlign::Stretch;
        let resolved = row.resolve_in_box([4.0, 2.0], &[[1.0, 0.5], [0.5, 1.0]]);
        assert_at(resolved[0].size, [1.0, 2.0]);
        assert_at(resolved[1].size, [0.5, 2.0]);
        assert_at(resolved[0].offset, [-0.5, 0.0]);
        assert_at(resolved[1].offset, [0.75, 0.0]);
    }

    #[test]
    fn children_start_inside_the_padding() {
        let mut column = layout(UiDirection::Column);
        column.justify = UiJustify::Start;
        column.align = UiAlign::Start;
        let child = UiLayoutChild::sized([1.0, 0.5]);
        // A 4 x 2 panel with 0.25 above and 0.5 to the left.
        let placed = column.resolve_boxes([4.0, 2.0], [0.25, 0.0, 0.0, 0.5], &[child]);
        // Its top-left corner sits exactly at the padding's inner corner.
        let top = placed[0].offset[1] + 0.25;
        let left = placed[0].offset[0] - 0.5;
        assert_at([left, top], [-2.0 + 0.5, 1.0 - 0.25]);
    }

    #[test]
    fn margins_add_to_the_gap_between_neighbours() {
        let mut row = layout(UiDirection::Row);
        row.spacing = 0.1;
        let plain = UiLayoutChild::sized([1.0, 1.0]);
        let mut spaced = plain;
        spaced.item.margin = [0.0, 0.0, 0.0, 0.3];
        let placed = row.resolve_boxes([4.0, 2.0], [0.0; 4], &[plain, spaced]);
        let first_right = placed[0].offset[0] + 0.5;
        let second_left = placed[1].offset[0] - 0.5;
        assert!(
            (second_left - first_right - 0.4).abs() < 1.0e-5,
            "{placed:?}"
        );
    }

    #[test]
    fn stretch_fills_the_content_box_less_the_childs_margins() {
        let mut row = layout(UiDirection::Row);
        row.align = UiAlign::Stretch;
        let mut child = UiLayoutChild::sized([1.0, 0.5]);
        child.item.margin = [0.1, 0.0, 0.2, 0.0];
        let placed = row.resolve_boxes([4.0, 2.0], [0.25, 0.25, 0.25, 0.25], &[child]);
        // 2 high, less 0.5 of padding, less 0.3 of margin.
        assert_at(placed[0].size, [1.0, 1.2]);
        // 0.1 of margin above and 0.2 below, so its middle sits 0.05 above
        // the content's middle.
        assert_at(placed[0].offset, [0.0, 0.05]);
    }
}
