use sindri_core::{SceneDocument, World};
use sindri_weave::PresentationWorld;
use weave::{Viewport, parse};

#[test]
fn slider_and_authored_parts_are_styleable_without_a_parallel_control_model() {
    let document = SceneDocument::from_json(
        r#"{
            "format_version": 10,
            "metadata": { "name": "weave-slider" },
            "entities": [
                {
                    "id": "volume",
                    "transform_3d": { "scale": [0.5, 0.1, 1.0] },
                    "components": {
                        "weave.style": { "classes": ["mixer-slider"] },
                        "sindri.ui.slider": {
                            "label": "Volume",
                            "orientation": "horizontal",
                            "min": 0.0,
                            "max": 255.0,
                            "step": 1.0,
                            "value": 128.0
                        }
                    }
                },
                {
                    "id": "volume-track",
                    "parent": "volume",
                    "transform_3d": { "scale": [0.4, 0.03, 1.0] },
                    "components": {
                        "weave.style": { "classes": ["slider-track"] },
                        "sindri.ui.shape": { "kind": "rect", "anchor": "center" }
                    }
                },
                {
                    "id": "volume-thumb",
                    "parent": "volume",
                    "transform_3d": { "scale": [0.04, 0.08, 1.0] },
                    "components": {
                        "weave.style": { "classes": ["slider-thumb"] },
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
            sindri.ui.slider { width: 320px; height: 48px; }
            .slider-track { width: 100%; height: 8px; background: #30343b; }
            .slider-thumb { width: 20px; height: 36px; background: #f8fafc; border-radius: 20%; }
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

    let entity = |id: &str| {
        styled
            .world()
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|source| source.as_str() == id)
            })
            .map(|(_, data)| data)
            .expect("styled entity")
    };

    let slider = entity("volume");
    let slider_scale = slider.transform_3d.expect("slider transform").scale_2d();
    assert!((slider_scale[0] - 0.8).abs() < f32::EPSILON);
    assert!((slider_scale[1] - 0.12).abs() < f32::EPSILON);
    assert_eq!(slider.components["sindri.ui.slider"]["value"], 128.0);

    let track_scale = entity("volume-track")
        .transform_3d
        .expect("track transform")
        .scale_2d();
    assert!((track_scale[0] - 0.8).abs() < f32::EPSILON);
    assert!((track_scale[1] - 0.02).abs() < f32::EPSILON);

    let thumb = entity("volume-thumb");
    let thumb_scale = thumb.transform_3d.expect("thumb transform").scale_2d();
    assert!((thumb_scale[0] - 0.05).abs() < f32::EPSILON);
    assert!((thumb_scale[1] - 0.09).abs() < f32::EPSILON);
    assert!(
        thumb.components["sindri.ui.shape"]["corner_radius"]
            .as_f64()
            .is_some()
    );

    // Presentation is disposable: styling must not rewrite the authored value.
    let authored = source
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|source| source.as_str() == "volume")
        })
        .map(|(_, data)| data)
        .expect("authored slider");
    assert_eq!(authored.components["sindri.ui.slider"]["value"], 128.0);
}
