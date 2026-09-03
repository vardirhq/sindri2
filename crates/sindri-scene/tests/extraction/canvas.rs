//! The UI overlay as a rectangle in the scene rather than across the viewport.

use glam::Vec4;
use sindri_render::{FrameCommand, Viewport};
use sindri_scene::{
    CameraView, SceneExtractor, SceneRuntime, TextureBindings, UiCanvas, WorldProjection,
};

use crate::support::{VIEWPORT, document, world_from};

/// A UI image and a camera to look at it with.
fn scene() -> sindri_core::World {
    world_from(&document(
        r#"
        { "id": "camera", "name": "Camera",
          "transform_3d": { "position": [0.0, 0.0, 10.0] },
          "components": { "sindri.camera": {
            "projection": "orthographic", "vertical_size": 4.0,
            "near": 0.1, "far": 100.0 } } },
        { "id": "badge", "transform_3d": { "position": [0.0, 0.0, 0.0],
            "scale": [0.5, 0.25, 1.0] },
          "components": { "sindri.ui.image": {
            "texture": "procedural:badge", "anchor": "center", "layer": 10 } } }"#,
    ))
}

fn drawn_at_with(canvas: UiCanvas, viewport: Viewport, view: CameraView) -> Vec4 {
    let frame = SceneExtractor::new()
        .unwrap()
        .extract_animated(
            &scene(),
            viewport,
            view,
            &TextureBindings::new(),
            SceneRuntime::default().with_canvas(canvas),
        )
        .expect("the scene extracts");
    for pass in frame.passes() {
        if let FrameCommand::SpriteBatch { instances, .. } = &pass.command
            && let Some(first) = instances.first()
        {
            // Where the element's own origin lands in clip space, which is what
            // decides where on the screen it is drawn.
            return pass.camera.view_projection * first.model() * Vec4::W;
        }
    }
    panic!("nothing was drawn");
}

fn drawn_at(canvas: UiCanvas, viewport: Viewport) -> Vec4 {
    drawn_at_with(canvas, viewport, CameraView::default())
}

/// The viewer's camera, panned sideways by `pan`.
fn panned(pan: f32) -> CameraView {
    CameraView {
        projection: WorldProjection::Orthographic,
        pan: glam::Vec2::new(pan, 0.0),
        ..CameraView::default()
    }
}

fn ndc_x(clip: Vec4) -> f32 {
    (clip.truncate() / clip.w).x
}

/// On the viewport, no camera can move the overlay — that is what makes a HUD
/// a HUD, and it is why the Game view keeps it there.
#[test]
fn panning_cannot_move_an_overlay_that_is_on_the_viewport() {
    let still = ndc_x(drawn_at_with(UiCanvas::OnViewport, VIEWPORT, panned(0.0)));
    let panned = ndc_x(drawn_at_with(UiCanvas::OnViewport, VIEWPORT, panned(3.0)));
    assert!((still - panned).abs() < 1.0e-4, "{still} became {panned}");
}

/// In the scene it moves with everything else, which is the whole point: the
/// Scene view could pan and zoom around the world and the UI stayed stuck to
/// the glass, so there was no way to look closely at a menu.
#[test]
fn panning_moves_a_canvas_that_is_in_the_scene() {
    let canvas = UiCanvas::InScene { aspect: 1.0 };
    let still = ndc_x(drawn_at_with(canvas, VIEWPORT, panned(0.0)));
    let moved = ndc_x(drawn_at_with(canvas, VIEWPORT, panned(3.0)));
    assert!((still - moved).abs() > 0.1, "{still} barely became {moved}");
}

/// The canvas keeps the game's shape whatever shape the panel is.
///
/// A canvas that took the Scene panel's aspect would change every time the
/// splitter moved, which is the one thing a picture of the screen must not do.
#[test]
fn the_canvas_keeps_its_own_shape_when_the_panel_changes() {
    let wide = drawn_at(UiCanvas::InScene { aspect: 0.5 }, Viewport::new(1600, 400));
    let tall = drawn_at(UiCanvas::InScene { aspect: 0.5 }, Viewport::new(400, 1600));
    // The element is at the canvas centre in both, because the canvas did not
    // reshape itself around the viewport.
    for clip in [wide, tall] {
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() < 1.0e-4 && ndc.y.abs() < 1.0e-4, "{ndc:?}");
    }
}
