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

/// Text on a canvas in the scene grows when the view zooms in on it.
///
/// This is the half of "the UI is in the scene" that the projection alone did
/// not fix: an element's *position* came from the projection and so followed
/// the camera, while its size was worked out from the viewport and so did not.
/// Zooming moved the words around without making them any bigger.
///
/// Text is geometry now, so the check is the same one that would be made of any
/// other quad: a fixed offset on the canvas covers more of the viewport when the
/// camera is closer to it.
#[test]
fn zooming_in_on_a_canvas_makes_its_text_bigger() {
    use sindri_scene::overlay_in_scene;

    let scene = scene();
    let extractor = SceneExtractor::new().unwrap();
    let covered = |scale: f32| {
        let view = CameraView {
            projection: WorldProjection::Orthographic,
            distance_scale: scale,
            ..CameraView::default()
        };
        let world = extractor
            .world_camera_for_viewport(&scene, 1.0, view)
            .expect("a camera resolves")
            .expect("there is one");
        let (overlay, _) = overlay_in_scene(world, 1.0);
        // How much of the viewport one overlay unit of height covers, which is
        // what a font size is a share of.
        let at = |y: f32| overlay.viewport_fraction(glam::Vec2::new(0.0, y))[1];
        (at(1.0) - at(0.0)).abs()
    };

    let far = covered(2.0);
    let near = covered(1.0);
    assert!(
        near > far * 1.5,
        "a closer view should cover more of the viewport per unit: {near} against {far}"
    );
}

/// On the viewport it is the viewport that decides, which is what makes a HUD
/// the same size wherever the camera is.
#[test]
fn an_overlay_on_the_viewport_is_sized_by_the_viewport() {
    let (overlay, _) = sindri_scene::overlay_for_viewport(1.6).expect("an overlay");
    // The overlay is two units tall and centred on the origin, so its top edge
    // is one unit up and lands exactly at the top of the viewport.
    let top = overlay.viewport_fraction(glam::Vec2::new(0.0, 1.0));
    assert!(top[1].abs() < 1.0e-3, "{top:?}");
    let middle = overlay.viewport_fraction(glam::Vec2::ZERO);
    assert!((middle[1] - 0.5).abs() < 1.0e-3, "{middle:?}");
}
