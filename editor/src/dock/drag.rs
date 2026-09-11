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

use super::{Corner, Panel, Place, Slot};

/// Where a dragged panel would land if it were released now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DropTarget {
    pub place: Place,
    /// Which tab position within it.
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

/// Where a pointer near the window's border would put a panel.
///
/// **Edges dock, corners float.** One rule, and it is the whole of how a place
/// that currently holds nothing is reached: everywhere else is reached by
/// dropping onto the group already there. A pointer inside the band of both a
/// side and a top or bottom edge is in a corner and means an overlay; inside
/// one band only, it means the dock against that edge.
///
/// The top edge alone answers nothing, because there is no dock along the top —
/// the menu bar is there. Its two corners still take overlays.
pub fn edge_place(window: Rect, pointer: Pos2) -> Option<Place> {
    if !window.contains(pointer) {
        return None;
    }
    let left = pointer.x - window.left() <= EDGE_BAND;
    let right = window.right() - pointer.x <= EDGE_BAND;
    let top = pointer.y - window.top() <= EDGE_BAND;
    let bottom = window.bottom() - pointer.y <= EDGE_BAND;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(Place::Overlay(Corner::TopLeft)),
        (_, true, true, _) => Some(Place::Overlay(Corner::TopRight)),
        (true, _, _, true) => Some(Place::Overlay(Corner::BottomLeft)),
        (_, true, _, true) => Some(Place::Overlay(Corner::BottomRight)),
        (true, ..) => Some(Place::Dock(Slot::FarLeft)),
        (_, true, ..) => Some(Place::Dock(Slot::FarRight)),
        (_, _, _, true) => Some(Place::Dock(Slot::Bottom)),
        _ => None,
    }
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

    fn window() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 900.0))
    }

    #[test]
    fn the_middle_of_the_window_opens_nothing() {
        assert_eq!(edge_place(window(), pos2(700.0, 450.0)), None);
    }

    #[test]
    fn each_border_docks_against_it() {
        assert_eq!(
            edge_place(window(), pos2(10.0, 450.0)),
            Some(Place::Dock(Slot::FarLeft))
        );
        assert_eq!(
            edge_place(window(), pos2(1390.0, 450.0)),
            Some(Place::Dock(Slot::FarRight))
        );
        assert_eq!(
            edge_place(window(), pos2(700.0, 890.0)),
            Some(Place::Dock(Slot::Bottom))
        );
    }

    /// Edges dock, corners float. The rule has to be unambiguous where the two
    /// bands overlap, or the highlight flickers between a dock and an overlay
    /// as the pointer moves a pixel.
    #[test]
    fn each_corner_overlays_it() {
        assert_eq!(
            edge_place(window(), pos2(8.0, 8.0)),
            Some(Place::Overlay(Corner::TopLeft))
        );
        assert_eq!(
            edge_place(window(), pos2(1392.0, 8.0)),
            Some(Place::Overlay(Corner::TopRight))
        );
        assert_eq!(
            edge_place(window(), pos2(8.0, 892.0)),
            Some(Place::Overlay(Corner::BottomLeft))
        );
        assert_eq!(
            edge_place(window(), pos2(1392.0, 892.0)),
            Some(Place::Overlay(Corner::BottomRight))
        );
    }

    /// There is no dock along the top -- the menu bar is there -- so the top
    /// edge between its two corners answers nothing rather than guessing.
    #[test]
    fn the_top_edge_between_the_corners_answers_nothing() {
        assert_eq!(edge_place(window(), pos2(700.0, 8.0)), None);
    }

    #[test]
    fn a_pointer_outside_the_window_lands_nowhere() {
        assert_eq!(edge_place(window(), pos2(-20.0, 450.0)), None);
    }
}
