//! What a frame is made of, and in what order.

use glam::Vec3;
use sindri_core::Transform3D;
use sindri_render::{FrameCommand, RenderStage};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, scene, world_from};

#[test]
fn a_world_with_only_a_camera_draws_nothing() {
    let world = world_from(&scene(""));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("an empty scene extracts");
    assert!(frame.passes().is_empty());
}

#[test]
fn meshes_and_sprites_extract_into_ordered_passes() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 0 } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "anchor": "center", "layer": 100 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].stage, RenderStage::Opaque3d);
    assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
}

#[test]
fn meshes_keep_one_pass_per_layer_in_layer_order() {
    let world = world_from(&scene(
        r#",
        { "id": "high", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 5 } } },
        { "id": "low", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t", "layer": 1 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].layer.0, 1);
    assert_eq!(frame.passes()[1].layer.0, 5);
}

/// The seam this crate exists for: gameplay writes the world, drawing follows.
#[test]
fn writing_a_transform_changes_what_is_drawn() {
    let mut world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let cube = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "cube")
        })
        .map(|(entity, _)| entity)
        .expect("the cube is in the world");

    world.get_mut(cube).unwrap().transform_3d = Some(Transform3D {
        position: [4.0, 0.0, 0.0],
        ..Transform3D::default()
    });

    let frame = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the scene extracts");
    let FrameCommand::TexturedCube { model, .. } = frame.passes()[0].command else {
        panic!("expected a cube");
    };
    assert_eq!(model.w_axis.truncate(), Vec3::new(4.0, 0.0, 0.0));
}
