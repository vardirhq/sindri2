use sindri_core::{SceneDocument, World};
use weave::{Viewport, parse};

use super::{ApplyError, PresentationWorld, srgb_channel};

fn assert_color(actual: &serde_json::Value, expected: [f64; 4]) {
    let channels = actual.as_array().expect("color is an array");
    for (actual, expected) in channels.iter().zip(expected) {
        let actual = actual.as_f64().expect("channel is numeric");
        assert!((actual - expected).abs() < 0.000_01);
    }
}

fn assert_number(actual: &serde_json::Value, expected: f64) {
    let actual = actual.as_f64().expect("value is numeric");
    assert!((actual - expected).abs() < 0.000_01);
}

#[test]
fn resolution_does_not_mutate_authored_world() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave" },
            "entities": [{
                "id": "panel",
                "transform_3d": { "scale": [0.5, 0.5, 1.0] },
                "components": {
                    "sindri.ui.image": { "texture": "sindri:white", "anchor": "center" }
                }
            }]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let before = source.clone();
    let sheet =
        parse("#panel { width: 420px; } @media (max-width: 700px) { #panel { width: 90vw; } }")
            .expect("Weave parses");
    let styled = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 390.0,
            height: 844.0,
        },
    )
    .expect("styles resolve");

    let source_scale = source
        .entities()
        .next()
        .expect("source entity")
        .1
        .transform_3d
        .expect("transform")
        .scale[0];
    let styled_scale = styled
        .world()
        .entities()
        .next()
        .expect("styled entity")
        .1
        .transform_3d
        .expect("transform")
        .scale[0];
    let before_scale = before
        .entities()
        .next()
        .expect("before entity")
        .1
        .transform_3d
        .expect("transform")
        .scale[0];
    assert_eq!(source_scale, before_scale);
    assert_ne!(styled_scale, source_scale);
}

#[test]
fn class_rules_style_shape_and_text_components() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-visuals" },
            "entities": [{
                "id": "panel",
                "transform_3d": { "scale": [0.5, 0.5, 1.0] },
                "components": {
                    "weave.style": { "classes": ["card"] },
                    "sindri.ui.shape": {
                        "kind": "rect",
                        "fill": [0.0, 0.0, 0.0, 1.0],
                        "anchor": "center"
                    },
                    "sindri.ui.text": {
                        "text": "hello",
                        "font": "fonts/test.ttf",
                        "font_size": 0.05
                    }
                }
            }]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let sheet = parse(
        r#"
            #panel { background: #abcdef; }
            .card {
                width: 400px;
                height: 200px;
                background: #112233;
                color: #f8fafc;
                border-color: #445566;
                border-width: 5%;
                border-radius: 20%;
                font-weight: 700;
                text-transform: uppercase;
                text-align: center;
            }
        "#,
    )
    .expect("Weave parses");

    let styled = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect("styles resolve");
    let (_, entity) = styled.world().entities().next().expect("styled entity");
    let transform = entity.transform_3d.expect("styled transform");
    assert_eq!(transform.scale[0], 1.0);
    assert_eq!(transform.scale[1], 0.5);

    let shape = entity
        .components
        .get("sindri.ui.shape")
        .expect("shape payload");
    assert_color(
        shape.get("fill").expect("fill"),
        [
            f64::from(srgb_channel(171)),
            f64::from(srgb_channel(205)),
            f64::from(srgb_channel(239)),
            1.0,
        ],
    );
    assert_color(
        shape.get("stroke").expect("stroke"),
        [
            f64::from(srgb_channel(68)),
            f64::from(srgb_channel(85)),
            f64::from(srgb_channel(102)),
            1.0,
        ],
    );
    assert_number(&shape["stroke_width"], 0.05);
    assert_number(&shape["corner_radius"], 0.2);

    let text = entity
        .components
        .get("sindri.ui.text")
        .expect("text payload");
    assert_color(
        text.get("color").expect("color"),
        [
            f64::from(srgb_channel(248)),
            f64::from(srgb_channel(250)),
            f64::from(srgb_channel(252)),
            1.0,
        ],
    );
    assert_eq!(text["bold"], true);
    assert_eq!(text["case"], "upper");
    assert_eq!(text["line_align"], "center");

    let source_entity = source.entities().next().expect("source entity").1;
    assert_eq!(source_entity.components["sindri.ui.shape"]["fill"][0], 0.0);
    assert!(
        source_entity.components["sindri.ui.text"]
            .get("bold")
            .is_none()
    );
}

#[test]
fn percentages_and_constraints_use_the_final_parent_box() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-sizing" },
            "entities": [
                {
                    "id": "child",
                    "name": "child",
                    "parent": "panel",
                    "transform_3d": { "scale": [0.1, 0.1, 1.0] },
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" }
                    }
                },
                {
                    "id": "panel",
                    "name": "panel",
                    "transform_3d": { "scale": [0.2, 0.2, 1.0] },
                    "components": {
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" }
                    }
                }
            ]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let sheet = parse(
        r#"
            #panel {
                width: 50vw;
                min-width: 600px;
                max-width: 700px;
                height: 400px;
            }
            #child {
                width: 50%;
                height: 150%;
                max-height: 100%;
            }
        "#,
    )
    .expect("Weave parses");

    let styled = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect("styles resolve");
    let scale = |name: &str| {
        styled
            .world()
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some(name))
            .and_then(|(_, data)| data.transform_3d)
            .expect("styled entity")
            .scale_2d()
    };

    assert_eq!(scale("panel"), [1.5, 1.0]);
    assert_eq!(scale("child"), [0.75, 1.0]);
}

#[test]
fn layout_alignment_maps_to_the_generic_sindri_layout_component() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-layout" },
            "entities": [{
                "id": "hero",
                "components": {
                    "sindri.ui.layout": { "direction": "column", "spacing": 0.25 }
                }
            }]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let sheet = parse(
        r#"
            #hero {
                direction: row;
                gap: 24px;
                justify-content: space-between;
                align-items: end;
            }
        "#,
    )
    .expect("Weave parses");

    let styled = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect("styles resolve");
    let (_, entity) = styled.world().entities().next().expect("styled entity");
    let layout = entity
        .components
        .get("sindri.ui.layout")
        .expect("layout payload");

    assert_eq!(layout["direction"], "row");
    assert_eq!(layout["justify"], "space_between");
    assert_eq!(layout["align"], "end");
    assert_number(&layout["spacing"], 0.06);

    let source_layout = &source
        .entities()
        .next()
        .expect("source entity")
        .1
        .components["sindri.ui.layout"];
    assert!(source_layout.get("justify").is_none());
    assert!(source_layout.get("align").is_none());
}

#[test]
fn negative_sizes_are_rejected_with_the_authored_property() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-invalid-sizing" },
            "entities": [{
                "id": "panel",
                "components": {
                    "sindri.ui.shape": { "kind": "rect", "anchor": "center" }
                }
            }]
        }"#,
    )
    .expect("scene parses");
    let source = World::from_scene(&document).expect("scene loads").world;
    let sheet = parse("#panel { min-width: -10px; }").expect("Weave parses");

    let error = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect_err("negative minimum is invalid");
    assert_eq!(
        error,
        ApplyError::InvalidValue {
            entity: "panel".into(),
            property: "min-width".into(),
            value: "-10px".into(),
        }
    );
}
