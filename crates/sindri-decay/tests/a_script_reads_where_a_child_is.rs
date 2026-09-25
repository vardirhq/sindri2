//! A child's transform is relative to its parent; `world_position` is where it
//! ends up.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn world(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    let parent = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [10.0, 0.0, 0.0],
            scale: [2.0, 2.0, 1.0],
            ..Transform3D::default()
        }),
        ..EntityData::default()
    });
    let child = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [1.0, 0.0, 0.0],
            ..Transform3D::default()
        }),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "reader.decay", "script": "Reader" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    world.set_parent(child, Some(parent)).expect("parents");
    let mut sources = ScriptSources::new();
    sources.insert("reader.decay", script);
    (world, child, sources)
}

fn run(world: &mut World, sources: &ScriptSources) {
    let report = Scripts::new().advance(
        world,
        &registry(),
        ScriptFrame::new(sources, &InputState::default(), 1.0 / 60.0),
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
}

#[test]
fn a_script_reads_where_a_child_really_is() {
    let (mut world, child, sources) = world(
        r"
        script Reader {
            fn update(dt: f32) {
                this.transform.position.z = this.transform.world_position.x;
            }
        }",
    );
    run(&mut world, &sources);
    let local = world
        .get(child)
        .and_then(|data| data.transform_3d)
        .expect("t");
    assert!(
        (local.position[0] - 1.0).abs() < 1e-5,
        "its own place is unchanged"
    );
    assert!(
        (local.position[2] - 12.0).abs() < 1e-5,
        "saw {}",
        local.position[2]
    );
}

#[test]
fn writing_where_a_child_really_is_stores_its_place_under_the_parent() {
    let (mut world, child, sources) = world(
        r"
        script Reader {
            fn update(dt: f32) {
                this.transform.world_position.x = 20.0;
            }
        }",
    );
    run(&mut world, &sources);
    let placed = world.world_transform(child).expect("t");
    assert!((placed.position[0] - 20.0).abs() < 1e-4);
    let local = world
        .get(child)
        .and_then(|data| data.transform_3d)
        .expect("t");
    assert!(
        (local.position[0] - 5.0).abs() < 1e-4,
        "ten units out, at half size"
    );
}
