//! Format 4: component names gain a namespace, and payloads do not move.

use serde_json::json;

use crate::{SCENE_FORMAT_VERSION, SceneMigrationError, SceneMigrator};

#[test]
fn format_four_component_names_migrate_without_touching_payloads() {
    let old = json!({
        "format_version": 4,
        "entities": [{
            "id": "player",
            "components": {
                "sindri.grid_navigation": { "walls": [[[0, 0], [1, 0]]] },
                "sindri.grid_occupant": { "grid": "floor", "footprint": [[0, 0]] },
                "sindri.sprite_animation": { "clips": { "idle": { "frames": ["idle"] } } },
                "sindri.audio": { "clip": "audio/pickup.wav", "volume": 0.75 }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(migrated["format_version"], json!(SCENE_FORMAT_VERSION));
    let components = &migrated["entities"][0]["components"];
    assert_eq!(
        components["sindri.grid.navigation"],
        json!({ "walls": [[[0, 0], [1, 0]]] })
    );
    assert_eq!(
        components["sindri.grid.occupant"],
        json!({ "grid": "floor", "footprint": [[0, 0]] })
    );
    assert_eq!(
        components["sindri.animation.sprite"],
        json!({ "clips": { "idle": { "frames": ["idle"] } } })
    );
    assert_eq!(
        components["sindri.audio.source"],
        json!({ "clip": "audio/pickup.wav", "volume": 0.75 })
    );
    for old in [
        "sindri.grid_navigation",
        "sindri.grid_occupant",
        "sindri.sprite_animation",
        "sindri.audio",
    ] {
        assert!(components.get(old).is_none(), "legacy key {old} survived");
    }
}

#[test]
fn namespace_migration_refuses_ambiguous_duplicate_spellings() {
    let error = SceneMigrator::builtin()
        .migrate(json!({
            "format_version": 4,
            "entities": [{
                "id": "player",
                "components": {
                    "sindri.audio": { "clip": "old.wav" },
                    "sindri.audio.source": { "clip": "new.wav" }
                }
            }]
        }))
        .unwrap_err();
    assert!(
        matches!(error, SceneMigrationError::Unconvertible(message) if message.contains("player") && message.contains("sindri.audio") && message.contains("sindri.audio.source"))
    );
}
