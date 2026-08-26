//! The canonical form, and the fixed point it is.

use std::collections::BTreeMap;

use serde_json::json;

use crate::{SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneMetadata, Transform3D};

use super::support::entity;

#[test]
fn round_trips_scene_json() {
    let scene = SceneDocument {
        metadata: SceneMetadata {
            name: Some("Test".into()),
            editor: BTreeMap::new(),
        },
        entities: vec![entity("player", None)],
        ..SceneDocument::default()
    };
    let json = serde_json::to_string(&scene).unwrap();
    let decoded: SceneDocument = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, scene);
    decoded.validate().unwrap();
}

#[test]
fn canonical_json_sorts_entities_and_is_a_fixed_point() {
    let scene = SceneDocument {
        entities: vec![entity("zeta", None), entity("alpha", None)],
        ..SceneDocument::default()
    };
    let json = scene.to_canonical_json().unwrap();
    assert!(json.ends_with('\n'));

    let decoded = SceneDocument::from_json(&json).unwrap();
    assert_eq!(
        decoded
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
    assert!(decoded.is_canonical());
    assert_eq!(decoded.to_canonical_json().unwrap(), json);
}

#[test]
fn canonical_json_omits_empty_sections() {
    let scene = SceneDocument {
        entities: vec![entity("solo", None)],
        ..SceneDocument::default()
    };
    let json = scene.to_canonical_json().unwrap();
    assert!(!json.contains("null"));
    assert!(!json.contains("components"));
    assert!(!json.contains("editor"));
    assert!(json.contains(&format!("\"format_version\": {SCENE_FORMAT_VERSION}")));
}

#[test]
fn editor_metadata_round_trips_and_can_be_stripped() {
    let mut scene = SceneDocument {
        metadata: SceneMetadata {
            name: Some("Authored".into()),
            editor: BTreeMap::from([("camera_bookmark".to_owned(), json!([1.0, 2.0]))]),
        },
        entities: vec![SceneEntity {
            editor: BTreeMap::from([("collapsed".to_owned(), json!(true))]),
            ..entity("root", None)
        }],
        ..SceneDocument::default()
    };

    let json = scene.to_canonical_json().unwrap();
    assert_eq!(SceneDocument::from_json(&json).unwrap(), scene);

    scene.strip_editor_metadata();
    let stripped = scene.to_canonical_json().unwrap();
    assert!(!stripped.contains("collapsed"));
    assert!(!stripped.contains("camera_bookmark"));
    assert!(stripped.contains("Authored"));

    // Runtimes ignore editor state, so stripping must not change the scene.
    let with_editor = SceneDocument::from_json(&json).unwrap();
    let without_editor = SceneDocument::from_json(&stripped).unwrap();
    assert_eq!(with_editor.entities.len(), without_editor.entities.len());
    assert_eq!(
        with_editor.entities[0].transform_3d,
        without_editor.entities[0].transform_3d
    );
}

#[test]
fn scalar_arrays_are_inlined_but_structured_arrays_are_not() {
    let scene = SceneDocument {
        entities: vec![SceneEntity {
            transform_3d: Some(Transform3D {
                position: [3.0, 2.0, 4.0],
                ..Transform3D::default()
            }),
            components: BTreeMap::from([(
                "game.path".to_owned(),
                json!({ "waypoints": [[0, 1], [2, 3]], "tags": ["a", "b"] }),
            )]),
            ..entity("mover", None)
        }],
        ..SceneDocument::default()
    };

    let json = scene.to_canonical_json().unwrap();
    assert!(json.contains("\"position\": [3.0, 2.0, 4.0]"));
    assert!(json.contains("\"tags\": [\"a\", \"b\"]"));
    // An array of arrays keeps one element per line.
    assert!(json.contains("\"waypoints\": [\n"));
    assert_eq!(SceneDocument::from_json(&json).unwrap(), scene);
    assert_eq!(
        SceneDocument::from_json(&json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        json
    );
}

#[test]
fn long_scalar_arrays_stay_expanded() {
    let scene = SceneDocument {
        entities: vec![SceneEntity {
            components: BTreeMap::from([(
                "game.tilemap".to_owned(),
                json!({ "tiles": (0..64).collect::<Vec<u32>>() }),
            )]),
            ..entity("map", None)
        }],
        ..SceneDocument::default()
    };

    let json = scene.to_canonical_json().unwrap();
    assert!(json.contains("\"tiles\": [\n"));
    assert_eq!(
        SceneDocument::from_json(&json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        json
    );
}

#[test]
fn brackets_inside_strings_do_not_confuse_the_formatter() {
    let scene = SceneDocument {
        entities: vec![SceneEntity {
            name: Some("a [ b { c \" d ] e".into()),
            ..entity("tricky", None)
        }],
        ..SceneDocument::default()
    };

    let json = scene.to_canonical_json().unwrap();
    let decoded = SceneDocument::from_json(&json).unwrap();
    assert_eq!(decoded, scene);
    assert_eq!(decoded.to_canonical_json().unwrap(), json);
}
