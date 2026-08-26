//! Formats 5 and 6: the camera's orientation, and the overlay camera's exit.

use serde_json::json;

use crate::{SCENE_FORMAT_VERSION, SceneMigrator};

use super::super::step::camera::CAMERA_COMPONENT;
use super::super::step::vector::migration_cross;

#[test]
fn format_five_camera_look_at_becomes_transform_rotation() {
    let old = json!({
        "format_version": 5,
        "entities": [{
            "id": "camera",
            "transform_3d": {
                "position": [3.0, 2.0, 4.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [2.0, 2.0, 2.0]
            },
            "components": {
                "sindri.camera": {
                    "projection": "perspective",
                    "target": [0.0, 0.0, 0.0],
                    "up": [0.0, 1.0, 0.0],
                    "vertical_fov_degrees": 60.0,
                    "near": 0.1,
                    "far": 100.0
                }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(migrated["format_version"], json!(SCENE_FORMAT_VERSION));
    let camera = &migrated["entities"][0]["components"]["sindri.camera"];
    assert!(camera.get("target").is_none());
    assert!(camera.get("up").is_none());
    assert_eq!(
        migrated["entities"][0]["transform_3d"]["scale"],
        json!([2.0, 2.0, 2.0])
    );

    let rotation = migrated["entities"][0]["transform_3d"]["rotation"]
        .as_array()
        .unwrap();
    let quaternion = [
        rotation[0].as_f64().unwrap(),
        rotation[1].as_f64().unwrap(),
        rotation[2].as_f64().unwrap(),
        rotation[3].as_f64().unwrap(),
    ];
    let rotate = |vector: [f64; 3]| {
        let [axis_x, axis_y, axis_z, scalar] = quaternion;
        let axis = [axis_x, axis_y, axis_z];
        let axis_cross_vector = migration_cross(axis, vector);
        let axis_cross_twice = migration_cross(axis, axis_cross_vector);
        [
            vector[0] + 2.0 * (scalar * axis_cross_vector[0] + axis_cross_twice[0]),
            vector[1] + 2.0 * (scalar * axis_cross_vector[1] + axis_cross_twice[1]),
            vector[2] + 2.0 * (scalar * axis_cross_vector[2] + axis_cross_twice[2]),
        ]
    };
    let length = 29.0_f64.sqrt();
    let forward = [-3.0 / length, -2.0 / length, -4.0 / length];
    let actual = rotate([0.0, 0.0, -1.0]);
    let error = [
        actual[0] - forward[0],
        actual[1] - forward[1],
        actual[2] - forward[2],
    ];
    assert!(error[0] * error[0] + error[1] * error[1] + error[2] * error[2] < 1.0e-12);
}

#[test]
fn format_five_overlay_camera_is_removed_without_reorienting_its_entity() {
    let old = json!({
        "format_version": 5,
        "entities": [{
            "id": "overlay",
            "transform_3d": { "rotation": [0.1, 0.2, 0.3, 0.9] },
            "components": {
                "sindri.camera": {
                    "projection": "orthographic",
                    "center": [0.0, 0.0],
                    "vertical_size": 2.0,
                    "near": -10.0,
                    "far": 10.0
                }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(
        migrated["entities"][0]["transform_3d"]["rotation"],
        json!([0.1, 0.2, 0.3, 0.9])
    );
    assert!(
        migrated["entities"][0]["components"]
            .get(CAMERA_COMPONENT)
            .is_none()
    );
}

#[test]
fn format_six_overlay_camera_component_is_removed_but_entity_survives() {
    let old = json!({
        "format_version": 6,
        "entities": [{
            "id": "overlay",
            "name": "Overlay Camera",
            "components": {
                "sindri.camera": {
                    "projection": "orthographic",
                    "center": [0.0, 0.0],
                    "vertical_size": 2.0,
                    "near": 0.0,
                    "far": 10.0
                },
                "game.keep": { "value": 1 }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(migrated["format_version"], json!(SCENE_FORMAT_VERSION));
    assert_eq!(migrated["entities"][0]["name"], json!("Overlay Camera"));
    assert!(
        migrated["entities"][0]["components"]
            .get(CAMERA_COMPONENT)
            .is_none()
    );
    assert_eq!(
        migrated["entities"][0]["components"]["game.keep"],
        json!({ "value": 1 })
    );
}

#[test]
fn format_six_perspective_camera_survives() {
    let old = json!({
        "format_version": 6,
        "entities": [{
            "id": "camera",
            "components": {
                "sindri.camera": {
                    "projection": "perspective",
                    "vertical_fov_degrees": 60.0,
                    "near": 0.1,
                    "far": 100.0
                }
            }
        }]
    });
    let migrated = SceneMigrator::builtin().migrate(old).unwrap();
    assert_eq!(
        migrated["entities"][0]["components"][CAMERA_COMPONENT]["projection"],
        json!("perspective")
    );
}
