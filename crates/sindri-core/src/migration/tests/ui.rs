//! Format 8: the screen half of a scene becomes the `sindri.ui.*` family.

use serde_json::json;

use crate::{SCENE_FORMAT_VERSION, SceneMigrationError, SceneMigrator};

#[test]
fn a_screen_sprite_becomes_a_ui_image_and_keeps_what_it_drew_with() {
    let old = json!({
        "format_version": 7,
        "entities": [{
            "id": "pip",
            "components": {
                "sindri.sprite": {
                    "texture": "textures/pip.png#full",
                    "anchor": "top_left",
                    "tint": [1.0, 1.0, 1.0, 0.5],
                    "layer": 100
                }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(migrated["format_version"], json!(SCENE_FORMAT_VERSION));
    let components = &migrated["entities"][0]["components"];
    assert_eq!(
        components["sindri.ui.image"],
        json!({
            "texture": "textures/pip.png#full",
            "anchor": "top_left",
            "tint": [1.0, 1.0, 1.0, 0.5],
            "layer": 100
        }),
        "everything that decided how it drew comes across"
    );
    assert!(components.get("sindri.sprite").is_none());
}

/// Screen was the default, so a sprite that said nothing about space was on
/// the screen. Reading the absence as "world" would move every HUD element
/// ever authored into the scene, which is the one thing a migration must not
/// do quietly.
#[test]
fn a_sprite_that_named_no_space_was_on_the_screen() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "badge", "components": {
            "sindri.sprite": { "texture": "badge.png" }
        }}]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    let components = &migrated["entities"][0]["components"];
    assert_eq!(
        components["sindri.ui.image"],
        json!({ "texture": "badge.png" })
    );
}

/// A world sprite keeps its name and loses the two fields that said it was one
/// and anchored it to an edge it never had.
#[test]
fn a_world_sprite_loses_the_fields_that_decided_nothing() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "orb", "components": {
            "sindri.sprite": {
                "texture": "orb.png",
                "space": "world",
                "anchor": "top_left",
                "layer": 10
            }
        }}]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    let components = &migrated["entities"][0]["components"];
    assert_eq!(
        components["sindri.sprite"],
        json!({ "texture": "orb.png", "layer": 10 })
    );
    assert!(components.get("sindri.ui.image").is_none());
}

/// Text was always screen-space, so only the key moves.
#[test]
fn text_is_renamed_and_its_payload_is_carried_across() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "title", "components": {
            "sindri.text": {
                "text": "GATHER",
                "font": "fonts/Inter.ttf",
                "anchor": "top",
                "layer": 101
            }
        }}]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    let components = &migrated["entities"][0]["components"];
    assert_eq!(
        components["sindri.ui.text"],
        json!({
            "text": "GATHER",
            "font": "fonts/Inter.ttf",
            "anchor": "top",
            "layer": 101
        })
    );
    assert!(components.get("sindri.text").is_none());
}

#[test]
fn a_world_tilemap_simply_loses_the_field() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "floor", "components": {
            "sindri.tilemap": {
                "texture": "tiles.png",
                "palette": ["floor"],
                "columns": 1,
                "rows": 1,
                "tiles": [0],
                "space": "world"
            }
        }}]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    let map = &migrated["entities"][0]["components"]["sindri.tilemap"];
    assert!(map.get("space").is_none());
    assert_eq!(map["tiles"], json!([0]));
}

/// Format 8 has no screen-space tilemap, and nothing here can invent one, so
/// the author is told rather than shown a floor that has silently moved.
#[test]
fn a_screen_tilemap_stops_the_migration_and_says_what_to_do() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "minimap", "components": {
            "sindri.tilemap": {
                "texture": "tiles.png",
                "palette": ["floor"],
                "columns": 1,
                "rows": 1,
                "tiles": [0]
            }
        }}]
    });
    let error = SceneMigrator::builtin().migrate(old).unwrap_err();
    let SceneMigrationError::Unconvertible(message) = error else {
        panic!("a screen tilemap is unconvertible, not {error:?}");
    };
    assert!(message.contains("minimap"), "{message}");
    assert!(message.contains("sindri.ui.image"), "{message}");
}

/// Two payloads that both claim the same key are different authored data, and
/// no choice between them is reliably the same scene.
#[test]
fn a_scene_carrying_both_spellings_stops_rather_than_choosing() {
    let old = json!({
        "format_version": 7,
        "entities": [{ "id": "badge", "components": {
            "sindri.sprite": { "texture": "a.png" },
            "sindri.ui.image": { "texture": "b.png" }
        }}]
    });
    let error = SceneMigrator::builtin().migrate(old).unwrap_err();
    let SceneMigrationError::Unconvertible(message) = error else {
        panic!("both spellings are unconvertible, not {error:?}");
    };
    assert!(message.contains("badge"), "{message}");
}
