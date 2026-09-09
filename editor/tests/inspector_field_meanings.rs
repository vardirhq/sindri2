//! What the inspector is told about a field, now that it stops guessing.
//!
//! The panel used to work out a control from a field's *name*: `texture` meant
//! the texture list, `clip` meant the audio list, a colour had to be spelled
//! `tint`. Those tables are gone, so this is the guard that nothing they got
//! right was lost with them — and that the two things they got wrong stay
//! fixed.

use sindri_core::{AssetKind, FieldMeaning};
use sindri_editor::native::scene_extractor;

/// Every dropdown the old table drew is still a dropdown.
#[test]
fn no_choice_the_editor_offered_was_lost() {
    let scene = scene_extractor();
    let components = scene.components();
    for (type_name, key) in [
        ("sindri.camera", "projection"),
        ("sindri.ui.image", "anchor"),
        ("sindri.ui.text", "anchor"),
        ("sindri.ui.text", "wrap"),
        ("sindri.ui.text", "line_align"),
        ("sindri.ui.text", "case"),
        ("sindri.tilemap", "projection"),
        ("sindri.physics2d.rigid_body", "kind"),
    ] {
        let meaning = components.meaning(type_name, key);
        assert!(
            matches!(meaning, Some(FieldMeaning::Choice(options)) if !options.is_empty()),
            "{type_name}.{key} used to be chosen from a list and now is {meaning:?}"
        );
    }
}

/// Every picker the old table drew is still a picker, including the script
/// source the editor registers itself.
#[test]
fn no_asset_picker_the_editor_offered_was_lost() {
    let scene = scene_extractor();
    let components = scene.components();
    for (type_name, key, kind) in [
        ("sindri.sprite", "texture", AssetKind::Texture),
        ("sindri.mesh", "texture", AssetKind::Texture),
        ("sindri.ui.image", "texture", AssetKind::Texture),
        ("sindri.tilemap", "texture", AssetKind::Texture),
        ("sindri.effect.burst", "texture", AssetKind::Texture),
        ("sindri.ui.text", "font", AssetKind::Font),
        ("sindri.audio.source", "clip", AssetKind::Audio),
        ("sindri.script", "source", AssetKind::Script),
    ] {
        assert_eq!(
            components.meaning(type_name, key),
            Some(&FieldMeaning::Asset(kind)),
            "{type_name}.{key} used to open a picker"
        );
    }
}

/// The first thing the name guess got wrong: a bare key matched any component.
///
/// `(_, "clip")` offered the project's audio to anything with a `clip` field,
/// including a game's own component that meant something else entirely. Meaning
/// is per component now, so a name alone claims nothing.
#[test]
fn a_field_name_alone_no_longer_claims_an_asset() {
    let scene = scene_extractor();
    let components = scene.components();
    // `sindri.animation`'s clips are authored frame ranges, not audio, and it
    // has a `speed` rather than a `clip`; the point is that nothing answers for
    // a component that never said.
    assert_eq!(components.meaning("sindri.tags", "tags"), None);
    assert_eq!(components.meaning("game.unregistered", "texture"), None);
}

/// The second thing it got wrong: a colour had to be spelled a certain way.
///
/// `sindri.ui.text` stores its colour as `color` and its outline's as
/// `outline.color`; the old check saw the first and never the second, because
/// it only ever looked at top-level keys.
#[test]
fn a_colour_is_named_by_the_component_not_by_its_spelling() {
    let scene = scene_extractor();
    let components = scene.components();
    assert_eq!(
        components.meaning("sindri.ui.text", "color"),
        Some(&FieldMeaning::Colour)
    );
    assert_eq!(
        components.meaning("sindri.ui.text", "outline.color"),
        Some(&FieldMeaning::Colour)
    );
    assert_eq!(
        components.meaning("sindri.shape", "stroke"),
        Some(&FieldMeaning::Colour),
        "a shape's stroke is a colour that the old spelling rule never matched"
    );
}

/// A compound collider's pieces are a list the panel can build from.
///
/// This is what made array-of-object editing possible: the template carries one
/// exemplar piece, so the editor knows what a piece consists of and what a
/// fresh one is without inventing either.
#[test]
fn a_colliders_pieces_are_a_list_with_a_blank_to_add() {
    let scene = scene_extractor();
    let components = scene.components();
    let pieces = components
        .exemplar("sindri.physics2d.collider", "pieces")
        .and_then(|value| value.as_array())
        .expect("a collider's pieces are a list");
    let blank = pieces.first().expect("with one exemplar piece");
    assert!(
        blank.is_object(),
        "a piece is an object, so it can be drawn"
    );
    for field in [
        "shape",
        "offset",
        "rotation",
        "sensor",
        "layers",
        "friction",
        "restitution",
    ] {
        assert!(
            blank.get(field).is_some(),
            "a fresh piece should carry {field}"
        );
    }
    // And the engine accepts what that blank would produce, so adding one
    // cannot write a scene that will not load.
    components
        .validate_payload(
            "sindri.physics2d.collider",
            &serde_json::json!({ "pieces": [blank, blank] }),
        )
        .expect("a collider of two fresh pieces is valid");
}

/// A list is decided by the template, so the shapes that are not lists stay
/// readouts.
///
/// A tilemap's thousand tiles and a footprint's pairs of numbers are arrays
/// too, and drawing either as an editable list of objects would be wrong.
#[test]
fn arrays_that_are_not_lists_of_objects_are_not_treated_as_lists() {
    let scene = scene_extractor();
    let components = scene.components();
    for (type_name, path) in [
        ("sindri.tilemap", "tiles"),
        ("sindri.tilemap", "palette"),
        ("sindri.grid.occupant", "footprint"),
        ("sindri.tags", "tags"),
    ] {
        let is_list = components
            .exemplar(type_name, path)
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .is_some_and(serde_json::Value::is_object);
        assert!(!is_list, "{type_name}.{path} should not draw as a list");
    }
}
