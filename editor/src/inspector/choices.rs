//! Fields whose value is one of a few named things, and what picking one writes.
//!
//! A stored string is a text box unless something says otherwise, and for an
//! enum that is a trap: `perspective` is a camera projection and `perspectve`
//! is a scene that will not load. Worse, some of these strings decide which
//! *other* fields the component has, so typing the other name by hand produces
//! a payload missing half of itself, which the schema then refuses — a field
//! that looks editable and cannot be edited.
//!
//! Both halves are the component's own business now. Which spellings a field
//! accepts is asked of the schema registry, and so is what each spelling makes
//! the component hold: the registry checked at startup that choosing any of
//! them produces a component the engine accepts.
//!
//! What is left here is the editor's side of that — the panel has a payload and
//! a path, and needs the one call that turns a picked word into a whole edit.
//! It used to be a hand-written rule that knew a camera's two projections by
//! name, which is why a collider piece's shape had no picker at all.

use serde_json::Value;
use sindri_core::ComponentSchemaRegistry;

/// The registry's blank, rewritten for the variant this payload actually is.
///
/// A registry holds one blank per component, and for a tagged component that
/// blank is one variant of it — a fresh camera is a perspective one. Filling an
/// orthographic camera's missing fields from that blank would give it a
/// vertical field of view as well as a vertical size, which is the payload of
/// two cameras. So the blank is put through the same switch the author's own
/// choice goes through, and comes out describing the same variant they are
/// looking at.
///
/// Only a tag the component itself carries. A tag inside a list is one tag per
/// item — each piece of a collider has its own shape — and a blank holds one
/// exemplar rather than one per item, so there is nothing there to rewrite. The
/// items are drawn from what they store.
#[must_use]
pub fn blank_for(
    registry: &ComponentSchemaRegistry,
    type_name: &str,
    defaults: &Value,
    payload: &Value,
) -> Value {
    let mut blank = defaults.clone();
    for tag in registry.variant_tags(type_name) {
        let Some(chosen) = tag
            .find(['.', '['])
            .is_none()
            .then(|| payload.get(tag).and_then(Value::as_str))
            .flatten()
        else {
            continue;
        };
        registry.switch_variant(type_name, tag, chosen, &mut blank);
    }
    blank
}

/// Writes a chosen value into the object that holds it, along with whatever
/// else the choice decides.
///
/// `holder` is the object the field belongs to rather than the field itself,
/// because for a tagged field those are different edits: switching a piece's
/// shape from a box to a circle takes the half extents away and puts a radius
/// there, and a caller holding only the word could do neither.
///
/// A choice that decides only itself — most of them — writes only itself.
pub fn choose(
    registry: &ComponentSchemaRegistry,
    type_name: &str,
    path: &str,
    chosen: &str,
    holder: &mut Value,
) {
    if registry.switch_variant(type_name, path, chosen, holder) {
        return;
    }
    let key = path.rsplit('.').next().unwrap_or(path);
    if let Some(fields) = holder.as_object_mut() {
        fields.insert(key.to_owned(), Value::String(chosen.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::{blank_for, choose};
    use serde_json::json;
    use sindri_core::ComponentSchemaRegistry;

    /// A choice nothing describes the shape of writes the word and stops. Most
    /// choices are this: a text element holds the same fields whichever way it
    /// wraps.
    #[test]
    fn a_plain_choice_writes_only_itself() {
        let registry = ComponentSchemaRegistry::default();
        let mut image = json!({ "texture": "a.png", "anchor": "center" });
        choose(
            &registry,
            "sindri.ui.image",
            "anchor",
            "top_left",
            &mut image,
        );
        assert_eq!(
            image,
            json!({ "texture": "a.png", "anchor": "top_left" }),
            "nothing else about the element changed"
        );
    }

    /// The path names the field being looked at, so the word lands on that
    /// field and not on the last thing the path happens to mention.
    #[test]
    fn a_nested_choice_writes_the_field_the_path_names() {
        let registry = ComponentSchemaRegistry::default();
        let mut piece = json!({ "shape": "box", "half_extents": [0.5, 0.5] });
        choose(
            &registry,
            "some.component",
            "pieces.2.shape.shape",
            "circle",
            &mut piece,
        );
        assert_eq!(piece["shape"], json!("circle"));
    }

    /// A component with no variants keeps its blank: there is nothing to
    /// rewrite it for.
    #[test]
    fn a_blank_with_no_variants_is_the_blank() {
        let registry = ComponentSchemaRegistry::default();
        let defaults = json!({ "texture": "a.png", "layer": 0 });
        assert_eq!(
            blank_for(&registry, "sindri.sprite", &defaults, &json!({})),
            defaults
        );
    }
}
