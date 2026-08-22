//! Logical grid coordinates at the Decay/world boundary.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFailure, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn scripted_world(script: &str, tilemap: serde_json::Value) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Floor".to_owned()),
        transform_3d: Some(Transform3D {
            position: [10.0, 20.0, -4.0],
            rotation: [
                0.0,
                0.0,
                std::f32::consts::FRAC_PI_4.sin(),
                std::f32::consts::FRAC_PI_4.cos(),
            ],
            scale: [2.0, 3.0, 1.0],
            ..Transform3D::default()
        }),
        components: [("sindri.tilemap".to_owned(), tilemap)]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let actor = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [0.0, 0.0, 7.0],
            ..Transform3D::default()
        }),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "grid.decay", "script": "Mover" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("grid.decay", script);
    (world, actor, sources)
}

fn run(world: &mut World, sources: &ScriptSources) -> Vec<ScriptFailure> {
    Scripts::new()
        .advance(
            world,
            &registry(),
            sources,
            &InputState::default(),
            1.0 / 60.0,
        )
        .failures
}

#[test]
fn isometric_grid_position_uses_the_maps_full_transform_and_round_trips() {
    let (mut world, actor, sources) = scripted_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 2.0, 1.0);
                this.transform.scale.x = Grid.position_x(this.entity, floor);
                this.transform.scale.y = Grid.position_y(this.entity, floor);
            }
        }
        "#,
        json!({
            "columns": 4,
            "rows": 4,
            "projection": "isometric",
            "space": "world",
            "tile_size": [2.0, 1.0],
            "texture": "tiles.png",
            "palette": ["tile"],
            "tiles": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        }),
    );

    assert!(run(&mut world, &sources).is_empty());
    let transform = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform");
    assert!(
        (transform.position[0] - 14.5).abs() < 1.0e-5
            && (transform.position[1] - 22.0).abs() < 1.0e-5,
        "the local diamond point should be scaled, rotated, and translated: {transform:?}"
    );
    assert_eq!(
        transform.position[2].to_bits(),
        7.0_f32.to_bits(),
        "placing preserves the layer"
    );
    assert!(
        (transform.scale[0] - 2.0).abs() < 1.0e-5 && (transform.scale[1] - 1.0).abs() < 1.0e-5,
        "reading the placed point should recover its logical coordinate"
    );
}

#[test]
fn orthogonal_grid_position_uses_the_tilemap_center_convention() {
    let (mut world, actor, sources) = scripted_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 1.0, 2.0);
            }
        }
        "#,
        json!({
            "columns": 2,
            "rows": 3,
            "space": "world",
            "tile_size": [2.0, 4.0],
            "texture": "tiles.png",
            "palette": ["tile"],
            "tiles": [0, 0, 0, 0, 0, 0]
        }),
    );

    assert!(run(&mut world, &sources).is_empty());
    let position = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .position;
    // Local (3, -10), map-scaled to (6, -30), quarter-turned to
    // (30, 6), and translated by (10, 20).
    assert!(
        (position[0] - 40.0).abs() < 1.0e-5 && (position[1] - 26.0).abs() < 1.0e-5,
        "orthogonal placement should use the same half-cell origin as tilemaps: {position:?}"
    );
}

#[test]
fn the_grid_argument_is_statically_an_entity() {
    let (mut world, _actor, sources) = scripted_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                Grid.place(this.entity, 1.0, 2.0, 1.0);
            }
        }
        "#,
        json!({}),
    );
    let failures = run(&mut world, &sources);
    assert!(
        failures
            .iter()
            .any(|failure| failure.to_string().contains("Entity")),
        "a number must not compile where a grid entity is required: {failures:?}"
    );
}
