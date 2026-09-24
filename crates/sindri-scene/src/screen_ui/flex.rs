//! The flexbox algorithm behind `sindri.ui.layout`.
//!
//! CSS's, in the single pass a game UI needs. Children are put in `order`,
//! broken into lines if the layout wraps, and each line's spare room is shared
//! out by `grow` or its shortfall taken back by `shrink` (weighted by size, as
//! CSS weights it), within each child's limits. Then the line is justified,
//! and each child aligned across it, stretched if asked to be.
//!
//! What CSS does and this does not: limits are applied once rather than by
//! freezing a clamped item and sharing its remainder again, and an item has
//! no minimum from its content, because nothing here measures content yet.
//!
//! Everything is worked in a *logical* frame first — along the line from its
//! start, and across it from the first line — then turned into overlay
//! offsets, where up is positive and a column starts at the top.

use super::UiAlignSelf;
use super::box_model::{UiSides, clean};
use super::layout::{
    UiAlign, UiDirection, UiJustify, UiLayoutBox, UiLayoutChild, UiLayoutComponent,
};

const TOP: usize = 0;
const RIGHT: usize = 1;
const BOTTOM: usize = 2;
const LEFT: usize = 3;

/// A sliver of tolerance for "does it still fit on this line", so a row that
/// is exactly full does not wrap its last item over a rounding error.
const FIT_EPSILON: f32 = 1.0e-5;

/// The two axes of a layout and which sides of a box are at each end of them.
struct Axes {
    direction: UiDirection,
    main: usize,
    cross: usize,
    main_start: usize,
    main_end: usize,
    cross_start: usize,
    cross_end: usize,
}

impl Axes {
    fn of(direction: UiDirection) -> Self {
        match direction {
            UiDirection::Row => Self {
                direction,
                main: 0,
                cross: 1,
                main_start: LEFT,
                main_end: RIGHT,
                cross_start: TOP,
                cross_end: BOTTOM,
            },
            UiDirection::Column => Self {
                direction,
                main: 1,
                cross: 0,
                main_start: TOP,
                main_end: BOTTOM,
                cross_start: LEFT,
                cross_end: RIGHT,
            },
        }
    }

    fn along(&self, sides: UiSides) -> f32 {
        sides[self.main_start] + sides[self.main_end]
    }

    fn across(&self, sides: UiSides) -> f32 {
        sides[self.cross_start] + sides[self.cross_end]
    }

    /// A logical position as an overlay offset from the parent's middle.
    fn offset(&self, main: f32, cross: f32) -> [f32; 2] {
        match self.direction {
            UiDirection::Row => [main, -cross],
            UiDirection::Column => [cross, -main],
        }
    }
}

/// Each child's box, in the order the children were given.
pub(super) fn resolve(
    layout: &UiLayoutComponent,
    parent_size: [f32; 2],
    padding: UiSides,
    children: &[UiLayoutChild],
) -> Vec<UiLayoutBox> {
    if children.is_empty() {
        return Vec::new();
    }
    let axes = Axes::of(layout.direction);
    let padding = clean(padding);
    let parent_main = parent_size[axes.main].abs();
    let parent_cross = parent_size[axes.cross].abs();
    let content_main = (parent_main - axes.along(padding)).max(0.0);
    let content_cross = (parent_cross - axes.across(padding)).max(0.0);
    let main_origin = -parent_main / 2.0 + padding[axes.main_start];
    let cross_origin = -parent_cross / 2.0 + padding[axes.cross_start];
    let gap = layout.spacing.max(0.0);

    let margins: Vec<UiSides> = children
        .iter()
        .map(|child| clean(child.item.margin))
        .collect();
    let base: Vec<f32> = children
        .iter()
        .map(|child| {
            let item = &child.item;
            let size = if item.basis >= 0.0 {
                item.basis
            } else {
                child.size[axes.main].abs()
            };
            item.clamp(axes.main, size)
        })
        .collect();
    let outer_base = |index: usize| base[index] + axes.along(margins[index]);

    let mut sequence: Vec<usize> = (0..children.len()).collect();
    sequence.sort_by_key(|index| children[*index].item.order);
    let lines = break_lines(&sequence, layout.wrap, content_main, gap, outer_base);

    let mut main_size = base.clone();
    for line in &lines {
        flex_line(
            line,
            children,
            &base,
            content_main,
            gap,
            &axes,
            &margins,
            &mut main_size,
        );
    }

    let cross_size: Vec<f32> = children
        .iter()
        .map(|child| child.item.clamp(axes.cross, child.size[axes.cross].abs()))
        .collect();
    let line_cross = line_extents(&lines, layout.wrap, content_cross, gap, |index| {
        cross_size[index] + axes.across(margins[index])
    });

    let mut boxes = vec![
        UiLayoutBox {
            offset: [0.0; 2],
            size: [0.0; 2],
        };
        children.len()
    ];
    let frame = Frame {
        layout,
        axes: &axes,
        children,
        margins: &margins,
        main_size: &main_size,
        cross_size: &cross_size,
        main_origin,
        content_main,
        gap,
    };
    let mut cross_cursor = cross_origin;
    for (line, extent) in lines.iter().zip(line_cross) {
        frame.place_line(line, cross_cursor, extent, &mut boxes);
        cross_cursor += extent + gap;
    }
    boxes
}

/// Everything placing a line needs, worked out once for the whole layout.
struct Frame<'a> {
    layout: &'a UiLayoutComponent,
    axes: &'a Axes,
    children: &'a [UiLayoutChild],
    margins: &'a [UiSides],
    main_size: &'a [f32],
    cross_size: &'a [f32],
    main_origin: f32,
    content_main: f32,
    gap: f32,
}

impl Frame<'_> {
    /// Justifies one line along its length and aligns each child across it.
    fn place_line(&self, line: &[usize], cross_start: f32, extent: f32, boxes: &mut [UiLayoutBox]) {
        let axes = self.axes;
        let used: f32 = line
            .iter()
            .map(|index| self.main_size[*index] + axes.along(self.margins[*index]))
            .sum::<f32>()
            + self.gap * gaps_in(line.len());
        let (mut cursor, between) = justify(
            self.layout.justify,
            self.content_main - used,
            line.len(),
            self.gap,
        );
        for index in line.iter().copied() {
            let item = &self.children[index].item;
            let margin = self.margins[index];
            let along = self.main_size[index];
            let start = self.main_origin + cursor + margin[axes.main_start];
            cursor += axes.along(margin) + along + between;

            let align = match item.align_self {
                UiAlignSelf::Auto => self.layout.align,
                UiAlignSelf::Start => UiAlign::Start,
                UiAlignSelf::Center => UiAlign::Center,
                UiAlignSelf::End => UiAlign::End,
                UiAlignSelf::Stretch => UiAlign::Stretch,
            };
            let room = (extent - axes.across(margin)).max(0.0);
            let across = if align == UiAlign::Stretch {
                item.clamp(axes.cross, room)
            } else {
                self.cross_size[index]
            };
            let slack = room - across;
            let top = cross_start
                + margin[axes.cross_start]
                + match align {
                    UiAlign::Start | UiAlign::Stretch => 0.0,
                    UiAlign::Center => slack / 2.0,
                    UiAlign::End => slack,
                };

            let mut size = [0.0; 2];
            size[axes.main] = along;
            size[axes.cross] = across;
            boxes[index] = UiLayoutBox {
                offset: axes.offset(start + along / 2.0, top + across / 2.0),
                size,
            };
        }
    }
}

/// The size a layout would be to hold its children exactly: their sizes,
/// margins and gaps along the line, the largest across it, and its padding.
///
/// On one line whatever `wrap` says, because a box sized to its content has
/// no width of its own to wrap at.
pub(super) fn content_size(
    layout: &UiLayoutComponent,
    padding: UiSides,
    children: &[UiLayoutChild],
) -> [f32; 2] {
    let axes = Axes::of(layout.direction);
    let padding = clean(padding);
    let gap = layout.spacing.max(0.0);
    let mut main = gap * gaps_in(children.len());
    let mut cross: f32 = 0.0;
    for child in children {
        let item = &child.item;
        let margin = clean(item.margin);
        let along = if item.basis >= 0.0 {
            item.basis
        } else {
            child.size[axes.main].abs()
        };
        main += item.clamp(axes.main, along) + axes.along(margin);
        cross =
            cross.max(item.clamp(axes.cross, child.size[axes.cross].abs()) + axes.across(margin));
    }
    let mut size = [0.0; 2];
    size[axes.main] = main + axes.along(padding);
    size[axes.cross] = cross + axes.across(padding);
    size
}

#[allow(clippy::cast_precision_loss)]
fn gaps_in(count: usize) -> f32 {
    count.saturating_sub(1) as f32
}

/// Children in order, broken into lines where the next would not fit.
fn break_lines(
    sequence: &[usize],
    wrap: bool,
    room: f32,
    gap: f32,
    outer: impl Fn(usize) -> f32,
) -> Vec<Vec<usize>> {
    let mut lines = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut used = 0.0;
    for index in sequence.iter().copied() {
        let size = outer(index);
        let needed = if current.is_empty() {
            size
        } else {
            used + gap + size
        };
        if wrap && !current.is_empty() && needed > room + FIT_EPSILON {
            lines.push(std::mem::take(&mut current));
            used = size;
        } else {
            used = needed;
        }
        current.push(index);
    }
    lines.push(current);
    lines
}

/// Shares one line's spare room by `grow`, or takes back its shortfall by
/// `shrink` weighted by size, as CSS does.
#[allow(clippy::too_many_arguments)]
fn flex_line(
    line: &[usize],
    children: &[UiLayoutChild],
    base: &[f32],
    room: f32,
    gap: f32,
    axes: &Axes,
    margins: &[UiSides],
    main_size: &mut [f32],
) {
    let used: f32 = line
        .iter()
        .map(|index| base[*index] + axes.along(margins[*index]))
        .sum::<f32>()
        + gap * gaps_in(line.len());
    let free = room - used;
    if free > 0.0 {
        let total: f32 = line
            .iter()
            .map(|index| children[*index].item.grow.max(0.0))
            .sum();
        if total > 0.0 {
            for index in line.iter().copied() {
                let item = &children[index].item;
                let share = free * item.grow.max(0.0) / total;
                main_size[index] = item.clamp(axes.main, base[index] + share);
            }
        }
    } else if free < 0.0 {
        let total: f32 = line
            .iter()
            .map(|index| children[*index].item.shrink.max(0.0) * base[*index])
            .sum();
        if total > 0.0 {
            for index in line.iter().copied() {
                let child = &children[index];
                let item = &child.item;
                let share = free * item.shrink.max(0.0) * base[index] / total;
                // An authored minimum replaces the automatic one, as in CSS.
                let floor = if item.min_size[axes.main] > 0.0 {
                    0.0
                } else {
                    child.min_content[axes.main].min(base[index])
                };
                main_size[index] = item.clamp(axes.main, (base[index] + share).max(floor));
            }
        }
    }
}

/// How far across each line reaches.
///
/// One line fills the layout, as a single-line flex container's does. Wrapped
/// lines are as tall as their tallest child, and share any room left over
/// equally, which is CSS's default `align-content: normal`.
fn line_extents(
    lines: &[Vec<usize>],
    wrap: bool,
    room: f32,
    gap: f32,
    outer: impl Fn(usize) -> f32,
) -> Vec<f32> {
    if !wrap {
        return vec![room; lines.len()];
    }
    let mut extents: Vec<f32> = lines
        .iter()
        .map(|line| line.iter().copied().map(&outer).fold(0.0, f32::max))
        .collect();
    let used: f32 = extents.iter().sum::<f32>() + gap * gaps_in(extents.len());
    let spare = room - used;
    if spare > 0.0 {
        #[allow(clippy::cast_precision_loss)]
        let each = spare / extents.len() as f32;
        for extent in &mut extents {
            *extent += each;
        }
    }
    extents
}

/// Where the first child starts along a line, and the space between each
/// child's end and the next one's start.
fn justify(justify: UiJustify, free: f32, count: usize, gap: f32) -> (f32, f32) {
    #[allow(clippy::cast_precision_loss)]
    let count_f = count as f32;
    match justify {
        UiJustify::SpaceBetween if free > 0.0 && count > 1 => (0.0, gap + free / (count_f - 1.0)),
        UiJustify::SpaceAround if free > 0.0 => {
            let share = free / count_f;
            (share / 2.0, gap + share)
        }
        UiJustify::SpaceEvenly if free > 0.0 => {
            let share = free / (count_f + 1.0);
            (share, gap + share)
        }
        // With nothing to spread, CSS falls back to the start for
        // space-between and to centring the line for the other two.
        UiJustify::Start | UiJustify::SpaceBetween => (0.0, gap),
        UiJustify::Center | UiJustify::SpaceAround | UiJustify::SpaceEvenly => (free / 2.0, gap),
        UiJustify::End => (free, gap),
    }
}

#[cfg(test)]
mod tests;
