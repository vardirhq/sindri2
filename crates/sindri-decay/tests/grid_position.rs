//! Logical grid coordinates at the Decay/world boundary.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFailure, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

fn scripted_world(script: &str, tilemap: serde_json::Value) -> (World, EntityId, ScriptSources) {
    scripted_grid(script, "sindri.tilemap", tilemap)
}

/// The same world, with the floor's geometry carried by a named component.
///
/// `sindri.tilemap` and `sindri.tile_grid` both describe where a cell is, and
/// the point of most of these tests is that a script cannot tell which one it
/// is standing on.
fn scripted_grid(
    script: &str,
    component: &str,
    payload: serde_json::Value,
) -> (World, EntityId, ScriptSources) {
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
        components: [(component.to_owned(), payload)].into_iter().collect(),
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
            ScriptFrame::new(sources, &InputState::default(), 1.0 / 60.0),
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

    assert_eq!(
        run(&mut world, &sources)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        Vec::<String>::new()
    );
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
        r"
        script Mover {
            fn update(dt: f32) {
                Grid.place(this.entity, 1.0, 2.0, 1.0);
            }
        }
        ",
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

#[test]
fn a_tile_grid_places_an_entity_exactly_where_a_tilemap_does() {
    // The whole point of the seam: a floor that has moved onto a tile volume
    // answers `Grid.place` with the same world position the flat map gave, so a
    // script that never mentioned either component keeps working.
    let script = r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 2.0, 1.0);
            }
        }
        "#;
    let placed = |component: &str, payload: serde_json::Value| {
        let (mut world, actor, sources) = scripted_grid(script, component, payload);
        assert!(run(&mut world, &sources).is_empty());
        world
            .get(actor)
            .and_then(|data| data.transform_3d)
            .expect("the actor kept its transform")
            .position
    };

    let flat = placed(
        "sindri.tilemap",
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
    let stacked = placed(
        "sindri.tile_grid",
        json!({
            "columns": 4,
            "rows": 4,
            "projection": "isometric",
            "cell_size": [2.0, 1.0],
            "level_step": [0.0, 0.5]
        }),
    );
    assert_eq!(
        flat.map(f32::to_bits),
        stacked.map(f32::to_bits),
        "a cell must not move when the component describing it changes"
    );
}

#[test]
fn a_tile_grid_answers_how_big_the_map_is() {
    let (mut world, actor, sources) = scripted_grid(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                this.transform.scale.x = Grid.columns(floor);
                this.transform.scale.y = Grid.rows(floor);
            }
        }
        "#,
        "sindri.tile_grid",
        json!({ "columns": 7, "rows": 5, "cell_size": [1.0, 1.0] }),
    );
    assert!(run(&mut world, &sources).is_empty());
    let scale = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .scale;
    assert!((scale[0] - 7.0).abs() < 1.0e-5 && (scale[1] - 5.0).abs() < 1.0e-5);
}

#[test]
fn reading_a_flat_cell_on_a_tile_grid_says_which_component_is_missing() {
    // A volume's cells are stacked and named, not indices into a palette, so
    // `Grid.tile` genuinely does not apply. The failure has to say that rather
    // than report that a tilemap the entity never had has no tiles.
    let (mut world, _actor, sources) = scripted_grid(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                this.transform.scale.x = Grid.tile(floor, 0.0, 0.0);
            }
        }
        "#,
        "sindri.tile_grid",
        json!({ "columns": 4, "rows": 4, "cell_size": [1.0, 1.0] }),
    );
    let failures = run(&mut world, &sources);
    let reported = failures
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        reported.contains("sindri.tilemap") && reported.contains("sindri.tile_grid"),
        "the failure should name both the component needed and the one present: {reported}"
    );
}

/// A floor whose cells are boxes, standing at the origin so that a cell's
/// world position reads off the grid coordinate directly.
fn solid_world(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    world.spawn(EntityData {
        name: Some("Floor".to_owned()),
        transform_3d: Some(Transform3D {
            position: [10.0, 0.0, -4.0],
            ..Transform3D::default()
        }),
        components: [(
            "sindri.tile_grid".to_owned(),
            json!({
                "columns": 8, "rows": 8, "cell_size": [1.1, 1.1],
                "cell_height": 1.1, "projection": "isometric", "space": "solid"
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let actor = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [0.0, 5.0, 0.0],
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

#[test]
fn a_solid_grid_places_across_and_into_the_scene_rather_than_in_a_picture() {
    // The whole of the migration to real coordinates, in one assertion. On a
    // projected grid a cell becomes a point on the plane the map is drawn on,
    // and a script's Y is a screen direction. Here a column is a distance
    // across the world and a row a distance into it, so the coordinate the
    // script names lands on X and Z -- and height, which the ground owns
    // rather than the script, is left exactly as it was found.
    let (mut world, actor, sources) = solid_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 2.0, 3.0);
            }
        }
        "#,
    );
    assert!(run(&mut world, &sources).is_empty());
    let position = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .position;
    assert!(
        (position[0] - (10.0 + 2.2)).abs() < 1.0e-5 && (position[2] - (-4.0 + 3.3)).abs() < 1.0e-5,
        "a cell on a solid grid is a place in the world: {position:?}"
    );
    assert_eq!(
        position[1].to_bits(),
        5.0_f32.to_bits(),
        "placing on a solid grid leaves height to whatever holds the walker up"
    );
}

#[test]
fn a_stretched_solid_grid_stretches_across_and_into_the_scene() {
    // A round trip on its own proves nothing here: a projected grid inverts
    // its own projection just as faithfully, so a test that only placed and
    // read back would pass whichever plane the host chose. What separates
    // them is which axes the floor's scale reaches. A solid grid is stretched
    // across the world and into it -- X and Z -- and a walker put on a cell of
    // a stretched floor has to land on the stretched place.
    let (mut world, actor, sources) = solid_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 5.0, 1.0);
                this.transform.scale.x = Grid.position_x(this.entity, floor);
                this.transform.scale.y = Grid.position_y(this.entity, floor);
            }
        }
        "#,
    );
    let floor = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Floor"))
        .map(|(entity, _)| entity)
        .expect("the floor is there");
    let mut stretched = world.get(floor).and_then(|data| data.transform_3d).unwrap();
    stretched.scale = [2.0, 1.0, 3.0];
    world.get_mut(floor).unwrap().transform_3d = Some(stretched);

    assert!(run(&mut world, &sources).is_empty());
    let transform = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform");
    assert!(
        (transform.position[0] - (10.0 + 5.0 * 1.1 * 2.0)).abs() < 1.0e-5
            && (transform.position[2] - (-4.0 + 1.0 * 1.1 * 3.0)).abs() < 1.0e-5,
        "a stretched solid floor stretches along X and Z: {:?}",
        transform.position
    );
    assert!(
        (transform.scale[0] - 5.0).abs() < 1.0e-5 && (transform.scale[1] - 1.0).abs() < 1.0e-5,
        "and reading it back undoes exactly that stretch: {transform:?}"
    );
}

#[test]
fn a_turned_solid_grid_is_refused_rather_than_silently_mis_placed() {
    let (mut world, _actor, sources) = solid_world(
        r#"
        script Mover {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.place(this.entity, floor, 1.0, 1.0);
            }
        }
        "#,
    );
    let floor = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Floor"))
        .map(|(entity, _)| entity)
        .expect("the floor is there");
    let mut transform = world.get(floor).and_then(|data| data.transform_3d).unwrap();
    transform.rotation = [0.0, 0.0, 0.383, 0.924];
    world.get_mut(floor).unwrap().transform_3d = Some(transform);

    let failures = run(&mut world, &sources);
    assert!(
        failures
            .iter()
            .any(|failure| failure.to_string().contains("turned")),
        "a turned solid grid should say so: {failures:?}"
    );
}
