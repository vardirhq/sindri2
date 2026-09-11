//! Moving a panel from one slot to another by dragging its tab.
//!
//! The gesture is the whole of the rearranging interface, so what it can say
//! has to cover what the model can hold: a tab dropped between two other tabs
//! joins that group at that position, and a tab dropped on the edge of the
//! window opens a slot that was empty. Anything else releases where it started.
//!
//! None of this is drawn here. This is the state a drag carries between frames
//! and the arithmetic that turns a pointer position into an answer; the
//! highlight, the floating label, and the tab strips live with the rest of the
//! editor's painting.

use eframe::egui::{Pos2, Rect};

use super::{Panel, Slot};

/// Where a dragged panel would land if it were released now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DropTarget {
    pub slot: Slot,
    /// Which tab position within the slot.
    pub index: usize,
}

/// A tab being dragged, and the last answer worked out for it.
///
/// The target is remembered rather than recomputed on release because the
/// release frame has no pointer position of its own worth trusting: egui
/// reports the release at the last position, and a pointer that left the window
/// reports nothing at all. Remembering the last good answer means letting go
/// does what the highlight said it would.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub panel: Panel,
    pub target: Option<DropTarget>,
    /// Where the pointer is, for drawing the label that follows it.
    pub pointer: Pos2,
}

impl Drag {
    pub const fn new(panel: Panel, pointer: Pos2) -> Self {
        Self {
            panel,
            target: None,
            pointer,
        }
    }
}

/// How close to the window edge a drop counts as opening an edge slot.
const EDGE_BAND: f32 = 72.0;

/// Which tab position a pointer sits at within a strip of tabs.
///
/// The tabs' rectangles are passed in rather than their widths: they are
/// measured while the strip is drawn, and a second guess at where each one
/// ended up would be a second layout that could disagree with the first.
///
/// The answer is an insertion point, so it ranges over `0..=tabs.len()`:
/// past the midpoint of a tab means after it.
pub fn tab_index(tabs: &[Rect], pointer: Pos2) -> usize {
    tabs.iter()
        .position(|tab| pointer.x < tab.center().x)
        .unwrap_or(tabs.len())
}

/// Which edge slot a pointer near the window's border would open.
///
/// Only the outermost slots, and only when the pointer is outside every panel
/// already drawn: the inner slots are reachable by dropping onto the group
/// already there, and an edge gesture that could mean either is a gesture that
/// means neither.
pub fn edge_slot(window: Rect, pointer: Pos2) -> Option<Slot> {
    if !window.contains(pointer) {
        return None;
    }
    // Closest edge wins, so a corner names one slot rather than flickering
    // between the two whose bands it is inside.
    [
        (pointer.x - window.left(), Slot::FarLeft),
        (window.right() - pointer.x, Slot::FarRight),
        (window.bottom() - pointer.y, Slot::Bottom),
    ]
    .into_iter()
    .filter(|(distance, _)| *distance <= EDGE_BAND)
    .min_by(|(one, _), (other, _)| one.total_cmp(other))
    .map(|(_, slot)| slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    fn tabs(widths: &[f32]) -> Vec<Rect> {
        let mut x = 0.0;
        widths
            .iter()
            .map(|width| {
                let rect = Rect::from_min_size(pos2(x, 0.0), vec2(*width, 28.0));
                x += width;
                rect
            })
            .collect()
    }

    #[test]
    fn a_pointer_left_of_every_tab_inserts_first() {
        assert_eq!(tab_index(&tabs(&[80.0, 80.0]), pos2(4.0, 10.0)), 0);
    }

    #[test]
    fn a_pointer_past_the_last_tab_appends() {
        assert_eq!(tab_index(&tabs(&[80.0, 80.0]), pos2(300.0, 10.0)), 2);
    }

    /// The midpoint is the seam: before it the drop goes in front of that tab,
    /// after it behind. Anything else makes the insertion marker jump.
    #[test]
    fn the_midpoint_of_a_tab_decides_which_side_of_it_a_drop_lands() {
        let tabs = tabs(&[80.0, 80.0]);
        assert_eq!(tab_index(&tabs, pos2(39.0, 10.0)), 0);
        assert_eq!(tab_index(&tabs, pos2(41.0, 10.0)), 1);
        assert_eq!(tab_index(&tabs, pos2(119.0, 10.0)), 1);
        assert_eq!(tab_index(&tabs, pos2(121.0, 10.0)), 2);
    }

    #[test]
    fn an_empty_strip_takes_the_first_position() {
        assert_eq!(tab_index(&[], pos2(10.0, 10.0)), 0);
    }

    #[test]
    fn the_middle_of_the_window_opens_no_edge_slot() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0));
        assert_eq!(edge_slot(window, pos2(700.0, 450.0)), None);
    }

    #[test]
    fn each_border_opens_the_slot_against_it() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0));
        assert_eq!(edge_slot(window, pos2(10.0, 450.0)), Some(Slot::FarLeft));
        assert_eq!(edge_slot(window, pos2(1390.0, 450.0)), Some(Slot::FarRight));
        assert_eq!(edge_slot(window, pos2(700.0, 890.0)), Some(Slot::Bottom));
    }

    /// A corner is within the band of two edges at once, and picking the nearer
    /// is what stops the highlight flickering between them.
    #[test]
    fn a_corner_resolves_to_the_nearer_edge() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0));
        assert_eq!(edge_slot(window, pos2(8.0, 880.0)), Some(Slot::FarLeft));
        assert_eq!(edge_slot(window, pos2(40.0, 885.0)), Some(Slot::Bottom));
    }

    #[test]
    fn a_pointer_outside_the_window_lands_nowhere() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0));
        assert_eq!(edge_slot(window, pos2(-20.0, 450.0)), None);
    }
}
