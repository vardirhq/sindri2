//! Editing what an entity's components hold.
//!
//! Until this existed the inspector showed components as read-only labels,
//! hand-written per type: a `sindri.camera` had three lines because someone
//! typed three lines. A component the code did not know about showed nothing,
//! and none of it could be changed. Every component an author wanted to set was
//! set by editing the scene file by hand.
//!
//! The values are edited from the **stored payload** rather than from a typed
//! view. That is the same rule `sindri-decay` follows for the same reason: a
//! component is a `Deserialize`-only view over a payload, the payload is what
//! gets written back, and rebuilding one from a view drops whatever the view
//! does not know about. Editing the payload also means a component nothing
//! understands is still editable, which is what `UnknownComponentPolicy::Preserve`
//! promises and could not previously deliver.
//!
//! What keeps that safe is the registry. Every edit is checked against the
//! component's own schema before it becomes a command, so a payload cannot be
//! edited into something the engine would refuse to load — the refusal happens
//! while the author is still looking at the field.

use serde_json::Value;

/// What kind of widget a stored value gets.
///
/// Decided from the value rather than from a schema, because the payload is
/// what exists: a schema describing fields would be a second description to
/// keep in step, and the registry's validator already covers the part that
/// matters, which is whether an edit is still valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    Number,
    Bool,
    Text,
    /// A short array of numbers — a position, a tint, a UV rect — drawn as a
    /// row rather than as a nested list, because that is what it is.
    Numbers(usize),
    /// A nested object, drawn indented with its own rows.
    Object,
    /// Something with no safe editor: a mixed array, an array of objects, or
    /// null. Shown as it is stored and left alone, which is better than a text
    /// field that can turn a scene into something that will not load.
    Opaque,
}

/// How wide an array can be and still read as one row of numbers.
///
/// Four covers every fixed-size value the engine has — a tint, a quaternion, a
/// UV rect — and stops a tilemap's thousand tiles from being drawn as a row.
const INLINE_NUMBERS: usize = 4;

#[must_use]
pub fn value_kind(value: &Value) -> ValueKind {
    match value {
        Value::Number(_) => ValueKind::Number,
        Value::Bool(_) => ValueKind::Bool,
        Value::String(_) => ValueKind::Text,
        Value::Object(_) => ValueKind::Object,
        Value::Array(items)
            if !items.is_empty()
                && items.len() <= INLINE_NUMBERS
                && items.iter().all(Value::is_number) =>
        {
            ValueKind::Numbers(items.len())
        }
        _ => ValueKind::Opaque,
    }
}

/// The label a stored key gets in the panel.
///
/// `vertical_fov_degrees` becomes "Vertical fov degrees". Derived rather than
/// tabulated, so a component that gains a field gains a readable label without
/// anyone remembering to add one.
#[must_use]
pub fn humanize(key: &str) -> String {
    let mut label = key.replace('_', " ");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}

/// What one axis of an inline number row is called.
///
/// Position-like values read as x/y/z/w and colours as r/g/b/a, which is the
/// only place the panel guesses at meaning — and it guesses from the key,
/// which is the author's own word for it.
#[must_use]
pub fn axis_labels(key: &str, len: usize) -> Vec<String> {
    let spatial = ["X", "Y", "Z", "W"];
    let colour = ["R", "G", "B", "A"];
    let rect = ["X", "Y", "W", "H"];
    let names: &[&str; 4] = match key {
        "tint" | "color" | "colour" => &colour,
        "uv_rect" => &rect,
        _ => &spatial,
    };
    (0..len)
        .map(|index| {
            names
                .get(index)
                .map_or_else(|| index.to_string(), |name| (*name).to_owned())
        })
        .collect()
}

/// Whether a field applies, given the rest of its component's payload.
///
/// A world-space sprite has no edge to anchor to, so its stored anchor decides
/// nothing — and a control that looks like it does something but does not is
/// the exact failure `docs/editor-audit.md` was written to remove. Hiding it is
/// the same answer `SpriteComponent::screen_anchor` gives in the engine, in the
/// one place a panel can give it.
///
/// This is deliberately a short list rather than a general mechanism. Every
/// entry is a rule that already exists in the engine; inventing one here would
/// be a second opinion about what a component means.
#[must_use]
pub fn applies(type_name: &str, key: &str, payload: &Value) -> bool {
    match (type_name, key) {
        ("sindri.sprite", "anchor") => {
            payload.get("space").and_then(Value::as_str) != Some("world")
        }
        // These are edited as one visual grid. Exposing columns and rows as
        // independent numbers produces an invalid cell count, while drawing
        // the compact palette and cell array as opaque JSON offers no authoring
        // at all.
        ("sindri.tilemap", "columns" | "rows" | "palette" | "tiles") => false,
        _ => true,
    }
}

/// Whether a component may be removed from an entity by the panel.
///
/// A script is removable, a sprite is removable. Nothing is protected today,
/// but the question is asked in one place so that when something is — a
/// component another depends on — the answer has somewhere to live.
#[must_use]
pub const fn is_removable(_type_name: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{ValueKind, axis_labels, humanize, value_kind};
    use serde_json::json;

    #[test]
    fn a_value_gets_the_widget_its_shape_deserves() {
        assert_eq!(value_kind(&json!(1.5)), ValueKind::Number);
        assert_eq!(value_kind(&json!(true)), ValueKind::Bool);
        assert_eq!(value_kind(&json!("badge.png")), ValueKind::Text);
        assert_eq!(value_kind(&json!([1.0, 2.0, 3.0])), ValueKind::Numbers(3));
        assert_eq!(value_kind(&json!({ "a": 1 })), ValueKind::Object);
    }

    /// Everything without a safe editor is shown and left alone, rather than
    /// offered a text field that could turn a scene into one that will not load.
    #[test]
    fn what_cannot_be_edited_safely_is_opaque() {
        assert_eq!(value_kind(&json!(null)), ValueKind::Opaque);
        assert_eq!(value_kind(&json!([])), ValueKind::Opaque);
        assert_eq!(
            value_kind(&json!([1.0, "two"])),
            ValueKind::Opaque,
            "a mixed array is not a vector"
        );
        assert_eq!(
            value_kind(&json!([1, 2, 3, 4, 5])),
            ValueKind::Opaque,
            "and a long one is not a row"
        );
        assert_eq!(value_kind(&json!([{ "a": 1 }])), ValueKind::Opaque);
    }

    /// A world-space sprite has no edge to anchor to, so it is offered no
    /// anchor to set as though it did.
    #[test]
    fn a_field_that_decides_nothing_is_not_offered() {
        let screen = json!({ "texture": "a.png", "anchor": "top_left" });
        assert!(super::applies("sindri.sprite", "anchor", &screen));

        let world = json!({ "texture": "a.png", "space": "world", "anchor": "top_left" });
        assert!(!super::applies("sindri.sprite", "anchor", &world));
        assert!(
            super::applies("sindri.sprite", "texture", &world),
            "and everything that does decide something still is"
        );
    }

    #[test]
    fn tilemap_storage_is_replaced_by_its_visual_editor() {
        let map = json!({
            "texture": "tiles.png",
            "columns": 2,
            "rows": 2,
            "palette": ["floor"],
            "tiles": [0, null, null, 0]
        });
        for key in ["columns", "rows", "palette", "tiles"] {
            assert!(!super::applies("sindri.tilemap", key, &map));
        }
        assert!(super::applies("sindri.tilemap", "texture", &map));
        assert!(super::applies("sindri.tilemap", "layer", &map));
    }

    #[test]
    fn a_stored_key_reads_as_a_label() {
        assert_eq!(humanize("vertical_fov_degrees"), "Vertical fov degrees");
        assert_eq!(humanize("layer"), "Layer");
        assert_eq!(humanize(""), "");
    }

    /// The one place the panel guesses at meaning, and it guesses from the
    /// author's own word for the field.
    #[test]
    fn a_number_row_is_labelled_by_what_the_field_is_called() {
        assert_eq!(axis_labels("position", 3), ["X", "Y", "Z"]);
        assert_eq!(axis_labels("tint", 4), ["R", "G", "B", "A"]);
        assert_eq!(axis_labels("uv_rect", 4), ["X", "Y", "W", "H"]);
        assert_eq!(axis_labels("something", 2), ["X", "Y"]);
    }
}
