//! What the inspector is told about a field, now that it stops guessing.
//!
//! The panel used to work out a control from a field's *name*: `texture` meant
//! the texture list, `clip` meant the audio list, a colour had to be spelled
//! `tint`. Those tables are gone, so this is the guard that nothing they got
//! right was lost with them — and that the two things they got wrong stay
//! fixed.

use serde_json::json;
use sindri_core::{AssetKind, FieldMeaning};
use sindri_editor::inspector::choices::choose;
use sindri_editor::inspector::fields::{drawn_payload, merge_edits};
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

/// The whole of what a variant switch has to do, end to end: the payload that
/// reaches the world is one camera rather than the fields of two.
///
/// Against the registry the engine actually registers, so the two projections
/// are the ones a camera really has rather than a pair written for the test.
#[test]
fn switching_a_projection_leaves_one_cameras_worth_of_fields() {
    let scene = scene_extractor();
    let components = scene.components();
    let defaults = components
        .fields("sindri.camera")
        .expect("a camera has fields");
    let mut stored = json!({
        "projection": "perspective",
        "vertical_fov_degrees": 45.0,
        "near": 0.2,
        "far": 80.0
    });

    let mut drawn = drawn_payload(Some(defaults), &stored);
    choose(
        components,
        "sindri.camera",
        "projection",
        "orthographic",
        &mut drawn,
    );
    merge_edits(Some(defaults), &mut stored, &drawn);

    assert_eq!(stored["projection"], json!("orthographic"));
    assert!(
        stored.get("vertical_fov_degrees").is_none(),
        "the projection it was switched away from took its field with it"
    );
    assert!(stored["vertical_size"].as_f64().is_some());
    assert_eq!(stored["near"], json!(0.2), "the planes are shared");
    assert_eq!(stored["far"], json!(80.0));
    components
        .validate_payload("sindri.camera", &stored)
        .expect("the switched camera is one the engine accepts");
}

/// The same switch one level down, which is the thing that had no control at
/// all: a piece's shape decides what the piece measures.
#[test]
fn switching_a_pieces_shape_rewrites_that_piece_alone() {
    let scene = scene_extractor();
    let components = scene.components();
    let mut collider = components
        .default_payload("sindri.physics2d.collider")
        .expect("a collider is addable")
        .clone();
    collider["pieces"].as_array_mut().unwrap().push(
        components
            .exemplar("sindri.physics2d.collider", "pieces[]")
            .expect("a piece to add")
            .clone(),
    );
    collider["pieces"][0]["friction"] = json!(0.25);

    let switched = components.switch_variant(
        "sindri.physics2d.collider",
        "pieces.0.shape.shape",
        "circle",
        &mut collider["pieces"][0]["shape"],
    );

    assert!(
        switched,
        "a piece's shape is a variant the registry describes"
    );
    assert_eq!(collider["pieces"][0]["shape"]["shape"], json!("circle"));
    assert!(
        collider["pieces"][0]["shape"].get("half_extents").is_none(),
        "the box's measurement went with the box"
    );
    assert!(collider["pieces"][0]["shape"]["radius"].as_f64().is_some());
    assert_eq!(
        collider["pieces"][0]["friction"],
        json!(0.25),
        "the rest of the piece is not part of the switch"
    );
    assert_eq!(
        collider["pieces"][1]["shape"]["shape"],
        json!("box"),
        "and neither is any other piece"
    );
    components
        .validate_payload("sindri.physics2d.collider", &collider)
        .expect("a compound of a circle and a box is one the engine accepts");
}

/// Every spelling the editor offers is one it can write. The registry proves
/// this at startup for its own variants; this is the guard that the two lists
/// stay the same list.
#[test]
fn every_spelling_of_a_tagged_field_is_a_variant() {
    let scene = scene_extractor();
    let components = scene.components();
    for (type_name, tag) in [
        ("sindri.camera", "projection"),
        ("sindri.physics2d.collider", "pieces[].shape.shape"),
    ] {
        let Some(FieldMeaning::Choice(spellings)) = components.meaning(type_name, tag) else {
            panic!("{type_name}.{tag} is offered as a choice");
        };
        let variants = components
            .variants(type_name, tag)
            .expect("a tagged field describes its variants");
        assert_eq!(
            spellings.len(),
            variants.len(),
            "{type_name}.{tag} offers {} spellings and describes {} variants",
            spellings.len(),
            variants.len()
        );
        for spelling in spellings {
            assert!(
                variants.iter().any(|(name, _)| name == spelling),
                "{type_name}.{tag} offers '{spelling}' with nothing to write for it"
            );
        }
    }
}

/// A switch below the top level has to survive the merge, not only the draw.
///
/// The panel edits a drawn copy and `merge_edits` decides what is written back,
/// so a piece's new shape reaching the scene is a separate claim from the
/// switch itself working.
#[test]
fn a_switched_shape_reaches_the_scene() {
    let scene = scene_extractor();
    let components = scene.components();
    let defaults = components
        .fields("sindri.physics2d.collider")
        .expect("a collider has fields");
    let mut stored = components
        .default_payload("sindri.physics2d.collider")
        .expect("a collider is addable")
        .clone();

    let mut drawn = drawn_payload(Some(defaults), &stored);
    components.switch_variant(
        "sindri.physics2d.collider",
        "pieces.0.shape.shape",
        "capsule",
        &mut drawn["pieces"][0]["shape"],
    );
    merge_edits(Some(defaults), &mut stored, &drawn);

    assert_eq!(stored["pieces"][0]["shape"]["shape"], json!("capsule"));
    assert!(
        stored["pieces"][0]["shape"]["half_height"]
            .as_f64()
            .is_some()
    );
    assert!(stored["pieces"][0]["shape"].get("half_extents").is_none());
    components
        .validate_payload("sindri.physics2d.collider", &stored)
        .expect("what was written back is a collider the engine accepts");
}
