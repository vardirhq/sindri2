//! A script can author a compact world-space polygon without escaping into JSON.

use serde_json::json;
use sindri_core::{ComponentSchemaRegistry, EntityData, SceneComponent, Transform3D, World};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::ShapeComponent;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
        .register::<ShapeComponent>("Shape")
        .expect("sindri.shape registers");
    registry
}

fn run(source: &str) -> (World, sindri_decay::ScriptReport) {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Hull".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [
            (
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": "hull.decay", "script": "Hull" }),
            ),
            (
                ShapeComponent::TYPE_NAME.to_owned(),
                json!({ "kind": "polygon", "count": 6.0 }),
            ),
        ]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("hull.decay", source);
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    (world, report)
}

#[test]
fn a_script_writes_bounded_polygon_vertices_and_can_use_exp() {
    let (world, report) = run(
        r"
        script Hull {
            fn start() {
                World.set_shape_point(0.0, 0.25, -0.5);
                World.set_shape_point(5.0, -0.25, 0.5);
                this.transform.position.x = exp(0.0);
            }
        }
        ",
    );
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    let (_, hull) = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Hull"))
        .expect("the scripted hull remains");
    assert_eq!(
        hull.components[ShapeComponent::TYPE_NAME]["points"],
        json!([
            [0.25, -0.5],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [-0.25, 0.5]
        ])
    );
    assert_eq!(
        hull.transform_3d.expect("the hull has a transform").position[0],
        1.0
    );
}

#[test]
fn an_authored_polygon_point_outside_the_renderer_limit_is_refused() {
    let (_, report) = run(
        r"
        script Hull {
            fn start() { World.set_shape_point(8.0, 0.0, 0.0); }
        }
        ",
    );
    let message = report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<String>();
    assert!(message.contains("0 through 7"), "{message}");
}
