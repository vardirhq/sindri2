//! Where a UI element lands on the viewport, and what it does not depend on.

use sindri_render::{FrameCommand, RenderStage, SpriteDepth};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, close, document, scene, world_from};

fn extract(world: &sindri_core::World) -> sindri_render::PreparedFrame {
    SceneExtractor::new()
        .unwrap()
        .extract(
            world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the scene extracts")
}

/// UI images sort by their authored Z within the viewport-owned projection, so
/// the lower Z is further back in the stack and is drawn first.
#[test]
fn ui_images_batch_per_layer_and_sort_back_to_front() {
    let world = world_from(&document(
        r#"
        { "id": "near", "transform_3d": { "position": [0.0, 0.0, -1.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "far", "transform_3d": { "position": [0.0, 0.0, -9.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.75] } } },
        { "id": "other-layer", "transform_3d": {},
          "components": { "sindri.ui.image": { "texture": "b", "layer": 200 } } }"#,
    ));
    let frame = extract(&world);

    assert_eq!(frame.passes().len(), 2, "one batch per layer");
    assert_eq!(frame.passes()[0].layer.0, 100);
    assert_eq!(frame.passes()[1].layer.0, 200);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the first UI pass should be a sprite batch");
    };
    let alphas: Vec<f32> = instances.iter().map(|sprite| sprite.tint()[3]).collect();
    assert!(
        close(alphas[0], 0.75) && close(alphas[1], 0.25),
        "the element pushed further back must be drawn first, got {alphas:?}"
    );
}

/// The Z of a UI element orders it without moving it. That is the one place the
/// sort key and the drawn position deliberately disagree, and it is what keeps
/// a HUD from disappearing when someone pushes it far back.
#[test]
fn a_ui_image_is_sorted_by_its_z_but_not_moved_by_it() {
    let world = world_from(&scene(
        r#",
        { "id": "front", "transform_3d": { "position": [0.1, 0.2, 0.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "back", "transform_3d": { "position": [0.1, 0.2, -400.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
    ));
    let frame = extract(&world);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert!(
        close(instances[0].tint()[3], 0.75),
        "the element pushed back must be drawn first"
    );
    let placements: Vec<_> = instances
        .iter()
        .map(|sprite| sprite.model().w_axis.truncate())
        .collect();
    assert_eq!(
        placements[0], placements[1],
        "a UI element's Z must not move it, even four hundred units of it"
    );
    assert!(
        close(placements[0].z, 0.0),
        "UI elements draw flat on the viewport, at {placements:?}"
    );
}

/// Anchors resolve against the viewport-owned screen extent rather than a
/// constant. The values here are the world positions extraction has produced
/// since screen space stopped being an authored camera's business.
#[test]
fn ui_anchors_resolve_against_the_screen_extent() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "anchor": "bottom_right" } } }"#,
    ));
    let frame = extract(&world);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let translation = instances[0].model().w_axis.truncate();
    // Screen half-extent is (1.0, 1.0) at this aspect, so bottom right is
    // (1.0, -1.0) and the element offsets from there.
    assert!(
        close(translation.x, 0.22) && close(translation.y, -0.56),
        "anchored element landed at {translation:?}"
    );
}

/// An element that names no anchor sits in the middle, which is the one place
/// on a viewport that needs no argument.
#[test]
fn an_image_that_names_no_anchor_is_centred() {
    let world = world_from(&document(
        r#"
        { "id": "centred", "transform_3d": { "position": [0.25, 0.5, 0.0] },
          "components": { "sindri.ui.image": { "texture": "b" } } },
        { "id": "explicit", "transform_3d": { "position": [0.25, 0.5, 0.0] },
          "components": { "sindri.ui.image": {
            "texture": "b", "anchor": "center", "layer": 5 } } }"#,
    ));
    let frame = extract(&world);
    let placements: Vec<_> = frame
        .passes()
        .iter()
        .map(|pass| {
            let FrameCommand::SpriteBatch {
                depth, instances, ..
            } = &pass.command
            else {
                panic!("expected sprite batches");
            };
            (pass.stage, *depth, instances[0].model().w_axis.truncate())
        })
        .collect();
    assert_eq!(
        placements.len(),
        2,
        "different layers are different batches"
    );
    assert_eq!(placements[0].0, RenderStage::Overlay);
    assert_eq!(placements[0].1, SpriteDepth::Ignore);
    assert_eq!(
        placements[0].2, placements[1].2,
        "naming the default must not move the element"
    );
}

/// The two spaces use different projections and pipelines, so they cannot share
/// a draw call however much else they have in common.
#[test]
fn a_ui_image_and_a_world_sprite_do_not_share_a_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "hud", "transform_3d": {},
          "components": { "sindri.ui.image": { "texture": "b", "layer": 100 } } },
        { "id": "prop", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } }"#,
    ));
    let frame = extract(&world);

    // Same layer, same texture, and still two passes — and the world one is
    // drawn first, because the UI is over the world.
    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].stage, RenderStage::Transparent2d);
    assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
}
