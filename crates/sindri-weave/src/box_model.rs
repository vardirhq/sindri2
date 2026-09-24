//! `margin` and `padding`, written to the element's `sindri.ui.box`.
//!
//! The language expands `padding: 4px 8px` into its four longhands, so what
//! reaches here is nearly always one side at a time. A shorthand built from
//! `var()` could not be split until it was substituted, and arrives whole.
//!
//! As in CSS, a percentage on any side is of the containing element's content
//! width — the vertical sides too, which is CSS's rule and keeps a box's
//! padding even when only one side is written as a percentage.

use sindri_core::{EntityId, World};
use weave::Viewport;

use crate::computed::Length;
use crate::{ApplyError, invalid};

const BOX: &str = "sindri.ui.box";
const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// Applies `property` if it is a margin or padding, answering whether it was.
pub(crate) fn apply(
    world: &mut World,
    entity: EntityId,
    id: &str,
    property: &str,
    value: &str,
    viewport: Viewport,
) -> Result<bool, ApplyError> {
    let Some((field, side)) = sided(property) else {
        return Ok(false);
    };
    let basis = containing_width(world, entity, viewport);
    let resolve = |text: &str| {
        Length::parse(text)
            .and_then(|length| length.resolve(viewport, Some(basis)))
            .filter(|resolved| field == "margin" || *resolved >= 0.0)
            .ok_or_else(|| invalid(id, property, value))
    };
    match side {
        Some(index) => {
            let resolved = resolve(value)?;
            set_side(world, entity, field, index, resolved);
        }
        None => {
            let sides =
                weave::shorthand::split_sides(value).ok_or_else(|| invalid(id, property, value))?;
            for (index, text) in sides.into_iter().enumerate() {
                let resolved = resolve(text)?;
                set_side(world, entity, field, index, resolved);
            }
        }
    }
    Ok(true)
}

/// The element's padding, top, right, bottom, left, in overlay units.
pub(crate) fn padding(world: &World, entity: EntityId) -> [f32; 4] {
    let mut sides = [0.0; 4];
    let Some(stored) = world
        .get(entity)
        .and_then(|data| data.components.get(BOX))
        .and_then(|payload| payload.get("padding"))
        .and_then(serde_json::Value::as_array)
    else {
        return sides;
    };
    for (side, value) in sides.iter_mut().zip(stored) {
        #[allow(clippy::cast_possible_truncation)]
        let read = value.as_f64().unwrap_or(0.0) as f32;
        *side = read.max(0.0);
    }
    sides
}

/// Which box field and side a property writes: `padding-left` is padding's
/// fourth side, and a bare `padding` is all of them.
fn sided(property: &str) -> Option<(&'static str, Option<usize>)> {
    for field in ["margin", "padding"] {
        let Some(rest) = property.strip_prefix(field) else {
            continue;
        };
        if rest.is_empty() {
            return Some((field, None));
        }
        let side = rest.strip_prefix('-')?;
        return SIDES
            .iter()
            .position(|name| *name == side)
            .map(|index| (field, Some(index)));
    }
    None
}

/// The width a percentage is of: the parent's, inside its padding, or the
/// screen's for an element with no parent.
fn containing_width(world: &World, entity: EntityId, viewport: Viewport) -> f32 {
    let Some(parent) = world.get(entity).and_then(|data| data.parent) else {
        return 2.0 * viewport.width / viewport.height.max(1.0);
    };
    let width = world
        .get(parent)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default()
        .scale_2d()[0]
        .abs();
    let inset = padding(world, parent);
    (width - inset[1] - inset[3]).max(0.0)
}

fn set_side(world: &mut World, entity: EntityId, field: &str, index: usize, value: f32) {
    let Some(data) = world.get_mut(entity) else {
        return;
    };
    let payload = data.components.entry(BOX.to_owned()).or_insert_with(
        || serde_json::json!({ "margin": [0.0, 0.0, 0.0, 0.0], "padding": [0.0, 0.0, 0.0, 0.0] }),
    );
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let sides = object
        .entry(field.to_owned())
        .or_insert_with(|| serde_json::json!([0.0, 0.0, 0.0, 0.0]));
    if let Some(array) = sides.as_array_mut() {
        array.resize(4, 0.0.into());
        array[index] = value.into();
    }
}

#[cfg(test)]
mod tests {
    use super::sided;

    #[test]
    fn longhands_name_their_side_and_shorthands_every_side() {
        assert_eq!(sided("padding-left"), Some(("padding", Some(3))));
        assert_eq!(sided("margin-top"), Some(("margin", Some(0))));
        assert_eq!(sided("margin"), Some(("margin", None)));
        assert_eq!(sided("padding-middle"), None);
        assert_eq!(sided("paddings"), None);
        assert_eq!(sided("width"), None);
    }
}
