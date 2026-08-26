//! What a world says it needs loading, and whether that list is
//! complete.

use std::error::Error;

use sindri_scene::{FONT_NAMING_COMPONENTS, SceneExtractor, TEXTURE_NAMING_COMPONENTS};

use crate::support::{scene, world_from};

/// A component that names a texture and is not in `TEXTURE_NAMING_COMPONENTS`
/// never has that texture requested, so it draws as the magenta checker while
/// everything else about it works. `sindri.tilemap` did exactly that.
///
/// This holds the list against the registry rather than against a second list,
/// so the way to pass it is to add the component, not to update the test.
#[test]
fn every_component_that_names_a_texture_is_one_hosts_load_from() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    for metadata in extractor.components().registered_components() {
        let Some(default) = extractor.components().default_payload(&metadata.type_name) else {
            continue;
        };
        if default.get("texture").is_none() {
            continue;
        }
        assert!(
            TEXTURE_NAMING_COMPONENTS.contains(&metadata.type_name.as_str()),
            "{} names a texture but hosts never load it, so it draws magenta",
            metadata.type_name
        );
    }
}

/// The same guard for fonts, where the symptom is quieter still: a component
/// that names a font and is not in `FONT_NAMING_COMPONENTS` never has it
/// requested, so nothing binds it and the text draws as nothing at all — a
/// blank where a label should be, with no failed frame to point at it.
///
/// This cannot be held against default payloads the way the list above is: a
/// component that names a font has no sensible blank, because it cannot invent
/// a font, so it is registered without a default and would never reach the
/// assert. It is held against the *validator* instead. A component reads a
/// `font` field exactly when handing it a wrongly-typed one changes what
/// validation says, so the probe below finds every such component whether or
/// not it can be created from nothing.
///
/// Both directions are checked, so the list cannot go stale in either: a
/// component that reads a font has to be named, and a name that no longer reads
/// one has to go.
#[test]
fn every_component_that_names_a_font_is_one_hosts_load_from() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    let reads_a_font = |type_name: &str| {
        let complaint = |payload: &serde_json::Value| {
            extractor
                .components()
                .validate_payload(type_name, payload)
                .err()
                .and_then(|error| error.source().map(ToString::to_string))
        };
        complaint(&serde_json::json!({})) != complaint(&serde_json::json!({ "font": 0 }))
    };

    for metadata in extractor.components().registered_components() {
        assert_eq!(
            reads_a_font(&metadata.type_name),
            FONT_NAMING_COMPONENTS.contains(&metadata.type_name.as_str()),
            "{} reads a font field without being listed, or is listed without \
             reading one. The first draws no text at all, because hosts never \
             load a font for it; the second is a name that loads nothing.",
            metadata.type_name
        );
    }
}

/// An animated sprite needs its sheet even though its own reference names no
/// part of one. The trap this guards: which part an animated sprite draws is its
/// clip's business, so its `texture` carries no fragment — and a host that asks
/// for sheets by looking at fragments alone never asks for this one. The sprite
/// then resolves its whole texture, which is every frame of the sheet at once.
#[test]
fn an_animated_sprite_asks_for_the_sheet_its_clips_read() {
    let world = world_from(&scene(
        r#",
        { "id": "runner", "transform_3d": {},
          "components": {
            "sindri.sprite": { "texture": "textures/walk.png" },
            "sindri.animation.sprite": {
              "clips": { "walk": { "frames": ["0", "1"], "seconds_per_frame": 0.1 } },
              "playing": "walk"
            }
          } }"#,
    ));
    assert!(
        sindri_scene::referenced_sheets(&world).contains("textures/walk.sheet.json"),
        "a sprite whose clips name parts of a sheet needs that sheet loaded, \
         even though its own reference names no part of one"
    );
}
