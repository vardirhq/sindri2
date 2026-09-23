//! A voxel world built from a block set names its blocks, and draws each with
//! the set's own art.

use glam::Mat4;
use sindri_core::TileSetDocument;
use sindri_render::{FrameCommand, TextureId};
use sindri_scene::{
    SceneExtractError, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings, ViewCamera,
};

use crate::support::{VIEWPORT, scene, world_from};

fn world(surface: &str) -> sindri_core::World {
    let entity = r#",
        { "id": "terrain", "transform_3d": {}, "components": {
          "sindri.voxel_world": {
            "generator": {
              "kind": "layered_terrain", "seed": 7,
              "base_height": 0, "height_variation": 0,
              "surface_voxel": "__SURFACE__", "subsurface_voxel": "dirt",
              "deep_voxel": "stone", "subsurface_depth": 3
            },
            "blocks": "terrain.tileset.json",
            "focus": [0, 0, 0], "render_radius": 0,
            "vertical_radius": 0, "layer": 0
          }
        } }"#
        .replace("__SURFACE__", surface);
    world_from(&scene(&entity))
}

fn blocks() -> TileSetBindings {
    let face = |sprite: &str| format!(r#"{{ "sprite": "{sprite}", "size": [1.0, 1.0] }}"#);
    let block = |top: &str, side: &str| {
        format!(
            r#"{{ "faces": {{ "top": {}, "south": {}, "east": {} }} }}"#,
            face(top),
            face(side),
            face(side)
        )
    };
    let document = TileSetDocument::from_json(&format!(
        r#"{{ "format_version": 1, "tiles": {{
             "grass": {}, "dirt": {}, "stone": {} }} }}"#,
        block("grass.png", "dirt.png"),
        block("dirt.png", "dirt.png"),
        block("stone.png", "stone.png")
    ))
    .expect("the block set parses");
    let mut bindings = TileSetBindings::new();
    bindings
        .bind("terrain.tileset.json", document)
        .expect("the block set is valid");
    bindings
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("grass.png", TextureId::new(1));
    bindings.bind("dirt.png", TextureId::new(2));
    bindings.bind("stone.png", TextureId::new(3));
    bindings
}

fn extract(
    world: &sindri_core::World,
    tile_sets: Option<&TileSetBindings>,
) -> Result<sindri_render::PreparedFrame, SceneExtractError> {
    SceneExtractor::new()
        .unwrap()
        .extract_animated_with_world_camera(
            world,
            VIEWPORT,
            ViewCamera {
                view: Mat4::IDENTITY,
                view_projection: Mat4::IDENTITY,
                framed_half_height: 1.0,
            },
            &textures(),
            SceneRuntime {
                tile_sets,
                ..SceneRuntime::default()
            },
        )
}

#[test]
fn a_world_built_from_blocks_draws_with_their_art() {
    let tile_sets = blocks();
    let frame = extract(&world("grass"), Some(&tile_sets)).expect("every block is in the set");
    let textures: Vec<&TextureId> = frame
        .passes()
        .iter()
        .filter_map(|pass| match &pass.command {
            FrameCommand::CachedTexturedMesh { texture, .. } => Some(texture),
            _ => None,
        })
        .collect();
    assert!(!textures.is_empty(), "the world draws");
    assert!(
        textures.contains(&&TextureId::new(1)),
        "the grass block's top is drawn: {textures:?}"
    );
}

#[test]
fn a_block_the_set_does_not_have_is_named() {
    let tile_sets = blocks();
    let error = extract(&world("lava"), Some(&tile_sets)).unwrap_err();
    assert_eq!(
        error.to_string(),
        "no block called `lava` in the world's block set"
    );
}

#[test]
fn a_block_set_still_loading_is_said_to_be() {
    let error = extract(&world("grass"), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        "tile set `terrain.tileset.json` has not been bound"
    );
}
