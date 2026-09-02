use glam::Mat4;
use serde_json::json;
use sindri_core::{EntityData, SceneComponent, Transform3D, World};
use sindri_render::{RenderStage, Viewport};
use sindri_scene::{
    CameraView, MeshComponent, SceneExtractError, SceneExtractor, TextureBindings, ViewCamera,
};

fn world_with_mesh_and_no_camera() -> World {
    let mut world = World::default();
    let mut data = EntityData {
        transform_3d: Some(Transform3D::default()),
        ..EntityData::default()
    };
    data.components.insert(
        MeshComponent::TYPE_NAME.to_owned(),
        json!({
            "primitive": "cube",
            "texture": "procedural:checkerboard",
            "layer": 0
        }),
    );
    world.spawn(data);
    world
}

#[test]
fn gameplay_extraction_still_requires_an_authored_world_camera() {
    let extractor = SceneExtractor::new().unwrap();
    let world = world_with_mesh_and_no_camera();
    let error = extractor
        .extract(
            &world,
            Viewport::new(640, 360),
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap_err();

    assert!(matches!(error, SceneExtractError::MissingWorldCamera));
}

#[test]
fn scene_view_extraction_needs_no_authored_world_camera() {
    let extractor = SceneExtractor::new().unwrap();
    let world = world_with_mesh_and_no_camera();
    let camera = ViewCamera {
        view: Mat4::IDENTITY,
        view_projection: Mat4::from_scale(glam::Vec3::splat(2.0)),
        framed_half_height: 3.0,
    };

    let frame = extractor
        .extract_animated_with_world_camera(
            &world,
            Viewport::new(640, 360),
            camera,
            &TextureBindings::new(),
            sindri_scene::SceneRuntime::default(),
        )
        .unwrap();

    let world_pass = frame
        .passes()
        .iter()
        .find(|pass| pass.stage == RenderStage::Opaque3d)
        .expect("mesh should produce a world pass");
    assert_eq!(world_pass.camera.view_projection, camera.view_projection);
}
