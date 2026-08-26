//! Binding a texture, and what is drawn when one is missing.

use sindri_render::{FrameCommand, TextureId, TextureRegistry};
use sindri_scene::{CameraView, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, animated_bindings, scene, world_from};

/// Sprites sharing a layer but not a texture cannot share a draw call.
#[test]
fn sprites_batch_per_texture_within_a_layer() {
    let world = world_from(&scene(
        r#",
        { "id": "a", "transform_3d": { "position": [0.0, 0.0, -2.0] },
          "components": { "sindri.sprite": { "texture": "one.png", "layer": 100 } } },
        { "id": "b", "transform_3d": { "position": [0.0, 0.0, -1.0] },
          "components": { "sindri.sprite": { "texture": "two.png", "layer": 100 } } },
        { "id": "c", "transform_3d": { "position": [0.0, 0.0, -3.0] },
          "components": { "sindri.sprite": { "texture": "one.png", "layer": 100 } } }"#,
    ));
    let mut bindings = TextureBindings::new();
    bindings.bind("one.png", TextureId::new(1));
    bindings.bind("two.png", TextureId::new(2));

    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("the scene extracts");

    assert_eq!(frame.passes().len(), 2, "one batch per texture");
    let batches: Vec<(TextureId, usize)> = frame
        .passes()
        .iter()
        .map(|pass| match &pass.command {
            FrameCommand::SpriteBatch {
                texture, instances, ..
            } => (*texture, instances.len()),
            FrameCommand::TexturedCube { .. } | FrameCommand::Text { .. } => {
                panic!("expected sprite batches")
            }
        })
        .collect();
    assert_eq!(batches, [(TextureId::new(1), 2), (TextureId::new(2), 1)]);
}

#[test]
fn an_unbound_texture_draws_as_missing_rather_than_failing() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "absent.png" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("a missing texture must not fail the frame");

    let FrameCommand::TexturedCube { texture, .. } = frame.passes()[0].command else {
        panic!("expected a cube");
    };
    assert_eq!(texture, TextureRegistry::MISSING);
}

#[test]
fn the_bound_texture_reaches_the_draw() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "world.png" } } }"#,
    ));
    let mut bindings = TextureBindings::new();
    bindings.bind("world.png", TextureId::new(7));

    let frame = SceneExtractor::new()
        .unwrap()
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("the scene extracts");
    let FrameCommand::TexturedCube { texture, .. } = frame.passes()[0].command else {
        panic!("expected a cube");
    };
    assert_eq!(texture, TextureId::new(7));
}

/// Missing textures are reported by name, not left as a magenta surprise.
#[test]
fn unresolved_references_are_reported_by_name() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "have.png" } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "lost.png" } } }"#,
    ));
    let mut bindings = TextureBindings::new();
    bindings.bind("have.png", TextureId::new(1));

    let missing = sindri_scene::unresolved_textures(&world, &bindings);
    assert_eq!(missing.len(), 1);
    assert!(missing.contains("lost.png"));

    bindings.bind("lost.png", TextureId::new(2));
    assert!(sindri_scene::unresolved_textures(&world, &bindings).is_empty());
}

#[test]
fn bindings_replace_rather_than_duplicate() {
    let mut bindings = TextureBindings::new();
    assert_eq!(bindings.bind("a.png", TextureId::new(1)), None);
    assert_eq!(
        bindings.bind("a.png", TextureId::new(2)),
        Some(TextureId::new(1))
    );
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings.resolve("a.png"), TextureId::new(2));
    assert_eq!(bindings.resolve("b.png"), TextureRegistry::MISSING);
    assert_eq!(bindings.get("b.png"), None);
}

/// A scene's texture references are the only statement anywhere of what it
/// needs loading, so asking for them has to include the ones already bound.
#[test]
fn a_world_lists_every_texture_it_draws_with_once() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "shared.png" } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "shared.png" } } },
        { "id": "other", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "textures/badge.png" } } }"#,
    ));

    let referenced = sindri_scene::referenced_textures(&world);
    assert_eq!(
        referenced.iter().map(String::as_str).collect::<Vec<_>>(),
        ["shared.png", "textures/badge.png"],
        "one entry per texture, however many entities name it"
    );

    let mut bindings = TextureBindings::new();
    bindings.bind("shared.png", TextureId::new(1));
    assert_eq!(
        sindri_scene::unresolved_textures(&world, &bindings)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["textures/badge.png"],
        "and what is missing is what is referenced and not bound"
    );
}

/// The property a sprite sheet exists for: many frames of one texture stay one
/// draw call. If the rect belonged to the batch instead of the instance, every
/// frame would be its own draw and the sheet would buy nothing.
#[test]
fn frames_of_one_sheet_share_a_single_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "a", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "sheet.png#0" } } },
        { "id": "b", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "sheet.png#1" } } },
        { "id": "c", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "sheet.png#2" } } }"#,
    ));
    let bindings = animated_bindings();

    let frame = SceneExtractor::new()
        .expect("built-in components register")
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("the sheet extracts");

    let batches: Vec<&FrameCommand> = frame
        .passes()
        .iter()
        .map(|pass| &pass.command)
        .filter(|command| matches!(command, FrameCommand::SpriteBatch { .. }))
        .collect();
    assert_eq!(
        batches.len(),
        1,
        "three frames of one sheet is one draw call"
    );

    let FrameCommand::SpriteBatch { instances, .. } = batches[0] else {
        unreachable!("filtered to sprite batches");
    };
    let mut rects: Vec<[f32; 4]> = instances
        .iter()
        .map(|instance| instance.uv_rect().to_array())
        .collect();
    rects.sort_by(|left, right| left.partial_cmp(right).expect("rects are finite"));
    assert_eq!(
        rects,
        [
            [0.0, 0.0, 0.5, 0.5],
            [0.0, 0.5, 0.5, 0.5],
            [0.5, 0.0, 0.5, 0.5],
        ],
        "and each instance kept its own frame"
    );
}

/// A scene written before rects existed reads as the whole texture, which is
/// what it drew.
#[test]
fn a_sprite_that_names_no_rect_draws_the_whole_texture() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "badge.png" } } }"#,
    ));
    let mut bindings = TextureBindings::new();
    bindings.bind("badge.png", TextureId::new(1));

    let frame = SceneExtractor::new()
        .expect("built-in components register")
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("the scene extracts");
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the scene draws one sprite batch");
    };
    assert!(instances[0].uv_rect().is_full());
}

/// A sprite naming a part its sheet does not have is reported by name, and
/// draws the whole image rather than failing the frame.
///
/// The same rule an unbound texture has always followed: the frame still draws,
/// so the failure has to be *said* — without that the only clue would be a
/// picture that is subtly the wrong part of an image.
#[test]
fn a_sprite_naming_nothing_in_its_sheet_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "bad", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "sheet.png#nope" } } }"#,
    ));
    let bindings = animated_bindings();
    SceneExtractor::new()
        .expect("built-in components register")
        .extract(&world, VIEWPORT, CameraView::default(), &bindings)
        .expect("an unresolved sprite still draws");
    assert!(
        sindri_scene::unresolved_sprites(&world, &bindings).contains("sheet.png#nope"),
        "the sprite nothing places is reported by name"
    );
}
