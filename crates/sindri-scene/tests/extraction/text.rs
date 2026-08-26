//! Text, laid out in pixels against the viewport.

use sindri_render::{FrameCommand, RenderStage};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings, referenced_fonts};

use crate::support::{VIEWPORT, close, document, world_from};

#[test]
fn text_extracts_in_pixels_from_a_screen_anchor_without_a_camera() {
    let world = world_from(&document(
        r#"
        { "id": "label", "transform_3d": { "position": [0.25, -0.5, 0.0] },
          "components": { "sindri.text": {
            "text": "Gather", "font": "fonts/Inter.ttf", "font_size": 18.0,
            "line_height": 24.0, "color": [1.0, 0.5, 0.25, 1.0],
            "anchor": "top_left", "layer": 101 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("screen text needs no authored camera");

    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Overlay);
    assert_eq!(frame.passes()[0].layer.0, 101);
    let FrameCommand::Text { instances } = &frame.passes()[0].command else {
        panic!("expected a text pass");
    };
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].text(), "Gather");
    assert_eq!(instances[0].font(), "fonts/Inter.ttf");
    let [x, y] = instances[0].position();
    assert!(close(x, 64.0) && close(y, 128.0));
    assert!(close(instances[0].font_size(), 18.0));
    assert!(close(instances[0].line_height(), 24.0));
    assert!(
        instances[0]
            .color()
            .into_iter()
            .zip([1.0, 0.5, 0.25, 1.0])
            .all(|(actual, expected)| close(actual, expected))
    );
    assert_eq!(
        referenced_fonts(&world).into_iter().collect::<Vec<_>>(),
        ["fonts/Inter.ttf"]
    );
}
