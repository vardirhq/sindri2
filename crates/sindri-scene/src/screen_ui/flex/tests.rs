//! The flex algorithm, one CSS behaviour at a time.

use super::super::{UiAlignSelf, UiBoxComponent};
use super::{content_size, resolve};
use crate::screen_ui::layout::{
    UiAlign, UiDirection, UiJustify, UiLayoutBox, UiLayoutChild, UiLayoutComponent,
};

fn row() -> UiLayoutComponent {
    UiLayoutComponent {
        direction: UiDirection::Row,
        spacing: 0.0,
        justify: UiJustify::Start,
        align: UiAlign::Start,
        wrap: false,
        fit_content: [false; 2],
    }
}

fn child(width: f32, item: UiBoxComponent) -> UiLayoutChild {
    UiLayoutChild {
        size: [width, 1.0],
        item,
        min_content: [0.0; 2],
    }
}

fn widths(boxes: &[UiLayoutBox]) -> Vec<f32> {
    boxes.iter().map(|placed| placed.size[0]).collect()
}

#[track_caller]
fn near(got: &[f32], want: &[f32]) {
    assert!(
        got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1.0e-5),
        "{got:?} is not {want:?}"
    );
}

#[test]
fn spare_room_is_shared_by_grow() {
    let one = UiBoxComponent {
        grow: 1.0,
        ..UiBoxComponent::default()
    };
    let two = UiBoxComponent { grow: 2.0, ..one };
    let fixed = UiBoxComponent::default();
    // 6 wide, 3 used, so 3 spare: one share to the first, two to the second.
    let boxes = resolve(
        &row(),
        [6.0, 1.0],
        [0.0; 4],
        &[child(1.0, one), child(1.0, two), child(1.0, fixed)],
    );
    near(&widths(&boxes), &[2.0, 3.0, 1.0]);
    // Packed from the left edge: the first spans -3 to -1.
    near(&[boxes[0].offset[0]], &[-2.0]);
}

#[test]
fn a_shortfall_is_taken_back_by_shrink_weighted_by_size() {
    let shrinks = UiBoxComponent::default();
    let rigid = UiBoxComponent {
        shrink: 0.0,
        ..shrinks
    };
    // 4 wide, 6 wanted: 2 short, taken from the two that shrink in
    // proportion to their sizes, 1 and 3.
    let boxes = resolve(
        &row(),
        [4.0, 1.0],
        [0.0; 4],
        &[child(1.0, shrinks), child(3.0, shrinks), child(2.0, rigid)],
    );
    near(&widths(&boxes), &[0.5, 1.5, 2.0]);
}

#[test]
fn shrinking_stops_at_what_the_child_holds() {
    let plain = UiBoxComponent::default();
    let mut badge = child(3.0, plain);
    badge.min_content = [2.5, 0.0];
    // 4 wide, 6 wanted: an even share would take the badge to 2, but its
    // content needs 2.5. (One pass: the other does not give up more.)
    let boxes = resolve(&row(), [4.0, 1.0], [0.0; 4], &[child(3.0, plain), badge]);
    near(&widths(&boxes), &[2.0, 2.5]);
}

#[test]
fn basis_and_limits_decide_the_starting_size_and_the_end() {
    let item = UiBoxComponent {
        grow: 1.0,
        basis: 0.0,
        max_size: [1.5, 0.0],
        ..UiBoxComponent::default()
    };
    let open = UiBoxComponent {
        grow: 1.0,
        basis: 0.0,
        ..UiBoxComponent::default()
    };
    // Both start from nothing and would share 4 equally; the first stops
    // at its maximum. (One pass: the second does not take up the rest.)
    let boxes = resolve(
        &row(),
        [4.0, 1.0],
        [0.0; 4],
        &[child(3.0, item), child(3.0, open)],
    );
    near(&widths(&boxes), &[1.5, 2.0]);
}

#[test]
fn order_moves_a_child_without_moving_it_in_the_scene() {
    let late = UiBoxComponent {
        order: 1,
        ..UiBoxComponent::default()
    };
    let plain = UiBoxComponent::default();
    let boxes = resolve(
        &row(),
        [4.0, 1.0],
        [0.0; 4],
        &[child(1.0, late), child(1.0, plain)],
    );
    // The second child comes first.
    assert!(boxes[1].offset[0] < boxes[0].offset[0], "{boxes:?}");
}

#[test]
fn wrapping_starts_a_new_line_below_and_lines_share_the_height() {
    let mut layout = row();
    layout.wrap = true;
    layout.spacing = 0.5;
    let plain = UiBoxComponent::default();
    // Three 1.5-wide children in a 4-wide row: two fit (1.5 + 0.5 + 1.5),
    // the third wraps. The row is 4 high: two lines of 1, a gap of 0.5,
    // and 1.5 left over, shared as 0.75 more per line.
    let boxes = resolve(
        &layout,
        [4.0, 4.0],
        [0.0; 4],
        &[child(1.5, plain), child(1.5, plain), child(1.5, plain)],
    );
    near(&[boxes[0].offset[1], boxes[1].offset[1]], &[1.5, 1.5]);
    // The second line starts 1.75 + 0.5 below the top (0.25 below the
    // middle), so its child's middle is 0.75 below.
    near(&[boxes[2].offset[0], boxes[2].offset[1]], &[-1.25, -0.75]);
}

#[test]
fn align_self_overrides_the_layout_for_one_child() {
    let mut layout = row();
    layout.align = UiAlign::Start;
    let stretched = UiBoxComponent {
        align_self: UiAlignSelf::Stretch,
        ..UiBoxComponent::default()
    };
    let ended = UiBoxComponent {
        align_self: UiAlignSelf::End,
        ..UiBoxComponent::default()
    };
    let boxes = resolve(
        &layout,
        [4.0, 3.0],
        [0.0; 4],
        &[child(1.0, stretched), child(1.0, ended)],
    );
    near(&[boxes[0].size[1], boxes[0].offset[1]], &[3.0, 0.0]);
    // One high, at the bottom of three: its middle is 1 below centre.
    near(&[boxes[1].size[1], boxes[1].offset[1]], &[1.0, -1.0]);
}

#[test]
fn space_around_and_evenly_share_the_ends_as_css_does() {
    let plain = UiBoxComponent::default();
    let children = [child(1.0, plain), child(1.0, plain)];
    let mut layout = row();
    // 6 wide, 2 used, 4 spare.
    layout.justify = UiJustify::SpaceEvenly;
    let even = resolve(&layout, [6.0, 1.0], [0.0; 4], &children);
    // A third of the spare before, between and after.
    near(
        &[even[0].offset[0], even[1].offset[0]],
        &[-3.0 + 4.0 / 3.0 + 0.5, -3.0 + 8.0 / 3.0 + 1.5],
    );
    layout.justify = UiJustify::SpaceAround;
    let around = resolve(&layout, [6.0, 1.0], [0.0; 4], &children);
    // Two each around: one at each end, two between.
    near(&[around[0].offset[0], around[1].offset[0]], &[-1.5, 1.5]);
}

#[test]
fn a_box_that_fits_its_content_is_its_children_its_gaps_and_its_padding() {
    let mut layout = row();
    layout.spacing = 0.25;
    let plain = UiBoxComponent::default();
    let spaced = UiBoxComponent {
        margin: [0.0, 0.0, 0.5, 0.0],
        ..plain
    };
    let size = content_size(
        &layout,
        [0.1, 0.2, 0.1, 0.2],
        &[child(1.0, plain), child(2.0, spaced)],
    );
    near(&size, &[1.0 + 0.25 + 2.0 + 0.4, 1.5 + 0.2]);
}
