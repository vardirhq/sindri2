use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

#[test]
fn percentage_children_use_the_parent_content_box() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-padding" },
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
                width: 600px;
                height: 400px;
                padding: 50px;
            }
            #child {
                width: 100%;
                height: 100%;
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
    assert_eq!(scale("child"), [1.25, 0.75]);

    let source_child = source
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("child"))
        .and_then(|(_, data)| data.transform_3d)
        .expect("source child")
        .scale_2d();
    assert_eq!(source_child, [0.1, 0.1]);
}

#[test]
fn negative_padding_is_rejected() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 9,
            "metadata": { "name": "weave-padding-invalid" },
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
    let sheet = parse("#panel { padding: -8px; }").expect("Weave parses");

    let error = PresentationWorld::resolve(
        &source,
        &sheet,
        Viewport {
            width: 1_200.0,
            height: 800.0,
        },
    )
    .expect_err("negative padding is invalid");

    assert!(error.to_string().contains("padding"));
    assert!(error.to_string().contains("-8px"));
}
