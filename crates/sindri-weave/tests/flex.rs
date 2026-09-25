//! Flexbox properties reach the layout and the box they describe.

use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

fn styled(sheet: &str) -> World {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 10,
            "metadata": { "name": "weave-flex" },
            "entities": [
                {
                    "id": "bar", "name": "bar",
                    "transform_3d": { "scale": [2.0, 0.5, 1.0] },
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" },
                        "sindri.ui.layout": { "direction": "column" }
                    }
                },
                {
                    "id": "item", "name": "item", "parent": "bar",
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" }
                    }
                }
            ]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    PresentationWorld::resolve(
        &source,
        &parse(sheet).expect("Weave parses"),
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect("styles resolve")
    .world()
    .clone()
}

fn component(world: &World, name: &str, type_name: &str) -> serde_json::Value {
    world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some(name))
        .and_then(|(_, data)| data.components.get(type_name).cloned())
        .unwrap_or_else(|| panic!("{name} has {type_name}"))
}

#[test]
fn a_container_and_its_items_say_what_css_says() {
    let world = styled(
        r"
            #bar {
                flex-direction: row;
                flex-wrap: wrap;
                justify-content: space-evenly;
                align-items: stretch;
                height: auto;
            }
            #item {
                flex: 2 0 50%;
                order: -1;
                align-self: flex-end;
                max-width: 400px;
            }
        ",
    );
    let layout = component(&world, "bar", "sindri.ui.layout");
    assert_eq!(layout["direction"], "row");
    assert_eq!(layout["wrap"], true);
    assert_eq!(layout["justify"], "space_evenly");
    assert_eq!(layout["align"], "stretch");
    assert_eq!(layout["fit_content"], serde_json::json!([false, true]));

    let item = component(&world, "item", "sindri.ui.box");
    assert_eq!(item["grow"], 2.0);
    assert_eq!(item["shrink"], 0.0);
    // Half the row's two units of width.
    assert!((item["basis"].as_f64().expect("a basis") - 1.0).abs() < 1.0e-6);
    assert_eq!(item["order"], -1);
    assert_eq!(item["align_self"], "end");
    // 800 pixels high is two units, so 400px is one.
    assert!((item["max_size"][0].as_f64().expect("a maximum") - 1.0).abs() < 1.0e-6);
}

#[test]
fn flex_auto_and_none_keep_their_css_meanings() {
    let world = styled("#item { flex: none; }");
    let item = component(&world, "item", "sindri.ui.box");
    assert_eq!(item["grow"], 0.0);
    assert_eq!(item["shrink"], 0.0);
    assert_eq!(item["basis"], -1.0);
}

#[test]
fn nonsense_is_refused_with_the_property_named() {
    for sheet in [
        "#bar { flex-wrap: sometimes; }",
        "#item { flex-grow: -1; }",
        "#item { align-self: middle; }",
    ] {
        let document = SceneDocument::from_json(
            r#"{ "format_version": 10, "metadata": { "name": "t" },
                 "entities": [{ "id": "bar", "components": {
                     "sindri.ui.layout": {} } },
                   { "id": "item", "parent": "bar", "components": {
                     "sindri.ui.shape": { "kind": "rect" } } }] }"#,
        )
        .expect("scene parses");
        let source = World::from_scene(&document).expect("scene loads").world;
        let error = PresentationWorld::resolve(
            &source,
            &parse(sheet).expect("Weave parses"),
            Viewport {
                width: 800.0,
                height: 800.0,
            },
        )
        .expect_err(sheet);
        assert!(error.to_string().contains("invalid"), "{error}");
    }
}

#[test]
fn auto_on_a_text_element_fits_its_words() {
    let document = SceneDocument::from_json(
        r#"{ "format_version": 10, "metadata": { "name": "t" },
             "entities": [{ "id": "label", "name": "label", "components": {
                 "sindri.ui.text": { "text": "Hi", "font": "f.ttf", "font_size": 0.05 } } }] }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let world = PresentationWorld::resolve(
        &source,
        &parse("#label { width: auto; height: 20px; }").expect("Weave parses"),
        Viewport {
            width: 800.0,
            height: 800.0,
        },
    )
    .expect("styles resolve")
    .world()
    .clone();
    let item = component(&world, "label", "sindri.ui.box");
    assert_eq!(item["fit_content"], serde_json::json!([true, false]));
}
