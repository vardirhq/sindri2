//! How a voxel world's blocks are drawn beyond their texture: a face that
//! moves through frames, a block that glows, a block with holes in it.

use glam::Mat4;
use sindri_core::{SpriteSheetDocument, TileSetDocument};
use sindri_render::{FrameCommand, MeshSurface, TextureId};
use sindri_scene::{
    SceneExtractError, SceneExtractor, SceneRuntime, TextureBindings, TileSetBindings, ViewCamera,
};

use crate::support::{VIEWPORT, scene, world_from};

/// A flat world whose surface is `surface`, over dirt.
fn world(surface: &str) -> sindri_core::World {
    let entity = r#",
        { "id": "terrain", "transform_3d": {}, "components": {
          "sindri.voxel_world": {
            "generator": {
              "kind": "layered_terrain", "seed": 7,
              "base_height": 0, "height_variation": 0,
              "surface_voxel": "__SURFACE__", "subsurface_voxel": "dirt",
              "deep_voxel": "dirt", "subsurface_depth": 3
            },
            "blocks": "surfaces.tileset.json",
            "focus": [0, 0, 0], "render_radius": 0,
            "vertical_radius": 0, "layer": 0
          }
        } }"#
        .replace("__SURFACE__", surface);
    world_from(&scene(&entity))
}

/// Every face of a block showing `face`, which is a JSON face visual.
fn block(extra: &str, face: &str) -> String {
    format!(r#"{{ {extra} "faces": {{ "top": {face}, "south": {face}, "east": {face} }} }}"#)
}

fn blocks(broken: bool) -> TileSetBindings {
    let dirt = r#"{ "sprite": "dirt.png", "size": [1.0, 1.0] }"#;
    let water = r#"{ "sprite": "water.png#0", "size": [1.0, 1.0],
                     "animation": { "frames": ["water.png#1"], "fps": 2.0 } }"#;
    // A frame on another texture than its first: the faces are meshed with
    // the first, so there is no offset that shows the second.
    let mismatched = r#"{ "sprite": "water.png#0", "size": [1.0, 1.0],
                          "animation": { "frames": ["dirt.png"], "fps": 2.0 } }"#;
    let mut tiles = vec![
        format!(r#""dirt": {}"#, block("", dirt)),
        format!(
            r#""water": {}"#,
            block(r#""occludes": false, "tags": ["liquid"],"#, water)
        ),
        format!(r#""lava": {}"#, block(r#""glow": 0.9,"#, dirt)),
        format!(r#""leaves": {}"#, block(r#""occludes": false,"#, dirt)),
    ];
    if broken {
        tiles.push(format!(r#""broken": {}"#, block("", mismatched)));
    }
    let document = TileSetDocument::from_json(&format!(
        r#"{{ "format_version": 1, "tiles": {{ {} }} }}"#,
        tiles.join(", ")
    ))
    .expect("the block set parses");
    let mut bindings = TileSetBindings::new();
    bindings
        .bind("surfaces.tileset.json", document)
        .expect("the block set is valid");
    bindings
}

const WATER: TextureId = TextureId::new(2);

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("dirt.png", TextureId::new(1));
    bindings.bind("water.png", WATER);
    bindings
        .bind_sheet("water.png", &SpriteSheetDocument::from_grid(2, 1))
        .expect("the sheet binds");
    bindings
}

/// Every cached mesh drawn, with the texture and surface it draws with.
fn surfaces(
    surface: &str,
    seconds: f32,
) -> Result<Vec<(TextureId, MeshSurface)>, SceneExtractError> {
    let tile_sets = blocks(surface == "broken");
    let frame = SceneExtractor::new()
        .unwrap()
        .extract_animated_with_world_camera(
            &world(surface),
            VIEWPORT,
            ViewCamera {
                view: Mat4::IDENTITY,
                view_projection: Mat4::IDENTITY,
                framed_half_height: 1.0,
            },
            &textures(),
            SceneRuntime::default()
                .with_tile_sets(&tile_sets)
                .with_seconds(seconds),
        )?;
    Ok(frame
        .passes()
        .iter()
        .filter_map(|pass| match &pass.command {
            FrameCommand::CachedTexturedMesh {
                texture, surface, ..
            } => Some((*texture, *surface)),
            _ => None,
        })
        .collect())
}

#[test]
fn an_animated_face_moves_through_its_frames_without_being_rebuilt() {
    let water_at = |seconds| {
        surfaces("water", seconds)
            .expect("the world draws")
            .into_iter()
            .find(|(texture, _)| *texture == WATER)
            .map(|(_, surface)| surface.uv_offset)
            .expect("the water draws")
    };
    let first = water_at(0.0);
    let second = water_at(0.6);
    assert!(first[0].abs() < 1.0e-6, "{first:?}");
    assert!(
        (second[0] - 0.5).abs() < 1.0e-6,
        "half a second at two frames a second is the second frame, half the sheet across: {second:?}"
    );
}

#[test]
fn a_glowing_block_is_drawn_glowing_and_plain_ground_is_not() {
    let glows = |surface| -> Vec<f32> {
        surfaces(surface, 0.0)
            .expect("the world draws")
            .iter()
            .map(|(_, drawn)| drawn.glow)
            .collect()
    };
    let lava = glows("lava");
    assert!(
        !lava.is_empty() && lava.iter().all(|glow| (glow - 0.9).abs() < 1.0e-6),
        "{lava:?}"
    );
    let dirt = glows("dirt");
    assert!(
        !dirt.is_empty() && dirt.iter().all(|glow| *glow == 0.0),
        "{dirt:?}"
    );
}

#[test]
fn a_block_that_hides_nothing_is_cut_out() {
    let drawn = surfaces("leaves", 0.0).expect("the world draws");
    assert!(
        drawn.iter().any(|(_, surface)| surface.alpha_cutoff > 0.0),
        "{drawn:?}"
    );
}

#[test]
fn a_frame_on_another_texture_is_refused_naming_the_block() {
    let error = surfaces("broken", 0.0).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("block `broken`'s animation frame `dirt.png`"),
        "{error}"
    );
}
