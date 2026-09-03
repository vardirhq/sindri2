//! Text, authored as a share of the screen and drawn as geometry on it.

use sindri_render::{FrameCommand, RenderStage};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings, referenced_fonts};

use crate::support::{VIEWPORT, close, document, world_from};

#[test]
fn text_is_sized_as_a_share_of_the_viewport_without_a_camera() {
    let world = world_from(&document(
        r#"
        { "id": "label", "transform_3d": { "position": [0.25, -0.5, 0.0] },
          "components": { "sindri.ui.text": {
            "text": "Gather", "font": "fonts/Inter.ttf", "font_size": 0.125,
            "line_height": 0.1875, "color": [1.0, 0.5, 0.25, 1.0],
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
    // Overlay units all the way to the renderer. The label is anchored top
    // left, which is the corner at (-aspect, 1) — one, here, for a square
    // viewport — and its transform is an offset from there.
    let [x, y] = instances[0].position();
    assert!(close(x, -0.75) && close(y, 0.5), "{x} {y}");
    // And the authored share is carried through untouched: nothing between the
    // scene and the quads converts it to a pixel count, so the same frame draws
    // at any resolution.
    assert!(close(instances[0].font_size(), 0.125));
    assert!(close(instances[0].line_height(), 0.1875));
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
