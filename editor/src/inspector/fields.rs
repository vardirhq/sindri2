//! Which fields a component shows, and in what order.
//!
//! The panel draws the stored payload, and a payload holds only what someone
//! wrote down: a field left at its default is simply absent, so two sprites
//! could show two different sets of rows while being the same component. One
//! showed a Layer and the other did not, and nothing about either said why.
//!
//! So the rows are the component's whole field set — what the registry says a
//! fresh one holds, plus whatever this payload holds beyond that. A field that
//! is absent is drawn at the value it already has in effect, and writing it is
//! what puts it in the file. Nothing is written by looking.

use std::collections::BTreeMap;

use serde_json::Value;

/// The payload as it is drawn: what is stored, filled out with the defaults of
/// whatever it leaves unsaid.
///
/// `defaults` is the registry's own blank for this component, or `None` for a
/// type that has none — an unknown component, or one whose blank cannot be
/// invented. Such a type shows exactly what it stores, which is all anyone can
/// honestly say about it.
#[must_use]
pub fn drawn_payload(defaults: Option<&Value>, payload: &Value) -> Value {
    let (Some(Value::Object(defaults)), Value::Object(stored)) = (defaults, payload) else {
        return payload.clone();
    };
    let mut drawn = stored.clone();
    for (key, value) in defaults {
        // A stored field wins: the default is what the field means when nobody
        // has said, not a value to impose on someone who has.
        drawn.entry(key.clone()).or_insert_with(|| value.clone());
    }
    Value::Object(drawn)
}

/// Folds what was drawn back into what is stored.
///
/// A field the payload did not have is written only if it was actually changed
/// away from its default — so opening an inspector does not add every field of
/// every component to the file, and a scene's diff stays the edit that was
/// made rather than the panels that were opened.
///
/// A field the drawn payload no longer has is removed. That is how a choice
/// which decides what else a component holds takes the other choice's fields
/// away with it: a camera switched to orthographic must stop carrying a
/// vertical field of view, or the payload describes two cameras.
pub fn merge_edits(defaults: Option<&Value>, payload: &mut Value, drawn: &Value) {
    let (Value::Object(stored), Value::Object(drawn)) = (&mut *payload, drawn) else {
        *payload = drawn.clone();
        return;
    };
    let blank = defaults.and_then(Value::as_object);
    stored.retain(|key, _| drawn.contains_key(key));
    for (key, value) in drawn {
        if stored.contains_key(key) {
            stored.insert(key.clone(), value.clone());
            continue;
        }
        let untouched = blank
            .and_then(|blank| blank.get(key))
            .is_some_and(|default| default == value);
        if !untouched {
            stored.insert(key.clone(), value.clone());
        }
    }
}

/// The order the fields of one payload are drawn in.
///
/// Alphabetical is what a stored map gives, and alphabetical put a camera's
/// Far above its Near and buried the one field that decides what the other
/// three mean. So each key is ranked by what it is — which the panel judges
/// from the key's own name, the same guess [`super::axis_labels`] makes — and
/// ties stay alphabetical so the order is stable.
///
/// Three ranks: what the component *is* (its kind, and the asset it draws),
/// then what it holds, then how it is drawn. It is a heuristic and says so; a
/// key it has never seen lands in the middle, which is where an ordinary field
/// belongs.
#[must_use]
pub fn ordered_keys(payload: &Value) -> Vec<String> {
    let Value::Object(fields) = payload else {
        return Vec::new();
    };
    let mut ranked: BTreeMap<(u8, String), String> = BTreeMap::new();
    for key in fields.keys() {
        ranked.insert((rank(key), key.clone()), key.clone());
    }
    ranked.into_values().collect()
}

fn rank(key: &str) -> u8 {
    match key {
        // What this component is, and what it draws.
        "projection" | "kind" | "shape" | "primitive" | "texture" | "font" | "text" | "source"
        | "script" | "clip" | "grid" => 0,
        // How it is drawn, once everything about what it is has been said.
        "anchor" | "tint" | "color" | "colour" | "layer" | "opacity" => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{drawn_payload, merge_edits, ordered_keys};
    use serde_json::json;

    /// Two of one component now show the same rows, whatever either of them
    /// happens to have written down.
    #[test]
    fn a_component_shows_every_field_it_has() {
        let defaults =
            json!({ "texture": "procedural:checkerboard", "tint": [1, 1, 1, 1], "layer": 0 });
        let stored = json!({ "texture": "orb.png" });
        let drawn = drawn_payload(Some(&defaults), &stored);
        assert_eq!(
            drawn,
            json!({ "texture": "orb.png", "tint": [1, 1, 1, 1], "layer": 0 }),
            "what is stored wins; what is missing is shown at what it means"
        );
    }

    #[test]
    fn a_component_with_no_blank_shows_what_it_stores() {
        let stored = json!({ "hp": 3 });
        assert_eq!(drawn_payload(None, &stored), stored);
    }

    /// Opening a panel is not an edit. Only a field actually moved away from
    /// its default is written, so a scene's diff is what someone changed.
    #[test]
    fn looking_at_a_default_does_not_write_it() {
        let defaults = json!({ "texture": "a.png", "tint": [1, 1, 1, 1], "layer": 0 });
        let mut stored = json!({ "texture": "orb.png" });
        let drawn = drawn_payload(Some(&defaults), &stored);
        merge_edits(Some(&defaults), &mut stored, &drawn);
        assert_eq!(stored, json!({ "texture": "orb.png" }));

        let mut edited = drawn.clone();
        edited["layer"] = json!(7);
        merge_edits(Some(&defaults), &mut stored, &edited);
        assert_eq!(
            stored,
            json!({ "texture": "orb.png", "layer": 7 }),
            "and the one that moved is written, alone"
        );
    }

    /// A field the payload already carried is written back even when it is
    /// edited to what the default happens to be: it was authored, and an edit
    /// is not a reason to quietly remove it.
    #[test]
    fn a_stored_field_stays_stored() {
        let defaults = json!({ "layer": 0 });
        let mut stored = json!({ "layer": 5 });
        let mut drawn = drawn_payload(Some(&defaults), &stored);
        drawn["layer"] = json!(0);
        merge_edits(Some(&defaults), &mut stored, &drawn);
        assert_eq!(stored, json!({ "layer": 0 }));
    }

    #[test]
    fn what_a_component_is_reads_before_how_it_is_drawn() {
        let camera = json!({
            "far": 100.0,
            "near": 0.1,
            "projection": "perspective",
            "vertical_fov_degrees": 60.0
        });
        assert_eq!(
            ordered_keys(&camera),
            ["projection", "far", "near", "vertical_fov_degrees"],
            "the field that decides what the others mean comes first"
        );

        let sprite = json!({ "layer": 3, "texture": "orb.png", "tint": [1, 1, 1, 1] });
        assert_eq!(
            ordered_keys(&sprite),
            ["texture", "layer", "tint"],
            "what it draws first, then the two that say how, alphabetically"
        );
    }
}

#[cfg(test)]
mod removal_tests {
    use super::{drawn_payload, merge_edits};
    use crate::inspector::choices::choose;
    use serde_json::json;

    /// The whole of what a variant switch has to do, end to end: the payload
    /// that reaches the world is one camera rather than the fields of two.
    #[test]
    fn a_variant_switch_leaves_one_cameras_worth_of_fields() {
        let defaults = json!({
            "projection": "perspective",
            "vertical_fov_degrees": 60.0,
            "near": 0.1,
            "far": 100.0
        });
        let mut stored = json!({
            "projection": "perspective",
            "vertical_fov_degrees": 45.0,
            "near": 0.1,
            "far": 100.0
        });
        let mut drawn = drawn_payload(Some(&defaults), &stored);
        choose("sindri.camera", "projection", "orthographic", &mut drawn);
        merge_edits(Some(&defaults), &mut stored, &drawn);

        assert_eq!(stored["projection"], json!("orthographic"));
        assert!(
            stored.get("vertical_fov_degrees").is_none(),
            "the projection it was switched away from took its field with it"
        );
        assert!(stored["vertical_size"].as_f64().is_some());
    }
}
