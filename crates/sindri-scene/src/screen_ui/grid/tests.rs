//! The grid, one CSS behaviour at a time.

use super::super::UiBoxComponent;
use super::super::layout::{UiAlign, UiLayoutBox, UiLayoutChild};
use super::{UiGridComponent, UiTrack, content_size, resolve};

fn grid(columns: &[&str]) -> UiGridComponent {
    UiGridComponent {
        columns: columns
            .iter()
            .map(|track| UiTrack::parse(track).expect("a track"))
            .collect(),
        ..UiGridComponent::default()
    }
}

fn child(size: [f32; 2]) -> UiLayoutChild {
    UiLayoutChild::sized(size)
}

fn at(column: u32, row: u32, spans: [u32; 2], size: [f32; 2]) -> UiLayoutChild {
    let mut placed = child(size);
    placed.item = UiBoxComponent {
        grid_column: [column, spans[0]],
        grid_row: [row, spans[1]],
        ..UiBoxComponent::default()
    };
    placed
}

/// Each box's left and top edge, measured right and down from the grid's
/// top-left corner, and its size: how a grid reads on paper.
fn corners(boxes: &[UiLayoutBox], outer: [f32; 2]) -> Vec<[f32; 4]> {
    boxes
        .iter()
        .map(|placed| {
            [
                placed.offset[0] - placed.size[0] / 2.0 + outer[0] / 2.0,
                -placed.offset[1] - placed.size[1] / 2.0 + outer[1] / 2.0,
                placed.size[0],
                placed.size[1],
            ]
        })
        .collect()
}

#[track_caller]
fn near(got: &[[f32; 4]], want: &[[f32; 4]]) {
    let close = got.len() == want.len()
        && got
            .iter()
            .zip(want)
            .all(|(a, b)| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1.0e-5));
    assert!(close, "{got:?} is not {want:?}");
}

#[test]
fn children_flow_into_cells_row_by_row_and_fill_them() {
    let mut three = grid(&["1fr", "1fr", "1fr"]);
    three.gap = [0.3, 0.2];
    let outer = [6.6, 4.0];
    let items = vec![child([0.5, 1.0]); 4];
    let boxes = resolve(&three, outer, [0.0; 4], &items);
    // Three 2-wide columns with 0.3 between; two rows of 1, 0.2 apart.
    // Stretched across, and down to their row, which is as tall as they are.
    near(
        &corners(&boxes, outer),
        &[
            [0.0, 0.0, 2.0, 1.0],
            [2.3, 0.0, 2.0, 1.0],
            [4.6, 0.0, 2.0, 1.0],
            [0.0, 1.2, 2.0, 1.0],
        ],
    );
}

#[test]
fn fixed_auto_and_fraction_columns_share_the_width_as_css_does() {
    let mixed = grid(&["1", "auto", "1fr"]);
    let outer = [6.0, 1.0];
    let items = [child([0.2, 1.0]), child([1.5, 1.0]), child([0.2, 1.0])];
    let boxes = resolve(&mixed, outer, [0.0; 4], &items);
    // 1 fixed, 1.5 for what the auto column holds, 3.5 left for the fr.
    near(
        &corners(&boxes, outer),
        &[
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 0.0, 1.5, 1.0],
            [2.5, 0.0, 3.5, 1.0],
        ],
    );
}

#[test]
fn a_child_can_start_where_it_says_and_span_tracks() {
    let two = grid(&["1fr", "1fr"]);
    let outer = [4.0, 2.0];
    // A header across both columns in row one, then two auto children
    // flowing into row two.
    let items = [
        child([0.1, 1.0]),
        child([0.1, 1.0]),
        at(1, 1, [2, 1], [0.1, 1.0]),
    ];
    let boxes = resolve(&two, outer, [0.0; 4], &items);
    near(
        &corners(&boxes, outer),
        &[
            [0.0, 1.0, 2.0, 1.0],
            [2.0, 1.0, 2.0, 1.0],
            [0.0, 0.0, 4.0, 1.0],
        ],
    );
}

#[test]
fn a_child_past_the_last_row_adds_rows() {
    let two = grid(&["1fr", "1fr"]);
    let outer = [2.0, 3.0];
    let items = [child([0.5, 0.5]), at(2, 3, [1, 1], [0.5, 0.5])];
    let boxes = resolve(&two, outer, [0.0; 4], &items);
    // Rows one and three hold something half a unit tall; row two is empty.
    near(
        &corners(&boxes, outer),
        &[[0.0, 0.0, 1.0, 0.5], [1.0, 0.5, 1.0, 0.5]],
    );
}

#[test]
fn items_keep_their_size_when_the_grid_aligns_rather_than_stretches() {
    let mut centred = grid(&["1fr", "1fr"]);
    centred.justify_items = UiAlign::Center;
    centred.align_items = UiAlign::End;
    let outer = [4.0, 2.0];
    let items = [child([1.0, 0.5]), child([1.0, 2.0])];
    let boxes = resolve(&centred, outer, [0.0; 4], &items);
    // The row is as tall as the taller child, 2; the short one sits at its
    // bottom and each is centred across its 2-wide column.
    near(
        &corners(&boxes, outer),
        &[[0.5, 1.5, 1.0, 0.5], [2.5, 0.0, 1.0, 2.0]],
    );
}

#[test]
fn a_grid_that_fits_its_content_is_its_tracks_gaps_and_padding() {
    let mut fitted = grid(&["0.5", "auto"]);
    fitted.gap = [0.1, 0.2];
    let items = [child([0.3, 0.4]), child([1.0, 0.6]), child([0.2, 0.2])];
    let size = content_size(&fitted, [0.05, 0.1, 0.05, 0.1], &items);
    // Columns 0.5 and 1.0 with 0.1 between; rows 0.6 and 0.2 with 0.2
    // between; and the padding round it all.
    assert!(
        (size[0] - (0.5 + 1.0 + 0.1 + 0.2)).abs() < 1.0e-5,
        "{size:?}"
    );
    assert!(
        (size[1] - (0.6 + 0.2 + 0.2 + 0.1)).abs() < 1.0e-5,
        "{size:?}"
    );
}

#[test]
fn a_child_past_the_last_column_adds_an_auto_column() {
    let one = grid(&["1"]);
    let outer = [3.0, 1.0];
    let items = [child([0.2, 1.0]), at(2, 1, [1, 1], [0.7, 1.0])];
    let boxes = resolve(&one, outer, [0.0; 4], &items);
    // The explicit column is 1; the implicit one is as wide as its child.
    near(
        &corners(&boxes, outer),
        &[[0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 0.7, 1.0]],
    );
}
