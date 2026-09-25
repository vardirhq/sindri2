//! `box-shadow`, written to the element's `sindri.ui.shape`.
//!
//! As in CSS: `box-shadow: <x> <y> [<blur> [<spread>]] <color>`, or `none`.
//! One shadow per element; a comma-separated list and `inset` are refused
//! rather than half drawn.

use sindri_core::{EntityId, World};
use weave::Viewport;

use crate::{ApplyError, color, invalid, length, set_component_field};

pub(crate) fn apply(
    world: &mut World,
    entity: EntityId,
    id: &str,
    value: &str,
    viewport: Viewport,
) -> Result<(), ApplyError> {
    let shadow = parse(value, viewport).ok_or_else(|| invalid(id, "box-shadow", value))?;
    set_component_field(world, entity, "sindri.ui.shape", "shadow", shadow);
    Ok(())
}

fn parse(value: &str, viewport: Viewport) -> Option<serde_json::Value> {
    let value = value.trim();
    if value == "none" {
        return Some(serde_json::json!({ "color": [0.0, 0.0, 0.0, 0.0] }));
    }
    let parts = weave::shorthand::words(value);
    if value.contains(',') || parts.contains(&"inset") {
        return None;
    }
    let mut lengths = Vec::new();
    let mut tint = None;
    for part in parts {
        if let Some(resolved) = length(part, viewport) {
            lengths.push(resolved);
        } else if tint.is_none() {
            tint = Some(color(part)?);
        } else {
            return None;
        }
    }
    let (x, y, blur, spread) = match lengths.as_slice() {
        [x, y] => (*x, *y, 0.0, 0.0),
        [x, y, blur] => (*x, *y, *blur, 0.0),
        [x, y, blur, spread] => (*x, *y, *blur, *spread),
        _ => return None,
    };
    if blur < 0.0 {
        return None;
    }
    // CSS's default shadow colour is the text colour; black at the opacity
    // most shadows are written at is the nearer thing a shape has.
    let tint = tint.unwrap_or([0.0, 0.0, 0.0, 0.5]);
    Some(serde_json::json!({
        "color": tint,
        // CSS measures y downwards; the overlay measures it up.
        "offset": [x, -y],
        "blur": blur,
        "spread": spread,
    }))
}

#[cfg(test)]
mod tests {
    use super::parse;
    use weave::Viewport;

    const SCREEN: Viewport = Viewport {
        width: 1_200.0,
        height: 800.0,
    };

    #[test]
    fn offsets_blur_spread_and_colour_read_as_in_css() {
        // 800 pixels high is two overlay units, so 40px is 0.1.
        let shadow = parse("0 40px 80px 4px #000000", SCREEN).expect("a shadow");
        let offset = shadow["offset"].as_array().expect("an offset");
        assert!((offset[1].as_f64().expect("y") + 0.1).abs() < 1.0e-6);
        assert!((shadow["blur"].as_f64().expect("blur") - 0.2).abs() < 1.0e-6);
        assert!((shadow["spread"].as_f64().expect("spread") - 0.01).abs() < 1.0e-6);
    }

    #[test]
    fn what_is_not_drawn_yet_is_refused() {
        assert!(parse("0 4px 8px red, 0 1px black", SCREEN).is_none());
        assert!(parse("inset 0 4px black", SCREEN).is_none());
        assert!(parse("4px", SCREEN).is_none());
        assert!(parse("0 4px -1px black", SCREEN).is_none());
        assert!(parse("none", SCREEN).is_some());
    }
}
