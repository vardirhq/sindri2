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
