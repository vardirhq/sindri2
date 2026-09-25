//! Where each child goes in a grid, and the box it ends up with.

use super::super::UiAlignSelf;
use super::super::box_model::{UiSides, clean};
use super::super::layout::{UiAlign, UiLayoutBox, UiLayoutChild};
use super::UiGridComponent;
use super::tracks::{UiTrack, size_line};

const TOP: usize = 0;
const RIGHT: usize = 1;
const BOTTOM: usize = 2;
const LEFT: usize = 3;

/// The most rows auto-placement opens, so a runaway placement cannot grow a
/// grid without end. No real screen is a thousand rows of widgets.
const MAX_ROWS: usize = 1_000;

/// One child's cell: first column and row, and how many of each it spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    column: usize,
    row: usize,
    columns: usize,
    rows: usize,
}

/// The tracks a grid lays out with, explicit and implicit, and every child's
/// cell, in the order the children were given.
struct Plan {
    columns: Vec<UiTrack>,
    rows: Vec<UiTrack>,
    cells: Vec<Cell>,
}

fn plan(grid: &UiGridComponent, children: &[UiLayoutChild]) -> Plan {
    let mut columns = if grid.columns.is_empty() {
        vec![UiTrack::Fraction(1.0)]
    } else {
        grid.columns.clone()
    };
    // Children placed past the last column widen the grid with `auto`
    // columns, as CSS's implicit tracks do.
    let widest = children
        .iter()
        .filter_map(|child| {
            let [start, span] = child.item.grid_column;
            (start > 0).then(|| to_index(start) - 1 + to_index(span.max(1)))
        })
        .max()
        .unwrap_or(0);
    while columns.len() < widest {
        columns.push(UiTrack::Auto);
    }
    let across = columns.len();

    let mut taken: Vec<Vec<bool>> = Vec::new();
    let mut cells = vec![
        Cell {
            column: 0,
            row: 0,
            columns: 1,
            rows: 1,
        };
        children.len()
    ];
    // In CSS's order: children placed in both axes first, then those held to
    // a row, then the rest, so an auto child never takes a cell a placed one
    // was given. Within each, by `order`, then as the scene has them.
    let mut sequence: Vec<usize> = (0..children.len()).collect();
    sequence.sort_by_key(|index| {
        let item = &children[*index].item;
        let phase = match (item.grid_column[0] > 0, item.grid_row[0] > 0) {
            (true, true) => 0,
            (false, true) => 1,
            _ => 2,
        };
        (phase, item.order)
    });
    // Auto-placement's cursor, which only moves forward, as CSS's default
    // (sparse) packing does.
    let mut cursor = (0, 0);
    for index in sequence {
        let item = &children[index].item;
        let span = [
            to_index(item.grid_column[1].max(1)).min(across),
            to_index(item.grid_row[1].max(1)),
        ];
        let fixed = [
            (item.grid_column[0] > 0).then(|| to_index(item.grid_column[0]) - 1),
            (item.grid_row[0] > 0).then(|| to_index(item.grid_row[0]) - 1),
        ];
        let cell = match fixed {
            [Some(column), Some(row)] => (column, row),
            [Some(column), None] => (0..MAX_ROWS)
                .map(|row| (column, row))
                .find(|at| free(&taken, *at, span))
                .unwrap_or((column, 0)),
            [None, Some(row)] => (0..=across - span[0])
                .map(|column| (column, row))
                .find(|at| free(&taken, *at, span))
                .unwrap_or((0, row)),
            [None, None] => {
                let found = next_free(&taken, cursor, span, across);
                cursor = (found.0 + span[0], found.1);
                found
            }
        };
        mark(&mut taken, cell, span);
        cells[index] = Cell {
            column: cell.0,
            row: cell.1,
            columns: span[0],
            rows: span[1],
        };
    }
    let mut rows = grid.rows.clone();
    let deepest = cells
        .iter()
        .map(|cell| cell.row + cell.rows)
        .max()
        .unwrap_or(0);
    while rows.len() < deepest {
        rows.push(UiTrack::Auto);
    }
    Plan {
        columns,
        rows,
        cells,
    }
}

fn to_index(line: u32) -> usize {
    usize::try_from(line).unwrap_or(usize::MAX).min(MAX_ROWS)
}

fn free(taken: &[Vec<bool>], (column, row): (usize, usize), span: [usize; 2]) -> bool {
    (row..row + span[1]).all(|r| {
        (column..column + span[0]).all(|c| {
            !taken
                .get(r)
                .and_then(|line| line.get(c))
                .copied()
                .unwrap_or(false)
        })
    })
}

fn mark(taken: &mut Vec<Vec<bool>>, (column, row): (usize, usize), span: [usize; 2]) {
    for r in row..row + span[1] {
        if taken.len() <= r {
            taken.resize(r + 1, Vec::new());
        }
        let line = &mut taken[r];
        if line.len() < column + span[0] {
            line.resize(column + span[0], false);
        }
        for cell in line.iter_mut().skip(column).take(span[0]) {
            *cell = true;
        }
    }
}

/// The first free cell at or after `cursor`, reading row by row.
fn next_free(
    taken: &[Vec<bool>],
    cursor: (usize, usize),
    span: [usize; 2],
    across: usize,
) -> (usize, usize) {
    let (mut column, mut row) = cursor;
    while row < MAX_ROWS {
        if column + span[0] > across {
            column = 0;
            row += 1;
            continue;
        }
        if free(taken, (column, row), span) {
            return (column, row);
        }
        column += 1;
    }
    (0, row)
}

/// What each track holds: the largest child that sits in it alone, margins
/// included. A child spanning several tracks does not widen them.
fn track_content(plan: &Plan, children: &[UiLayoutChild], axis: usize) -> Vec<f32> {
    let count = if axis == 0 {
        plan.columns.len()
    } else {
        plan.rows.len()
    };
    let mut content = vec![0.0_f32; count];
    for (cell, child) in plan.cells.iter().zip(children) {
        let (at, span) = if axis == 0 {
            (cell.column, cell.columns)
        } else {
            (cell.row, cell.rows)
        };
        if span != 1 {
            continue;
        }
        let margin = clean(child.item.margin);
        let around = if axis == 0 {
            margin[LEFT] + margin[RIGHT]
        } else {
            margin[TOP] + margin[BOTTOM]
        };
        // The size it has, as CSS takes an item's set size for an `auto`
        // track; a text fitting its words already has them measured in.
        let own = child.item.clamp(axis, child.size[axis].abs());
        if let Some(slot) = content.get_mut(at) {
            *slot = slot.max(own + around);
        }
    }
    content
}

#[allow(clippy::cast_precision_loss)]
fn span_of(sizes: &[f32], start: usize, span: usize, gap: f32) -> f32 {
    sizes.iter().skip(start).take(span).sum::<f32>() + gap * span.saturating_sub(1) as f32
}

fn starts(sizes: &[f32], gap: f32) -> Vec<f32> {
    let mut at = 0.0;
    sizes
        .iter()
        .map(|size| {
            let start = at;
            at += size + gap;
            start
        })
        .collect()
}

/// The size a grid is to hold its tracks exactly, padding included.
pub(in super::super) fn content_size(
    grid: &UiGridComponent,
    padding: UiSides,
    children: &[UiLayoutChild],
) -> [f32; 2] {
    let plan = plan(grid, children);
    let padding = clean(padding);
    let gap = [grid.gap[0].max(0.0), grid.gap[1].max(0.0)];
    let columns = size_line(
        &plan.columns,
        &track_content(&plan, children, 0),
        None,
        gap[0],
    );
    let rows = size_line(&plan.rows, &track_content(&plan, children, 1), None, gap[1]);
    [
        span_of(&columns, 0, columns.len(), gap[0]) + padding[LEFT] + padding[RIGHT],
        span_of(&rows, 0, rows.len(), gap[1]) + padding[TOP] + padding[BOTTOM],
    ]
}

/// Each child's box in the grid, in the order the children were given.
pub(in super::super) fn resolve(
    grid: &UiGridComponent,
    parent_size: [f32; 2],
    padding: UiSides,
    children: &[UiLayoutChild],
) -> Vec<UiLayoutBox> {
    if children.is_empty() {
        return Vec::new();
    }
    let plan = plan(grid, children);
    let padding = clean(padding);
    let gap = [grid.gap[0].max(0.0), grid.gap[1].max(0.0)];
    let outer = [parent_size[0].abs(), parent_size[1].abs()];
    let room = [
        (outer[0] - padding[LEFT] - padding[RIGHT]).max(0.0),
        (outer[1] - padding[TOP] - padding[BOTTOM]).max(0.0),
    ];
    let columns = size_line(
        &plan.columns,
        &track_content(&plan, children, 0),
        Some(room[0]),
        gap[0],
    );
    let rows = size_line(
        &plan.rows,
        &track_content(&plan, children, 1),
        Some(room[1]),
        gap[1],
    );
    let column_starts = starts(&columns, gap[0]);
    let row_starts = starts(&rows, gap[1]);
    // From the grid's top-left content corner, measured right and down.
    let origin = [
        -outer[0] / 2.0 + padding[LEFT],
        -outer[1] / 2.0 + padding[TOP],
    ];

    plan.cells
        .iter()
        .zip(children)
        .map(|(cell, child)| {
            let margin = clean(child.item.margin);
            let across = align_in(
                grid.justify_items,
                span_of(&columns, cell.column, cell.columns, gap[0]),
                [margin[LEFT], margin[RIGHT]],
                child,
                0,
            );
            let down_align = match child.item.align_self {
                UiAlignSelf::Auto => grid.align_items,
                UiAlignSelf::Start => UiAlign::Start,
                UiAlignSelf::Center => UiAlign::Center,
                UiAlignSelf::End => UiAlign::End,
                UiAlignSelf::Stretch => UiAlign::Stretch,
            };
            let down = align_in(
                down_align,
                span_of(&rows, cell.row, cell.rows, gap[1]),
                [margin[TOP], margin[BOTTOM]],
                child,
                1,
            );
            let left =
                origin[0] + column_starts.get(cell.column).copied().unwrap_or(0.0) + across.0;
            let top = origin[1] + row_starts.get(cell.row).copied().unwrap_or(0.0) + down.0;
            UiLayoutBox {
                // Up is positive in the overlay, and rows run down.
                offset: [left + across.1 / 2.0, -(top + down.1 / 2.0)],
                size: [across.1, down.1],
            }
        })
        .collect()
}

/// Where a child starts inside its cell on one axis, and how big it is there.
fn align_in(
    align: UiAlign,
    cell: f32,
    [before, after]: [f32; 2],
    child: &UiLayoutChild,
    axis: usize,
) -> (f32, f32) {
    let room = (cell - before - after).max(0.0);
    let size = if align == UiAlign::Stretch {
        child.item.clamp(axis, room)
    } else {
        child.item.clamp(axis, child.size[axis].abs())
    };
    let slack = room - size;
    let start = before
        + match align {
            UiAlign::Start | UiAlign::Stretch => 0.0,
            UiAlign::Center => slack / 2.0,
            UiAlign::End => slack,
        };
    (start, size)
}
