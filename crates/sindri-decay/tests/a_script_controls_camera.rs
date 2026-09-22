use serde_json::json;
use sindri_core::{EntityData, SceneComponent, World};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::{CameraBehaviorComponent, SceneExtractor};

fn registry() -> sindri_core::ComponentSchemaRegistry {
    let mut registry = SceneExtractor::new()
        .expect("the builtin components register")
        .components()
        .clone();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

#[test]
fn a_script_adds_trauma_to_the_authored_camera_behavior() {
    let mut world = World::default();
    let camera = world.spawn(EntityData {
        components: [
            (
                "sindri.camera".to_owned(),
                json!({
                    "projection": "orthographic",
                    "vertical_size": 8.0,
                    "near": 0.1,
                    "far": 100.0
                }),
            ),
            (
                CameraBehaviorComponent::TYPE_NAME.to_owned(),
                json!({ "shake": { "trauma": 0.2 } }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    world.spawn(EntityData {
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "camera.decay", "script": "CameraHit" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "camera.decay",
        r"script CameraHit { fn start() { Camera.add_trauma(0.5); } }",
    );

    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.is_quiet(), "{report:?}");
    let trauma = world.get(camera).unwrap().components[CameraBehaviorComponent::TYPE_NAME]
        ["shake"]["trauma"]
        .as_f64()
        .unwrap();
    assert!((trauma - 0.7).abs() < f64::EPSILON);
}

#[test]
fn camera_trauma_without_one_behavior_camera_is_a_runtime_error() {
    let mut world = World::default();
    world.spawn(EntityData {
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "camera.decay", "script": "CameraHit" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "camera.decay",
        r"script CameraHit { fn start() { Camera.add_trauma(0.5); } }",
    );

    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    assert_eq!(report.failures.len(), 1, "{report:?}");
    assert!(
        format!("{:?}", report.failures).contains("exactly one authored camera"),
        "{report:?}"
    );
}
