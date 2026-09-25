//! Grid properties reach the grid they describe and its items' boxes.

use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

fn styled(sheet: &str) -> World {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 10,
            "metadata": { "name": "weave-grid" },
            "entities": [
                { "id": "inventory", "name": "inventory",
                  "transform_3d": { "scale": [2.0, 1.0, 1.0] },
                  "components": {
                      "sindri.ui.shape": { "kind": "rect" },
                      "sindri.ui.layout": { "direction": "row" } } },
                { "id": "slot", "name": "slot", "parent": "inventory",
                  "components": { "sindri.ui.shape": { "kind": "rect" } } }
            ]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    PresentationWorld::resolve(
        &source,
        &parse(sheet).expect("Weave parses"),
        Viewport {
            width: 800.0,
            height: 800.0,
        },
    )
    .expect("styles resolve")
    .world()
    .clone()
}

fn entity<'a>(world: &'a World, name: &str) -> &'a sindri_core::EntityData {
    world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some(name))
        .map(|(_, data)| data)
        .expect("the entity is there")
}

#[test]
fn display_grid_makes_a_grid_that_takes_css_tracks_gaps_and_alignment() {
    let world = styled(
        r"
            #inventory {
                display: grid;
                grid-template-columns: repeat(2, 1fr) 100px;
                grid-template-rows: auto 50%;
                gap: 20px 40px;
                justify-items: center;
                align-items: end;
                height: auto;
            }
            #slot { grid-column: 2 / span 2; grid-row: 1; }
        ",
    );
    let inventory = entity(&world, "inventory");
    // The flex line it was is gone: an element is one kind of container.
    assert!(!inventory.components.contains_key("sindri.ui.layout"));
    let grid = &inventory.components["sindri.ui.grid"];
    // 800 pixels high is two units: 100px is a quarter, 20px a twentieth,
    // and 50% is half the grid's one-unit height.
    assert_eq!(grid["columns"], serde_json::json!(["1fr", "1fr", "0.25"]));
    assert_eq!(grid["rows"], serde_json::json!(["auto", "0.5"]));
    let gap = grid["gap"].as_array().expect("a gap");
    assert!((gap[0].as_f64().expect("column gap") - 0.1).abs() < 1.0e-6);
    assert!((gap[1].as_f64().expect("row gap") - 0.05).abs() < 1.0e-6);
    assert_eq!(grid["justify_items"], "center");
    assert_eq!(grid["align_items"], "end");
    assert_eq!(grid["fit_content"], serde_json::json!([false, true]));

    let slot = &entity(&world, "slot").components["sindri.ui.box"];
    assert_eq!(slot["grid_column"], serde_json::json!([2, 2]));
    assert_eq!(slot["grid_row"], serde_json::json!([1, 1]));
}

#[test]
fn display_flex_turns_a_grid_back_into_a_line() {
    let world = styled(
        "#inventory { display: grid; } @media (min-width: 1px) { #inventory { display: flex; } }",
    );
    let inventory = entity(&world, "inventory");
    assert!(inventory.components.contains_key("sindri.ui.layout"));
    assert!(!inventory.components.contains_key("sindri.ui.grid"));
}
