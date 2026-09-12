//! A script can read and write the cells of a tilemap.
//!
//! The ground is gameplay state in anything that farms, builds, burns or floods
//! it. Before these calls a script could only put entities on top of the floor,
//! so the floor and the game's idea of the floor were two different things that
//! had to be kept in agreement by hand.

use serde_json::json;
use sindri_core::{ComponentSchemaRegistry, EntityData, SceneComponent, Transform3D, World};
use sindri_decay::{ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::TilemapComponent;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
        .register::<TilemapComponent>("Tilemap")
        .expect("sindri.tilemap registers");
    registry
}

/// A two-by-two field: grass, grass, grass, and one cell holding nothing.
fn field() -> serde_json::Value {
    json!({
        "texture": "textures/ground.png",
        "palette": ["grass", "tilled", "watered"],
        "columns": 2,
        "rows": 2,
        "tiles": [0, 0, 0, null],
    })
}

fn run(source: &str) -> (World, sindri_decay::ScriptReport) {
    let mut world = World::default();
    let map = world.spawn(EntityData {
        name: Some("Floor".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(TilemapComponent::TYPE_NAME.to_owned(), field())]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    let _ = map;
    world.spawn(EntityData {
        name: Some("Farmer".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "farmer.decay", "script": "Farmer" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("farmer.decay", source);
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    (world, report)
}

/// The cells of the map after a frame, as the payload actually holds them.
fn cells(world: &World) -> Vec<Option<u64>> {
    world
        .entities()
        .find_map(|(_, data)| data.components.get(TilemapComponent::TYPE_NAME))
        .expect("the map is still there")
        .get("tiles")
        .and_then(serde_json::Value::as_array)
        .expect("tiles")
        .iter()
        .map(serde_json::Value::as_u64)
        .collect()
}

fn ok(report: &sindri_decay::ScriptReport) {
    assert!(report.failures.is_empty(), "{report:#?}");
}

#[test]
fn a_script_reads_a_cell_and_writes_a_different_one() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.tile(floor, 0.0, 0.0) == 0.0 {
                    Grid.set_tile(floor, 0.0, 0.0, 1.0);
                }
            }
        }
        "#);
    ok(&report);
    assert_eq!(cells(&world), vec![Some(1), Some(0), Some(0), None]);
}

/// Row-major, and the same cell the rest of the grid calls mean. A map whose
/// rows and columns were swapped would still pass a test that only ever wrote
/// to the origin.
#[test]
fn a_column_and_a_row_are_not_the_same_axis() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                Grid.set_tile(floor, 1.0, 0.0, 2.0);
            }
        }
        "#);
    ok(&report);
    assert_eq!(
        cells(&world),
        vec![Some(0), Some(2), Some(0), None],
        "column one of row zero is the second cell, not the third"
    );
}

/// Off the map reads as empty rather than failing, because anything that moves
/// will ask about the edge and a script should not have to guard every call.
#[test]
fn asking_about_a_cell_the_map_does_not_have_is_not_an_error() {
    let (_, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                print(Grid.tile(floor, 9.0, 9.0));
                print(Grid.tile(floor, 0.0 - 1.0, 0.0));
            }
        }
        "#);
    ok(&report);
}

/// Writing there is a different matter: it is a mistake with no sensible
/// meaning, and silently dropping it would hide the bug.
#[test]
fn writing_off_the_map_says_so() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                Grid.set_tile(World.find("Floor"), 5.0, 0.0, 1.0);
            }
        }
        "#);
    assert!(
        !report.failures.is_empty(),
        "a write off the map should be reported"
    );
    assert_eq!(cells(&world), vec![Some(0), Some(0), Some(0), None]);
}

/// A palette index the map cannot answer would fail validation on the next
/// load, long after and nowhere near the script that wrote it.
#[test]
fn a_palette_index_the_map_does_not_have_is_refused_at_the_write() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                Grid.set_tile(World.find("Floor"), 0.0, 0.0, 7.0);
            }
        }
        "#);
    assert!(
        !report.failures.is_empty(),
        "an index past the palette should be reported"
    );
    assert_eq!(cells(&world), vec![Some(0), Some(0), Some(0), None]);
}

#[test]
fn a_negative_index_empties_the_cell() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                Grid.set_tile(World.find("Floor"), 0.0, 0.0, 0.0 - 1.0);
            }
        }
        "#);
    ok(&report);
    assert_eq!(cells(&world), vec![None, Some(0), Some(0), None]);
}

#[test]
fn an_empty_cell_reads_back_as_empty_rather_than_as_a_tile() {
    let (_, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                if Grid.tile(floor, 1.0, 1.0) == 0.0 - 1.0 {
                    Grid.set_tile(floor, 1.0, 1.0, 0.0);
                }
            }
        }
        "#);
    ok(&report);
}

#[test]
fn a_map_says_how_big_it_is() {
    let (world, report) = run(r#"
        script Farmer {
            fn update(dt: f32) {
                let floor = World.find("Floor");
                var column: f32 = 0.0;
                while column < Grid.columns(floor) {
                    var row: f32 = 0.0;
                    while row < Grid.rows(floor) {
                        Grid.set_tile(floor, column, row, 2.0);
                        row += 1.0;
                    }
                    column += 1.0;
                }
            }
        }
        "#);
    ok(&report);
    assert_eq!(
        cells(&world),
        vec![Some(2), Some(2), Some(2), Some(2)],
        "walking the map's own size should reach every cell, including the empty one"
    );
}

/// The payload is edited in place, so a field this build does not model is
/// still there afterwards. Rebuilding the typed view would drop it.
#[test]
fn writing_a_cell_keeps_the_rest_of_the_payload() {
    let mut world = World::default();
    let mut map = field();
    map.as_object_mut()
        .expect("an object")
        .insert("something_a_later_build_added".to_owned(), json!("kept"));
    world.spawn(EntityData {
        name: Some("Floor".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(TilemapComponent::TYPE_NAME.to_owned(), map)]
            .into_iter()
            .collect(),
        ..EntityData::default()
    });
    world.spawn(EntityData {
        name: Some("Farmer".to_owned()),
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "farmer.decay", "script": "Farmer" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert(
        "farmer.decay",
        "script Farmer { fn update(dt: f32) { Grid.set_tile(World.find(\"Floor\"), 0.0, 0.0, 1.0); } }",
    );
    let report = Scripts::new().advance(
        &mut world,
        &registry(),
        ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
    );
    ok(&report);
    let payload = world
        .entities()
        .find_map(|(_, data)| data.components.get(TilemapComponent::TYPE_NAME))
        .expect("the map");
    assert_eq!(
        payload.get("something_a_later_build_added"),
        Some(&json!("kept")),
        "a field the typed view does not know about survived the write"
    );
}
