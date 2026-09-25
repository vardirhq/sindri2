//! Flexbox properties, written to the layout they describe.
//!
//! A container's (`flex-direction`, `justify-content`, `align-items`,
//! `flex-wrap`, `gap`) go to its `sindri.ui.layout`, which must already be
//! there: styling does not turn an element into a layout. An item's
//! (`flex-grow`, `flex-shrink`, `flex-basis`, `order`, `align-self`) go to its
//! `sindri.ui.box`, which is added if it is missing, because any element can
//! be an item in its parent's layout.
//!
//! CSS's `flex-start` and `flex-end` are accepted beside `start` and `end`.

use sindri_core::{EntityId, World};
use weave::Viewport;

use crate::computed::Length;
use crate::{ApplyError, box_model, invalid, set_component_field};

const LAYOUT: &str = "sindri.ui.layout";

/// Applies `property` if it is a flexbox property, answering whether it was.
pub(crate) fn apply(
    world: &mut World,
    entity: EntityId,
    id: &str,
    property: &str,
    value: &str,
    viewport: Viewport,
) -> Result<bool, ApplyError> {
    let refuse = || invalid(id, property, value);
    let trimmed = value.trim();
    match property {
        "direction" | "flex-direction" => {
            let stored = match trimmed {
                "row" => "row",
                "column" => "column",
                _ => return Err(refuse()),
            };
            set_component_field(world, entity, LAYOUT, "direction", stored.into());
        }
        "justify-content" => {
            let stored = match trimmed {
                "start" | "flex-start" | "left" => "start",
                "center" => "center",
                "end" | "flex-end" | "right" => "end",
                "space-between" | "space_between" => "space_between",
                "space-around" => "space_around",
                "space-evenly" => "space_evenly",
                _ => return Err(refuse()),
            };
            set_component_field(world, entity, LAYOUT, "justify", stored.into());
        }
        "align-items" => {
            let stored = alignment(trimmed).ok_or_else(refuse)?;
            set_component_field(world, entity, LAYOUT, "align", stored.into());
            crate::grid::align_items(world, entity, stored);
        }
        "flex-wrap" => {
            let wraps = match trimmed {
                "wrap" => true,
                "nowrap" => false,
                _ => return Err(refuse()),
            };
            set_component_field(world, entity, LAYOUT, "wrap", wraps.into());
        }
        "flex-grow" | "flex-shrink" => {
            let factor = trimmed
                .parse::<f32>()
                .ok()
                .filter(|factor| factor.is_finite() && *factor >= 0.0)
                .ok_or_else(refuse)?;
            let field = if property == "flex-grow" {
                "grow"
            } else {
                "shrink"
            };
            box_model::set_field(world, entity, field, factor.into());
        }
        "flex-basis" => {
            let basis = if matches!(trimmed, "auto" | "content") {
                -1.0
            } else {
                let room = box_model::containing_main(world, entity, viewport);
                Length::parse(trimmed)
                    .and_then(|length| length.resolve(viewport, Some(room)))
                    .filter(|basis| *basis >= 0.0)
                    .ok_or_else(refuse)?
            };
            box_model::set_field(world, entity, "basis", basis.into());
        }
        "order" => {
            let order = trimmed.parse::<i32>().map_err(|_| refuse())?;
            box_model::set_field(world, entity, "order", order.into());
        }
        "align-self" => {
            let stored = if trimmed == "auto" {
                "auto"
            } else {
                alignment(trimmed).ok_or_else(refuse)?
            };
            box_model::set_field(world, entity, "align_self", stored.into());
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn alignment(value: &str) -> Option<&'static str> {
    match value {
        "start" | "flex-start" => Some("start"),
        "center" => Some("center"),
        "end" | "flex-end" => Some("end"),
        "stretch" => Some("stretch"),
        _ => None,
    }
}
