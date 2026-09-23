//! Format 10: the environment's sun becomes a light entity.

use serde_json::{Value, json};

use crate::{SCENE_FORMAT_VERSION, SceneDocument, SceneMigrator};

fn format_nine(directional: &Value) -> Value {
    json!({
        "format_version": 9,
        "entities": [{
            "id": "environment",
            "components": { "sindri.environment": {
                "ambient_intensity": 0.4,
                "directional": directional
            } }
        }]
    })
}

/// Local -Z of a quaternion `[x, y, z, w]`, the way the light shines.
fn forward(rotation: &Value) -> [f64; 3] {
    let quaternion: Vec<f64> = rotation
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap())
        .collect();
    let [x, y, z, w] = [quaternion[0], quaternion[1], quaternion[2], quaternion[3]];
    let (xz, yz, wx, wy) = (x * z, y * z, w * x, w * y);
    [
        -(2.0 * (xz + wy)),
        -(2.0 * (yz - wx)),
        -(1.0 - 2.0 * (x * x + y * y)),
    ]
}

#[test]
fn a_lit_sun_becomes_a_light_entity_aimed_the_same_way() {
    let migrated = SceneMigrator::builtin()
        .migrate(format_nine(&json!({
            "direction": [0.0, -2.0, 0.0],
            "color": [1.0, 0.5, 0.25],
            "intensity": 1.5
        })))
        .unwrap();
    assert_eq!(migrated["format_version"], json!(SCENE_FORMAT_VERSION));
    let environment = &migrated["entities"][0]["components"]["sindri.environment"];
    assert!(environment.get("directional").is_none());
    assert_eq!(environment["ambient_intensity"], json!(0.4));

    let sun = &migrated["entities"][1];
    assert_eq!(sun["id"], json!("sun"));
    assert_eq!(sun["name"], json!("Sun"));
    let light = &sun["components"]["sindri.light"];
    assert_eq!(light["kind"], json!("directional"));
    assert_eq!(light["color"], json!([1.0, 0.5, 0.25]));
    assert_eq!(light["intensity"], json!(1.5));
    let aimed = forward(&sun["transform_3d"]["rotation"]);
    assert!(
        (aimed[1] + 1.0).abs() < 1.0e-9 && aimed[0].abs() < 1.0e-9,
        "{aimed:?}"
    );
    SceneDocument::from_json(&migrated.to_string()).expect("the migrated scene loads");
}

#[test]
fn a_sun_shining_upwards_keeps_shining_upwards() {
    // The case that put shadows on hilltops. A migration is not the place to
    // correct it: the light is now drawn, so the mistake can be seen.
    let migrated = SceneMigrator::builtin()
        .migrate(format_nine(&json!({
            "direction": [-0.55, 1.01, -0.4],
            "intensity": 1.0
        })))
        .unwrap();
    let aimed = forward(&migrated["entities"][1]["transform_3d"]["rotation"]);
    let length = (0.55_f64 * 0.55 + 1.01 * 1.01 + 0.4 * 0.4).sqrt();
    for (got, want) in aimed
        .iter()
        .zip([-0.55 / length, 1.01 / length, -0.4 / length])
    {
        assert!((got - want).abs() < 1.0e-9, "{aimed:?}");
    }
}

#[test]
fn an_unlit_sun_leaves_no_entity_behind() {
    for directional in [json!({}), json!({ "intensity": 0.0 })] {
        let migrated = SceneMigrator::builtin()
            .migrate(format_nine(&directional))
            .unwrap();
        assert_eq!(migrated["entities"].as_array().unwrap().len(), 1);
        assert!(
            migrated["entities"][0]["components"]["sindri.environment"]
                .get("directional")
                .is_none()
        );
    }
}

#[test]
fn the_new_entity_does_not_take_an_id_already_in_use() {
    let mut old = format_nine(&json!({ "intensity": 1.0 }));
    old["entities"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "sun" }));
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(migrated["entities"][2]["id"], json!("sun-2"));
}
