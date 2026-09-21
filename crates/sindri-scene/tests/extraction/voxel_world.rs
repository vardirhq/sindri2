//! Engine voxel worlds are extracted through the same scene path as games and
//! the editor, while their compiled sections persist between frames.

use glam::Mat4;
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{
    CameraView, SceneExtractor, SceneRuntime, TextureBindings, ViewCamera,
};

use crate::support::{VIEWPORT, scene, world_from};

fn voxel_world() -> sindri_core::World {
    voxel_world_with_radius(0)
}

fn voxel_world_with_radius(render_radius: u32) -> sindri_core::World {
    world_from(&scene(&format!(
        r#",
        { "id": "terrain", "transform_3d": {}, "components": {
          "sindri.voxel_world": {
            "generator": {
              "kind": "layered_terrain", "seed": 7,
              "base_height": 0, "height_variation": 0,
              "surface_voxel": 1, "subsurface_voxel": 2,
              "deep_voxel": 3, "subsurface_depth": 3
            },
            "materials": [
              { "voxel": 1, "top": "surface.png", "side": "soil.png",
                "bottom": "soil.png" },
              { "voxel": 2, "top": "soil.png", "side": "soil.png",
                "bottom": "soil.png" },
              { "voxel": 3, "top": "stone.png", "side": "stone.png",
                "bottom": "stone.png" }
            ],
            "focus": [0, 0, 0], "render_radius": {render_radius},
            "vertical_radius": 0, "layer": 0
          }
        } }"#,
    )))
}

#[test]
fn resident_sections_outside_the_camera_frustum_are_not_submitted() {
    let extractor = SceneExtractor::new().unwrap();
    let world = voxel_world_with_radius(1);
    let frame = extractor
        .extract_animated_with_world_camera(
            &world,
            VIEWPORT,
            ViewCamera {
                view: Mat4::IDENTITY,
                view_projection: Mat4::IDENTITY,
                framed_half_height: 1.0,
            },
            &textures(),
            SceneRuntime::default(),
        )
        .expect("the voxel world extracts through the test camera");
    let submitted = frame
        .passes()
        .iter()
        .filter(|pass| matches!(&pass.command, FrameCommand::CachedTexturedMesh { .. }))
        .count();
    assert!(submitted > 0, "sections intersecting the frustum remain visible");
    assert!(
        submitted < 9,
        "the 3x3 resident window should not all be submitted through a unit clip volume"
    );
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("surface.png", TextureId::new(1));
    bindings.bind("soil.png", TextureId::new(2));
    bindings.bind("stone.png", TextureId::new(3));
    bindings
}

#[test]
fn settled_voxel_world_reuses_its_compiled_section() {
    let extractor = SceneExtractor::new().unwrap();
    let world = voxel_world();
    let textures = textures();

    let first = extractor
        .extract(&world, VIEWPORT, CameraView::default(), &textures)
        .expect("the voxel world extracts");
    assert!(first.passes().iter().any(|pass| matches!(
        &pass.command,
        FrameCommand::CachedTexturedMesh {
            replacement: Some(_),
            ..
        }
    )));

    let settled = extractor
        .extract(&world, VIEWPORT, CameraView::default(), &textures)
        .expect("the settled voxel world extracts");
    let cached = settled
        .passes()
        .iter()
        .filter_map(|pass| match &pass.command {
            FrameCommand::CachedTexturedMesh { replacement, .. } => Some(replacement),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!cached.is_empty(), "settled sections remain drawable");
    assert!(
        cached.iter().all(|replacement| replacement.is_none()),
        "an unchanged resident section must not upload replacement geometry"
    );
}
