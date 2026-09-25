//! CSS grid, written to the element's `sindri.ui.grid` and its items' boxes.
//!
//! `display: grid` makes an element a grid (and `display: flex` a flex line),
//! the one place styling changes what an element is, because that is what the
//! property means. The container's tracks, gaps and item alignment go to
//! `sindri.ui.grid`; an item's `grid-column` and `grid-row` go to its box.

use sindri_core::{EntityId, World};
use weave::Viewport;

use crate::computed::Length;
use crate::{ApplyError, box_model, invalid, length, set_component_field};

const GRID: &str = "sindri.ui.grid";
const LAYOUT: &str = "sindri.ui.layout";

/// Makes the element a grid or a flex line, as `display` says.
pub(crate) fn display(
    world: &mut World,
    entity: EntityId,
    id: &str,
    value: &str,
) -> Result<(), ApplyError> {
    let (wanted, other) = match value.trim() {
        "grid" => (GRID, LAYOUT),
        "flex" => (LAYOUT, GRID),
        _ => return Err(invalid(id, "display", value)),
    };
    let Some(data) = world.get_mut(entity) else {
        return Ok(());
    };
    data.components.remove(other);
    data.components
        .entry(wanted.to_owned())
        .or_insert_with(|| serde_json::json!({}));
    Ok(())
}

/// Applies `property` if it is a grid property, answering whether it was.
pub(crate) fn apply(
    world: &mut World,
    entity: EntityId,
    id: &str,
    property: &str,
    value: &str,
    viewport: Viewport,
) -> Result<bool, ApplyError> {
    let refuse = || invalid(id, property, value);
    match property {
        "grid-template-columns" | "grid-template-rows" => {
            let axis = usize::from(property == "grid-template-rows");
            let room = content_extent(world, entity, axis);
            // `none` is no explicit tracks: every one is implicit and `auto`.
            let tracks = if value.trim() == "none" {
                Vec::new()
            } else {
                tracks(value, viewport, room).ok_or_else(refuse)?
            };
            let field = if axis == 0 { "columns" } else { "rows" };
            set_component_field(world, entity, GRID, field, tracks.into());
        }
        // CSS's `gap` is the row gap then the column gap; a flex line takes
        // the one along it, which for the lines here is the first.
        "gap" | "row-gap" | "column-gap" => {
            let parts: Vec<f32> = weave::shorthand::words(value)
                .into_iter()
                .map(|part| length(part, viewport).filter(|gap| *gap >= 0.0))
                .collect::<Option<_>>()
                .ok_or_else(refuse)?;
            let (row, column) = match (property, parts.as_slice()) {
                ("gap", [both]) => (Some(*both), Some(*both)),
                ("gap", [row, column]) => (Some(*row), Some(*column)),
                ("row-gap", [row]) => (Some(*row), None),
                ("column-gap", [column]) => (None, Some(*column)),
                _ => return Err(refuse()),
            };
            set_gap(world, entity, [column, row]);
            if let Some(first) = if property == "column-gap" {
                column
            } else {
                row
            } {
                set_component_field(world, entity, LAYOUT, "spacing", first.into());
            }
        }
        "justify-items" => {
            let stored = alignment(value.trim()).ok_or_else(refuse)?;
            set_component_field(world, entity, GRID, "justify_items", stored.into());
        }
        "grid-column" | "grid-row" => {
            let line = placement(value).ok_or_else(refuse)?;
            let field = if property == "grid-column" {
                "grid_column"
            } else {
                "grid_row"
            };
            box_model::set_field(world, entity, field, serde_json::json!(line));
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// Writes a grid's alignment across its rows, which `align-items` means for
/// a grid as it does for a flex line.
pub(crate) fn align_items(world: &mut World, entity: EntityId, stored: &str) {
    set_component_field(world, entity, GRID, "align_items", stored.into());
}

/// Marks whether a grid sizes itself to its tracks on `axis`; answers whether
/// the element is a grid at all.
pub(crate) fn set_fit_content(
    world: &mut World,
    entity: EntityId,
    axis: usize,
    fits: bool,
) -> bool {
    let Some(grid) = world
        .get_mut(entity)
        .and_then(|data| data.components.get_mut(GRID))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return false;
    };
    let pair = grid
        .entry("fit_content".to_owned())
        .or_insert_with(|| serde_json::json!([false, false]));
    if let Some(array) = pair.as_array_mut() {
        array.resize(2, false.into());
        array[axis] = fits.into();
    }
    true
}

fn set_gap(world: &mut World, entity: EntityId, gap: [Option<f32>; 2]) {
    let Some(grid) = world
        .get_mut(entity)
        .and_then(|data| data.components.get_mut(GRID))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let pair = grid
        .entry("gap".to_owned())
        .or_insert_with(|| serde_json::json!([0.0, 0.0]));
    if let Some(array) = pair.as_array_mut() {
        array.resize(2, 0.0.into());
        for (axis, value) in gap.into_iter().enumerate() {
            if let Some(value) = value {
                array[axis] = value.into();
            }
        }
    }
}

fn alignment(value: &str) -> Option<&'static str> {
    match value {
        "start" | "flex-start" | "self-start" | "left" => Some("start"),
        "center" => Some("center"),
        "end" | "flex-end" | "self-end" | "right" => Some("end"),
        "stretch" | "normal" => Some("stretch"),
        _ => None,
    }
}

/// The element's content box on `axis`, which a percentage track is of.
fn content_extent(world: &World, entity: EntityId, axis: usize) -> f32 {
    let size = world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default()
        .scale_2d()[axis]
        .abs();
    let padding = box_model::padding(world, entity);
    let inset = if axis == 0 {
        padding[1] + padding[3]
    } else {
        padding[0] + padding[2]
    };
    (size - inset).max(0.0)
}

/// A track list as CSS writes it — `1fr 200px auto`, `repeat(3, 1fr)` — as
/// the tracks are stored: `"1fr"`, `"auto"`, or overlay units.
fn tracks(value: &str, viewport: Viewport, room: f32) -> Option<Vec<String>> {
    let mut stored = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("repeat(") {
            let close = after.find(')')?;
            let (count, pattern) = after[..close].split_once(',')?;
            let count: usize = count
                .trim()
                .parse()
                .ok()
                .filter(|count| (1..=100).contains(count))?;
            let one = tracks(pattern, viewport, room)?;
            for _ in 0..count {
                stored.extend(one.iter().cloned());
            }
            rest = after[close + 1..].trim_start();
            continue;
        }
        let word = weave::shorthand::words(rest).into_iter().next()?;
        stored.push(track(word, viewport, room)?);
        rest = rest[word.len()..].trim_start();
    }
    (!stored.is_empty()).then_some(stored)
}

fn track(text: &str, viewport: Viewport, room: f32) -> Option<String> {
    if text == "auto" {
        return Some("auto".to_owned());
    }
    if let Some(share) = text.strip_suffix("fr") {
        let share: f32 = share.parse().ok()?;
        return (share.is_finite() && share >= 0.0).then(|| format!("{share}fr"));
    }
    let length = Length::parse(text)?.resolve(viewport, Some(room))?;
    (length.is_finite() && length >= 0.0).then(|| length.to_string())
}

/// `grid-column` or `grid-row` as CSS writes it — `2`, `span 2`, `2 / 4`,
/// `2 / span 3`, `auto` — as a start line (zero for automatic) and a span.
fn placement(value: &str) -> Option<[u32; 2]> {
    let (start, end) = match value.split_once('/') {
        Some((start, end)) => (start.trim(), Some(end.trim())),
        None => (value.trim(), None),
    };
    let span_of = |text: &str| -> Option<u32> {
        text.strip_prefix("span")
            .and_then(|count| count.trim().parse().ok())
            .filter(|count| *count > 0)
    };
    let line = |text: &str| -> Option<u32> { text.parse().ok().filter(|line| *line > 0) };
    let (first, spans_alone) = if start == "auto" {
        (0, None)
    } else if let Some(span) = span_of(start) {
        (0, Some(span))
    } else {
        (line(start)?, None)
    };
    let span = match end {
        None | Some("auto") => spans_alone.unwrap_or(1),
        Some(end) => match span_of(end) {
            Some(span) => span,
            None if first > 0 => line(end)?.checked_sub(first).filter(|span| *span > 0)?,
            None => return None,
        },
    };
    Some([first, span])
}

#[cfg(test)]
mod tests {
    use super::{placement, tracks};
    use weave::Viewport;

    const SCREEN: Viewport = Viewport {
        width: 800.0,
        height: 800.0,
    };

    #[test]
    fn track_lists_read_as_css_writes_them() {
        // 800 pixels high is two units, so 200px is half a unit.
        assert_eq!(
            tracks("1fr 200px auto", SCREEN, 2.0),
            Some(vec!["1fr".into(), "0.5".into(), "auto".into()])
        );
        assert_eq!(
            tracks("repeat(3, 1fr) 50%", SCREEN, 2.0),
            Some(vec!["1fr".into(), "1fr".into(), "1fr".into(), "1".into()])
        );
        assert_eq!(tracks("minmax(10px, 1fr)", SCREEN, 2.0), None);
        assert_eq!(tracks("", SCREEN, 2.0), None);
    }

    #[test]
    fn placements_read_as_css_writes_them() {
        assert_eq!(placement("2"), Some([2, 1]));
        assert_eq!(placement("span 2"), Some([0, 2]));
        assert_eq!(placement("2 / 4"), Some([2, 2]));
        assert_eq!(placement("2 / span 3"), Some([2, 3]));
        assert_eq!(placement("auto"), Some([0, 1]));
        assert_eq!(placement("3 / 2"), None);
        assert_eq!(placement("0"), None);
    }
}
