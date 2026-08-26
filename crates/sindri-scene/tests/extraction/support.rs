//! The worlds and scenes every extraction test builds on.

use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SpriteSheetDocument, UnknownComponentPolicy, World,
};
use sindri_render::{FrameCommand, TextureId, UvRect, Viewport};
use sindri_scene::{SceneExtractor, TextureBindings};

pub(crate) const VIEWPORT: Viewport = Viewport::new(512, 512);

pub(crate) fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-5
}

pub(crate) fn world_from(json: &str) -> World {
    let document = SceneDocument::from_json(json).expect("fixture scene parses");
    let extractor = SceneExtractor::new().expect("built-in components register");
    extractor
        .validate(&document, UnknownComponentPolicy::Reject)
        .expect("fixture scene matches the built-in schemas");
    World::from_scene(&document)
        .expect("fixture scene loads")
        .world
}

pub(crate) fn world_camera() -> &'static str {
    r#"
    {
      "id": "main-camera",
      "transform_3d": {
        "position": [3.0, 2.0, 4.0],
        "rotation": [-0.17940314, 0.31052187, 0.05980105, 0.93156564]
      },
      "components": { "sindri.camera": {
        "projection": "perspective",
        "vertical_fov_degrees": 45.0, "near": 0.1, "far": 100.0 } }
    }"#
}

pub(crate) fn scene(entities: &str) -> String {
    document(&format!("{}{entities}", world_camera()))
}

/// A document holding exactly the entities given, at whatever the current
/// format version is.
pub(crate) fn document(entities: &str) -> String {
    format!(r#"{{ "format_version": {SCENE_FORMAT_VERSION}, "entities": [{entities}] }}"#)
}

/// A world holding one sprite that reads a two-by-two sheet, with a `walk` clip
/// running its four cells at a tenth of a second each.
pub(crate) fn animated_sheet(playing: &str, looping: bool, speed: f32) -> World {
    animated_sheet_with_rect(playing, looping, speed, None)
}

pub(crate) fn animated_sheet_with_rect(
    playing: &str,
    looping: bool,
    speed: f32,
    rect: Option<&str>,
) -> World {
    // A sprite that names a part of its own sheet, which is what a scene now
    // writes instead of carrying a rect.
    let named = rect.map_or(String::new(), |name| format!("#{name}"));
    world_from(&scene(&format!(
        r#",
        {{ "id": "runner", "transform_3d": {{}},
          "components": {{
            "sindri.sprite": {{ "texture": "sheet.png{named}" }},
            "sindri.animation.sprite": {{
              "clips": {{ "walk": {{ "frames": ["0", "1", "2", "3"],
                "seconds_per_frame": 0.1, "looping": {looping} }} }},
              "playing": {playing},
              "speed": {speed}
            }}
          }} }}"#
    )))
}

/// The two-by-two sheet the animated fixture reads, as a host would load it.
pub(crate) fn animated_bindings() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("sheet.png", TextureId::new(1));
    bindings
        .bind_sheet("sheet.png", &SpriteSheetDocument::from_grid(2, 2))
        .expect("a two-by-two grid slices");
    bindings
}

pub(crate) fn only_instance_rect(frame: &sindri_render::PreparedFrame) -> UvRect {
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the scene draws one sprite batch");
    };
    assert_eq!(instances.len(), 1, "one sprite, one instance");
    instances[0].uv_rect()
}
