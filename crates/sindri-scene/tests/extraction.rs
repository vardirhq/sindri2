//! Headless coverage for world-to-frame extraction.
//!
//! Everything here runs without a GPU: a scene is loaded into a world, the
//! world is extracted, and the resulting passes are inspected directly.

use std::error::Error;

use glam::{Vec2, Vec3};
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SpriteSheetDocument, Transform3D, UnknownComponentPolicy,
    World,
};
use sindri_render::{
    FrameCommand, RenderStage, SpriteDepth, TextureId, TextureRegistry, UvRect, Viewport,
};
use sindri_scene::{
    CameraView, FONT_NAMING_COMPONENTS, SceneExtractError, SceneExtractor, SpriteAnimations,
    TEXTURE_NAMING_COMPONENTS, TextureBindings, WorldProjection, referenced_fonts,
};

const VIEWPORT: Viewport = Viewport::new(512, 512);

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-5
}

fn world_from(json: &str) -> World {
    let document = SceneDocument::from_json(json).expect("fixture scene parses");
    let extractor = SceneExtractor::new().expect("built-in components register");
    extractor
        .validate(&document, UnknownComponentPolicy::Reject)
        .expect("fixture scene matches the built-in schemas");
    World::from_scene(&document)
        .expect("fixture scene loads")
        .world
}

fn cameras() -> &'static str {
    r#"
    {
      "id": "main-camera",
      "transform_3d": { "position": [3.0, 2.0, 4.0] },
      "components": { "sindri.camera": {
        "projection": "perspective", "target": [0.0, 0.0, 0.0], "up": [0.0, 1.0, 0.0],
        "vertical_fov_degrees": 45.0, "near": 0.1, "far": 100.0 } }
    },
    {
      "id": "overlay-camera",
      "components": { "sindri.camera": {
        "projection": "orthographic", "center": [0.0, 0.0],
        "vertical_size": 2.0, "near": 0.0, "far": 10.0 } }
    }"#
}

fn scene(entities: &str) -> String {
    document(&format!("{}{entities}", cameras()))
}

/// A document holding exactly the entities given, at whatever the current
/// format version is.
fn document(entities: &str) -> String {
    format!(r#"{{ "format_version": {SCENE_FORMAT_VERSION}, "entities": [{entities}] }}"#)
}

#[test]
fn a_world_with_only_cameras_draws_nothing() {
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
fn text_extracts_in_pixels_from_an_overlay_anchor() {
    let world = world_from(&scene(
        r#",
        { "id": "label", "transform_3d": { "position": [0.25, -0.5, 0.0] },
          "components": { "sindri.text": {
            "text": "Gather", "font": "fonts/Inter.ttf", "font_size": 18.0,
            "line_height": 24.0, "color": [1.0, 0.5, 0.25, 1.0],
            "anchor": "top_left", "layer": 101 } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("text extracts");

    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Overlay);
    assert_eq!(frame.passes()[0].layer.0, 101);
    let FrameCommand::Text { instances } = &frame.passes()[0].command else {
        panic!("expected a text pass");
    };
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].text(), "Gather");
    assert_eq!(instances[0].font(), "fonts/Inter.ttf");
    let [x, y] = instances[0].position();
    assert!(close(x, 64.0) && close(y, 128.0));
    assert!(close(instances[0].font_size(), 18.0));
    assert!(close(instances[0].line_height(), 24.0));
    assert!(
        instances[0]
            .color()
            .into_iter()
            .zip([1.0, 0.5, 0.25, 1.0])
            .all(|(actual, expected)| close(actual, expected))
    );
    assert_eq!(
        referenced_fonts(&world).into_iter().collect::<Vec<_>>(),
        ["fonts/Inter.ttf"]
    );
}

/// Sprites sort by where they are rather than by a number typed beside them:
/// the overlay camera looks along the axis from `+Z`, so the lower Z is further
/// away and is drawn first.
#[test]
fn sprites_batch_per_layer_and_sort_back_to_front() {
    let world = world_from(&scene(
        r#",
        { "id": "near", "transform_3d": { "position": [0.0, 0.0, -1.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "far", "transform_3d": { "position": [0.0, 0.0, -9.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "layer": 100, "tint": [1.0, 1.0, 1.0, 0.75] } } },
        { "id": "other-layer", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 200 } } }"#,
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

    assert_eq!(frame.passes().len(), 2, "one batch per layer");
    assert_eq!(frame.passes()[0].layer.0, 100);
    assert_eq!(frame.passes()[1].layer.0, 200);

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the first overlay pass should be a sprite batch");
    };
    assert_eq!(instances.len(), 2);
    let alphas: Vec<f32> = instances.iter().map(|sprite| sprite.tint()[3]).collect();
    assert!(
        close(alphas[0], 0.75) && close(alphas[1], 0.25),
        "the further sprite must be drawn first, got {alphas:?}"
    );
}

/// The Z of a screen-space sprite orders it without moving it. That is the one
/// place the sort key and the drawn position deliberately disagree, and it is
/// what keeps a HUD from disappearing when someone pushes it far back.
#[test]
fn a_screen_sprite_is_sorted_by_its_z_but_not_moved_by_it() {
    let world = world_from(&scene(
        r#",
        { "id": "front", "transform_3d": { "position": [0.1, 0.2, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "back", "transform_3d": { "position": [0.1, 0.2, -400.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
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

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert!(
        close(instances[0].tint()[3], 0.75),
        "the sprite pushed back must be drawn first"
    );
    let placements: Vec<_> = instances
        .iter()
        .map(|sprite| sprite.model().w_axis.truncate())
        .collect();
    assert_eq!(
        placements[0], placements[1],
        "a screen sprite's Z must not move it, even four hundred units of it"
    );
    assert!(
        close(placements[0].z, 0.0),
        "screen sprites draw flat against the overlay, at {placements:?}"
    );
}

/// A world sprite is sorted by its real distance from the camera, so moving the
/// camera can reverse two sprites without either of them moving.
#[test]
fn world_sprites_sort_by_distance_from_the_camera_that_draws_them() {
    let world = world_from(&scene(
        r#",
        { "id": "east", "transform_3d": { "position": [3.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "space": "world", "tint": [1.0, 1.0, 1.0, 0.25] } } },
        { "id": "west", "transform_3d": { "position": [-3.0, 0.0, 0.0] },
          "components": { "sindri.sprite": {
            "texture": "b", "space": "world", "tint": [1.0, 1.0, 1.0, 0.75] } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let alphas = |view| {
        let frame = extractor
            .extract(&world, VIEWPORT, view, &TextureBindings::new())
            .expect("the scene extracts");
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
            panic!("expected a sprite batch");
        };
        instances
            .iter()
            .map(|sprite| sprite.tint()[3])
            .collect::<Vec<f32>>()
    };

    // The authored camera is at (3, 2, 4), so the western sprite is further.
    let authored = alphas(CameraView::default());
    assert!(close(authored[0], 0.75) && close(authored[1], 0.25));

    // Half a turn around the target puts the camera on the other side, and the
    // pair must swap without the scene changing at all.
    let orbited = alphas(CameraView {
        orbit: Vec2::new(std::f32::consts::PI, 0.0),
        projection: WorldProjection::Perspective,
        ..CameraView::default()
    });
    assert!(
        close(orbited[0], 0.25) && close(orbited[1], 0.75),
        "orbiting past the sprites must reverse them, got {orbited:?}"
    );
}

/// A world-space sprite is in the world: it is drawn through the world camera,
/// in the transparent stage rather than the overlay, and its transform reaches
/// it whole — Z included, which a screen-anchored sprite has nowhere to put.
#[test]
fn world_space_sprites_draw_through_the_world_camera_with_their_full_transform() {
    let world = world_from(&scene(
        r#",
        { "id": "prop", "transform_3d": { "position": [1.0, 2.0, -3.0] },
          "components": { "sindri.sprite": { "texture": "b", "space": "world" } } }"#,
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

    assert_eq!(frame.passes().len(), 1);
    assert_eq!(frame.passes()[0].stage, RenderStage::Transparent2d);
    let FrameCommand::SpriteBatch {
        depth, instances, ..
    } = &frame.passes()[0].command
    else {
        panic!("expected a sprite batch");
    };
    assert_eq!(*depth, SpriteDepth::Test, "world sprites test depth");
    let translation = instances[0].model().w_axis.truncate();
    assert!(
        close(translation.x, 1.0) && close(translation.y, 2.0) && close(translation.z, -3.0),
        "the world sprite landed at {translation:?} rather than where it was authored"
    );

    // The world camera moved, so the sprite's picture must move with it. This
    // is what a screen-anchored sprite cannot do: the overlay camera cancels
    // its own centre.
    let orbited = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                orbit: Vec2::new(0.4, 0.0),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .expect("the scene extracts");
    assert_ne!(
        frame.passes()[0].camera.view_projection,
        orbited.passes()[0].camera.view_projection
    );
}

/// The default is what every sprite already was, so a scene written before the
/// choice existed keeps drawing exactly where it did.
#[test]
fn a_sprite_that_names_no_space_is_still_screen_anchored() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "anchor": "bottom_right" } } },
        { "id": "explicit", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "space": "screen",
            "anchor": "bottom_right", "layer": 5 } } }"#,
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
        "naming the default must not move the sprite"
    );
}

/// Two spaces are two cameras and two pipelines, so they cannot share a draw
/// call however much else they have in common.
#[test]
fn sprites_in_different_spaces_do_not_share_a_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "hud", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } },
        { "id": "prop", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "space": "world", "layer": 100 } } }"#,
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

    // Same layer, same texture, and still two passes — and the world one is
    // drawn first, because the overlay is over everything.
    assert_eq!(frame.passes().len(), 2);
    assert_eq!(frame.passes()[0].stage, RenderStage::Transparent2d);
    assert_eq!(frame.passes()[1].stage, RenderStage::Overlay);
}

/// A camera is required only when something needs it, and which camera a sprite
/// needs is now something the sprite says.
#[test]
fn each_sprite_space_asks_for_the_camera_it_uses() {
    let world_sprite_without_a_world_camera = document(
        r#"
        { "id": "overlay-camera",
          "components": { "sindri.camera": {
            "projection": "orthographic", "center": [0.0, 0.0],
            "vertical_size": 2.0, "near": 0.0, "far": 10.0 } } },
        { "id": "prop", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "space": "world" } } }"#,
    );
    let world = world_from(&world_sprite_without_a_world_camera);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingWorldCamera)
    ));

    let screen_sprite_without_an_overlay_camera = document(
        r#"
        { "id": "main-camera", "transform_3d": { "position": [3.0, 2.0, 4.0] },
          "components": { "sindri.camera": {
            "projection": "perspective", "target": [0.0, 0.0, 0.0], "up": [0.0, 1.0, 0.0],
            "vertical_fov_degrees": 45.0, "near": 0.1, "far": 100.0 } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b" } } }"#,
    );
    let world = world_from(&screen_sprite_without_an_overlay_camera);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingOverlayCamera)
    ));
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

/// Anchors resolve against the overlay camera's extent rather than a constant.
///
/// The values here are the world positions the previous hand-written extraction
/// produced, so the generalised anchor cannot silently move the demo overlay.
#[test]
fn sprite_anchors_resolve_against_the_overlay_extent() {
    let world = world_from(&scene(
        r#",
        { "id": "badge", "transform_3d": { "position": [-0.78, 0.44, 0.0] },
          "components": { "sindri.sprite": { "texture": "b", "anchor": "bottom_right" } } }"#,
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

    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let translation = instances[0].model().w_axis.truncate();
    // Overlay half-extent is (1.0, 1.0) at this aspect, so bottom right is
    // (1.0, -1.0) and the sprite offsets from there.
    assert!(
        close(translation.x, 0.22) && close(translation.y, -0.56),
        "anchored sprite landed at {translation:?}"
    );
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

#[test]
fn a_camera_view_moves_the_camera_without_moving_the_model() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let authored = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap();
    let orbited = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                orbit: glam::Vec2::new(0.5, 0.25),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();

    assert_ne!(
        authored.passes()[0].camera.view_projection,
        orbited.passes()[0].camera.view_projection
    );
    let (
        FrameCommand::TexturedCube { model: before, .. },
        FrameCommand::TexturedCube { model: after, .. },
    ) = (&authored.passes()[0].command, &orbited.passes()[0].command)
    else {
        panic!("expected cubes");
    };
    assert_eq!(before, after);
}

/// Where the authored target lands on screen once the view is panned.
///
/// The target sits at the centre of an unpanned frame, so its projected
/// position is exactly the pan, which is what makes the convention checkable.
fn projected_target(projection: WorldProjection, pan: glam::Vec2) -> glam::Vec2 {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                pan,
                projection,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();
    let clip = frame.passes()[0].camera.view_projection * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
    glam::Vec2::new(clip.x / clip.w, clip.y / clip.w)
}

/// Panning is measured in fractions of the framed half-height, so half a unit
/// moves the picture half a screen — and the same distance under either
/// projection, which is the whole reason it is not measured in world units.
#[test]
fn panning_moves_the_picture_by_the_same_amount_under_both_projections() {
    let unpanned = projected_target(WorldProjection::Perspective, glam::Vec2::ZERO);
    assert!(
        unpanned.abs_diff_eq(glam::Vec2::ZERO, 1.0e-5),
        "an unpanned camera should frame its target at the centre, got {unpanned:?}"
    );

    // A square viewport, so a pan of 0.5 half-heights is 0.5 in clip space.
    let perspective = projected_target(WorldProjection::Perspective, glam::Vec2::new(0.5, 0.0));
    let orthographic = projected_target(WorldProjection::Orthographic, glam::Vec2::new(0.5, 0.0));

    assert!(
        (perspective.x - 0.5).abs() < 1.0e-4,
        "panning right should move the picture right by half a screen, got {perspective:?}"
    );
    assert!(
        perspective.abs_diff_eq(orthographic, 1.0e-4),
        "the projections disagree about a pan: {perspective:?} against {orthographic:?}"
    );
}

#[test]
fn panning_up_moves_the_picture_up() {
    let panned = projected_target(WorldProjection::Perspective, glam::Vec2::new(0.0, 0.5));
    assert!(
        (panned.y - 0.5).abs() < 1.0e-4,
        "panning up should move the picture up, got {panned:?}"
    );
    assert!(panned.x.abs() < 1.0e-5, "panning up moved it sideways");
}

#[test]
fn panning_moves_the_camera_without_moving_the_model() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let authored = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap();
    let panned = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                pan: glam::Vec2::new(0.3, -0.2),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();

    assert_ne!(
        authored.passes()[0].camera.view_projection,
        panned.passes()[0].camera.view_projection
    );
    let (
        FrameCommand::TexturedCube { model: before, .. },
        FrameCommand::TexturedCube { model: after, .. },
    ) = (&authored.passes()[0].command, &panned.passes()[0].command)
    else {
        panic!("expected cubes");
    };
    assert_eq!(before, after, "panning must not move what it is looking at");
}

#[test]
fn a_pan_that_is_not_finite_is_rejected() {
    let world = world_from(&scene(""));
    let extracted = SceneExtractor::new().unwrap().extract(
        &world,
        VIEWPORT,
        CameraView {
            pan: glam::Vec2::new(f32::NAN, 0.0),
            ..CameraView::default()
        },
        &TextureBindings::new(),
    );
    assert!(matches!(
        extracted,
        Err(SceneExtractError::InvalidCameraPan)
    ));
}

#[test]
fn switching_the_world_projection_changes_only_the_world_camera() {
    let world = world_from(&scene(
        r#",
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } },
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b", "layer": 100 } } }"#,
    ));
    let extractor = SceneExtractor::new().unwrap();
    let perspective = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .unwrap();
    let orthographic = extractor
        .extract(
            &world,
            VIEWPORT,
            CameraView {
                projection: WorldProjection::Orthographic,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        )
        .unwrap();

    assert_ne!(
        perspective.passes()[0].camera.view_projection,
        orthographic.passes()[0].camera.view_projection
    );
    assert_eq!(
        perspective.passes()[1].camera.view_projection,
        orthographic.passes()[1].camera.view_projection
    );
}

#[test]
fn drawing_without_a_camera_reports_which_one_is_missing() {
    let mesh_only = document(
        r#"
        { "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t" } } }"#,
    );
    let world = world_from(&mesh_only);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingWorldCamera)
    ));

    let sprite_only = document(
        r#"
        { "id": "badge", "transform_3d": {},
          "components": { "sindri.sprite": { "texture": "b" } } }"#,
    );
    let world = world_from(&sprite_only);
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new()
        ),
        Err(SceneExtractError::MissingOverlayCamera)
    ));
}

#[test]
fn an_invalid_camera_distance_is_rejected() {
    let world = world_from(&scene(""));
    assert!(matches!(
        SceneExtractor::new().unwrap().extract(
            &world,
            VIEWPORT,
            CameraView {
                distance_scale: 0.0,
                ..CameraView::default()
            },
            &TextureBindings::new(),
        ),
        Err(SceneExtractError::InvalidCameraDistanceScale)
    ));
}

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

/// What a viewport's own chrome is drawn from. The orbit that turns the picture
/// has to turn the view it is asked for, or an axis indicator drawn from it
/// will disagree with the scene under it — which is exactly what the editor's
/// did while it was painted at fixed angles.
#[test]
fn the_world_camera_view_answers_the_orbit_the_frame_was_drawn_with() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");

    let resting = extractor
        .world_camera(&world, CameraView::default())
        .expect("the world holds a perspective camera")
        .expect("a perspective camera resolves")
        .view;
    // The authored camera sits at (3, 2, 4) looking at the origin, so world X
    // is already partly towards the viewer rather than straight across.
    let across = resting.transform_vector3(Vec3::X);
    assert!(across.x > 0.0, "world X points right of the picture");
    assert!(across.z > 0.0, "and towards the viewer");

    let quarter_turn = extractor
        .world_camera(
            &world,
            CameraView {
                orbit: Vec2::new(std::f32::consts::FRAC_PI_2, 0.0),
                projection: WorldProjection::Perspective,
                ..CameraView::default()
            },
        )
        .expect("the world holds a perspective camera")
        .expect("a perspective camera resolves")
        .view;
    let turned = quarter_turn.transform_vector3(Vec3::X);
    assert!(
        (turned - across).length() > 0.5,
        "a quarter turn has to move the axes: {across} became {turned}"
    );
}

/// A world with nothing to look through says so rather than inventing a view.
#[test]
fn a_world_with_no_perspective_camera_has_no_view_to_offer() {
    let world = world_from(&document(
        r#"{ "id": "cube", "transform_3d": {},
          "components": { "sindri.mesh": { "primitive": "cube", "texture": "t.png" } } }"#,
    ));
    let extractor = SceneExtractor::new().expect("built-in components register");
    assert_eq!(
        extractor
            .world_camera(&world, CameraView::default())
            .expect("asking is not an error"),
        None
    );
}

/// A pan of one moves the picture by exactly the framed half-height, which is
/// what lets an editor turn a distance on screen back into a pan and centre
/// itself on something.
#[test]
fn a_pan_of_one_moves_the_picture_by_the_framed_half_height() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");
    let camera = |pan| {
        extractor
            .world_camera(
                &world,
                CameraView {
                    pan,
                    projection: WorldProjection::Perspective,
                    ..CameraView::default()
                },
            )
            .expect("the world holds a perspective camera")
            .expect("a perspective camera resolves")
    };

    let resting = camera(Vec2::ZERO);
    let panned = camera(Vec2::new(0.0, 1.0));
    assert!(
        close(panned.framed_half_height, resting.framed_half_height),
        "panning frames the same amount of world"
    );
    // The origin was in the middle; after a pan of one upwards it is exactly a
    // half-height below it, measured in the view's own units.
    let before = resting.view.transform_point3(Vec3::ZERO);
    let after = panned.view.transform_point3(Vec3::ZERO);
    assert!(
        close(after.y - before.y, resting.framed_half_height),
        "a pan of one moved the origin by {} rather than {}",
        after.y - before.y,
        resting.framed_half_height
    );
}

/// The camera must never reach the pole, whatever it is asked for.
///
/// There the offset is parallel to `up` and nothing says which way round the
/// picture goes, so dragging through straight down whips the scene round to
/// face the other way. `look_at` returns a matrix rather than failing, which is
/// why this has to be checked on the angle rather than on a NaN. The guard
/// lives in the orbit maths because that is where the authored elevation is
/// known: a caller clamping its own pitch would be guessing at how far the
/// scene had already tilted.
#[test]
fn an_orbit_stops_short_of_the_pole_however_far_it_is_driven() {
    let world = world_from(&scene(""));
    let extractor = SceneExtractor::new().expect("built-in components register");
    let up = Vec3::Y;
    // The authored camera sits at (3, 2, 4), which is 1.19 radians off the up
    // axis, so this is the pitch that lands exactly on the pole.
    let onto_the_pole = -up.angle_between(Vec3::new(3.0, 2.0, 4.0));

    for pitch in [onto_the_pole, -100.0, -2.0, -1.5, 1.5, 2.0, 100.0] {
        for yaw in [0.0, 1.0, -2.5] {
            let camera = extractor
                .world_camera(
                    &world,
                    CameraView {
                        orbit: Vec2::new(yaw, pitch),
                        ..CameraView::default()
                    },
                )
                .expect("the world holds a perspective camera")
                .expect("a perspective camera resolves");
            assert!(
                camera.view.is_finite(),
                "yaw {yaw} pitch {pitch} produced {:?}",
                camera.view
            );
            // No pan, so the target is the origin and the camera's own position
            // is the offset the orbit produced.
            let eye = camera.view.inverse().w_axis.truncate();
            let polar = up.angle_between(eye);
            assert!(
                polar > 0.005 && polar < std::f32::consts::PI - 0.005,
                "yaw {yaw} pitch {pitch} put the camera {polar} radians from the pole"
            );
        }
    }
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

/// A world holding one sprite that reads a two-by-two sheet, with a `walk` clip
/// running its four cells at a tenth of a second each.
fn animated_sheet(playing: &str, looping: bool, speed: f32) -> World {
    animated_sheet_with_rect(playing, looping, speed, None)
}

fn animated_sheet_with_rect(playing: &str, looping: bool, speed: f32, rect: Option<&str>) -> World {
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
fn animated_bindings() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("sheet.png", TextureId::new(1));
    bindings
        .bind_sheet("sheet.png", &SpriteSheetDocument::from_grid(2, 2))
        .expect("a two-by-two grid slices");
    bindings
}

fn only_instance_rect(frame: &sindri_render::PreparedFrame) -> UvRect {
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("the scene draws one sprite batch");
    };
    assert_eq!(instances.len(), 1, "one sprite, one instance");
    instances[0].uv_rect()
}

fn runner(world: &World) -> sindri_core::EntityId {
    world
        .entities()
        .find(|(_, data)| data.components.contains_key("sindri.animation.sprite"))
        .map(|(entity, _)| entity)
        .expect("the world holds the animated sprite")
}

/// What the whole feature is for: time passing changes which part of the sheet a
/// sprite draws, without the scene changing at all.
#[test]
fn advancing_time_moves_a_sprite_through_its_sheet() {
    let world = animated_sheet(r#""walk""#, true, 1.0);
    let extractor = SceneExtractor::new().expect("built-in components register");
    let bindings = animated_bindings();
    let mut animations = SpriteAnimations::new();

    let mut seen = Vec::new();
    for _ in 0..4 {
        animations
            .advance(&world, extractor.components(), 0.1)
            .expect("the clip advances");
        let frame = extractor
            .extract_animated(
                &world,
                VIEWPORT,
                CameraView::default(),
                &bindings,
                &animations,
            )
            .expect("the animated scene extracts");
        seen.push(only_instance_rect(&frame));
    }

    let cells: Vec<UvRect> = (0..4)
        .map(|cell| UvRect::cell(cell % 2, cell / 2, 2, 2).expect("a cell of a two by two sheet"))
        .collect();
    // The first advance is a whole frame, so the run starts on cell one and
    // wraps back to cell zero.
    assert_eq!(seen, [cells[1], cells[2], cells[3], cells[0]]);
}

/// Nothing about the world changes as an animation runs. Playback is runtime
/// state, so a scene saved mid-run is the scene that was opened.
#[test]
fn playing_an_animation_does_not_change_the_scene() {
    let world = animated_sheet(r#""walk""#, true, 1.0);
    let extractor = SceneExtractor::new().expect("built-in components register");
    let saved = |world: &World| {
        world
            .to_scene()
            .expect("the world saves")
            .to_canonical_json()
            .expect("and writes canonically")
    };
    let before = saved(&world);

    let mut animations = SpriteAnimations::new();
    for _ in 0..10 {
        animations
            .advance(&world, extractor.components(), 0.1)
            .expect("the clip advances");
    }

    assert_eq!(saved(&world), before);
}

/// A sprite whose animation has never been advanced draws its authored rect,
/// which is what makes the authored rect the pose a scene shows at rest.
#[test]
fn an_unplayed_animation_leaves_the_sprites_own_rect_alone() {
    let world = animated_sheet_with_rect(r#""walk""#, true, 1.0, Some("1"));
    let frame = SceneExtractor::new()
        .expect("built-in components register")
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &animated_bindings(),
        )
        .expect("the scene extracts");
    assert_eq!(
        only_instance_rect(&frame),
        UvRect::new(0.5, 0.0, 0.5, 0.5).expect("the authored rect is valid")
    );
}

/// And one that authored no rect draws its clip's first frame instead of the
/// whole sheet. Drawing the sheet whole is every frame at once, which is never
/// a picture anyone meant — a scene loaded but not yet ticked, or an entity
/// sitting in the editor outside play mode, would otherwise look like that.
#[test]
fn an_unplayed_animation_without_a_rect_shows_its_first_frame() {
    let world = animated_sheet(r#""walk""#, true, 1.0);
    let frame = SceneExtractor::new()
        .expect("built-in components register")
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &animated_bindings(),
        )
        .expect("the scene extracts");
    // Cell zero of a two by two sheet, which is where advancing would start it.
    assert_eq!(
        only_instance_rect(&frame),
        UvRect::new(0.0, 0.0, 0.5, 0.5).expect("the first cell is valid")
    );
}

/// A clip nothing selected leaves the sprite alone too, rather than picking a
/// frame for it.
#[test]
fn an_animation_with_no_clip_playing_draws_nothing_of_its_own() {
    let world = animated_sheet("null", true, 1.0);
    let extractor = SceneExtractor::new().expect("built-in components register");
    let mut animations = SpriteAnimations::new();
    animations
        .advance(&world, extractor.components(), 1.0)
        .expect("an animation with nothing playing still advances");

    let frame = extractor
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &animated_bindings(),
            &animations,
        )
        .expect("the scene extracts");
    assert_eq!(only_instance_rect(&frame), UvRect::FULL);
    assert!(animations.frame(runner(&world)).is_none());
}

#[test]
fn speed_scales_how_fast_a_clip_runs() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    for (speed, expected) in [(0.0, 0), (0.5, 1), (2.0, 0)] {
        let world = animated_sheet(r#""walk""#, true, speed);
        let mut animations = SpriteAnimations::new();
        // Two tenths of a second: none of a frame at zero, one frame at half
        // speed, four frames — a whole loop — at double.
        animations
            .advance(&world, extractor.components(), 0.2)
            .expect("the clip advances");
        assert_eq!(
            animations.frame(runner(&world)),
            Some(expected),
            "at speed {speed}"
        );
    }
}

#[test]
fn a_clip_that_does_not_loop_finishes_and_can_be_restarted() {
    let world = animated_sheet(r#""walk""#, false, 1.0);
    let extractor = SceneExtractor::new().expect("built-in components register");
    let mut animations = SpriteAnimations::new();
    animations
        .advance(&world, extractor.components(), 10.0)
        .expect("the clip advances");

    let entity = runner(&world);
    assert_eq!(animations.frame(entity), Some(3), "it holds its last frame");
    assert!(animations.is_finished(entity));

    animations.restart(entity);
    animations
        .advance(&world, extractor.components(), 0.0)
        .expect("a restarted clip advances");
    assert_eq!(animations.frame(entity), Some(0), "restarting goes back");
    assert!(!animations.is_finished(entity));
}

/// A clip naming a sprite its sheet does not have is reported by name rather
/// than drawing a neighbouring frame's edge texels.
///
/// Advancing succeeds, because where a clip has got to does not depend on where
/// its sprites are — that is the sheet's business, and the sheet is consulted at
/// extraction. So the name survives to be reported there.
#[test]
fn a_clip_naming_a_sprite_the_sheet_lacks_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "runner", "transform_3d": {},
          "components": {
            "sindri.sprite": { "texture": "sheet.png" },
            "sindri.animation.sprite": {
              "clips": { "walk": { "frames": ["0", "9"], "seconds_per_frame": 0.1 } },
              "playing": "walk"
            }
          } }"#,
    ));
    let extractor = SceneExtractor::new().expect("built-in components register");
    let bindings = animated_bindings();
    let mut animations = SpriteAnimations::new();
    animations
        .advance(&world, extractor.components(), 0.1)
        .expect("a clip advances whether or not its sprites resolve");

    // Frame one of the clip is `9`, which a two-by-two sheet does not have, so
    // the sprite falls back to the whole image rather than to a neighbour.
    let frame = extractor
        .extract_animated(
            &world,
            VIEWPORT,
            CameraView::default(),
            &bindings,
            &animations,
        )
        .expect("an unresolved frame still draws");
    assert!(
        only_instance_rect(&frame).is_full(),
        "a frame nothing places draws the whole image, which is visibly wrong"
    );
}

/// A tilemap draws its filled cells and skips its empty ones.
#[test]
fn a_tilemap_draws_only_the_cells_that_hold_a_tile() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a", "b"],
            "columns": 3, "rows": 2, "space": "world",
            "tiles": [0, 1, null, 1, null, 0] } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the tilemap extracts");

    assert_eq!(frame.passes().len(), 1);
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert_eq!(
        instances.len(),
        4,
        "four of the six cells hold a tile, and the two nulls draw nothing"
    );
}

/// The tilemap's whole reason for existing: the same floor, authored as one
/// entity instead of one per tile, is the same picture.
#[test]
fn a_tilemap_places_its_tiles_where_loose_sprites_were() {
    let loose = world_from(&scene(
        r#",
        { "id": "a", "transform_3d": { "position": [0.5, -0.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "b", "transform_3d": { "position": [1.5, -0.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "c", "transform_3d": { "position": [0.5, -1.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } },
        { "id": "d", "transform_3d": { "position": [1.5, -1.5, 0.0] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } }"#,
    ));
    let mapped = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 2, "rows": 2, "space": "world",
            "tiles": [0, 0, 0, 0] } } }"#,
    ));

    let extractor = SceneExtractor::new().unwrap();
    let positions = |world: &World| -> Vec<Vec3> {
        let frame = extractor
            .extract(
                world,
                VIEWPORT,
                CameraView::default(),
                &TextureBindings::new(),
            )
            .expect("the scene extracts");
        let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
            panic!("expected a sprite batch");
        };
        let mut found: Vec<Vec3> = instances
            .iter()
            .map(|instance| instance.model().w_axis.truncate())
            .collect();
        found.sort_by(|left, right| {
            (left.x, left.y, left.z)
                .partial_cmp(&(right.x, right.y, right.z))
                .expect("no NaN in a placed tile")
        });
        found
    };

    let from_sprites = positions(&loose);
    let from_tilemap = positions(&mapped);
    assert_eq!(from_sprites.len(), 4);
    for (sprite, tile) in from_sprites.iter().zip(&from_tilemap) {
        assert!(
            close(sprite.x, tile.x) && close(sprite.y, tile.y) && close(sprite.z, tile.z),
            "one tilemap put a tile at {tile:?} where a sprite was at {sprite:?}"
        );
    }
}

/// A map's transform applies to the grid itself, not only to the quads in it.
/// Picking has always inverted the full model matrix, so rendering must compose
/// the same matrix or a rotated floor is painted somewhere other than it draws.
#[test]
fn a_tilemap_transform_moves_rotates_and_scales_its_grid() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {
            "position": [5.0, -2.0, 0.0],
            "rotation": [0.0, 0.0, 0.70710677, 0.70710677],
            "scale": [2.0, 3.0, 1.0] },
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 1, "rows": 1, "space": "world",
            "tiles": [0] } } }"#,
    ));
    let frame = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect("the transformed map extracts");
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    let position = instances[0].model().w_axis.truncate();
    assert!(
        close(position.x, 6.5) && close(position.y, -1.0),
        "the map-local centre is scaled and rotated before translation, got {position:?}"
    );
}

/// A map whose array does not match the size it claims is reported by name
/// rather than drawing part of a floor.
#[test]
fn a_tilemap_of_the_wrong_size_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 4, "rows": 4, "space": "world",
            "tiles": [0, 0] } } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("a map that is not the shape it claims does not extract");
    assert!(
        error.to_string().contains("16 cells"),
        "the error says how many cells the size calls for, got: {error}"
    );
}

/// A tile naming a cell the sheet does not have is reported rather than drawn
/// as whatever the maths happened to produce.
#[test]
fn a_tile_outside_the_sheet_is_reported() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a", "b"],
            "columns": 1, "rows": 1, "space": "world",
            "tiles": [7] } } }"#,
    ));
    let error = SceneExtractor::new()
        .unwrap()
        .extract(
            &world,
            VIEWPORT,
            CameraView::default(),
            &TextureBindings::new(),
        )
        .expect_err("a tile outside the sheet does not extract");
    assert!(
        error.to_string().contains("tile 7"),
        "the error names the tile, got: {error}"
    );
}

/// A tilemap and a loose sprite sharing a texture and a layer share a batch, so
/// a prop can sit among the floor rather than behind a plane of it.
#[test]
fn a_tilemap_and_a_sprite_share_one_batch() {
    let world = world_from(&scene(
        r#",
        { "id": "floor", "transform_3d": {},
          "components": { "sindri.tilemap": {
            "texture": "tiles", "palette": ["a"],
            "columns": 2, "rows": 1, "space": "world",
            "tiles": [0, 0] } } },
        { "id": "prop", "transform_3d": { "position": [0.5, -0.5, 0.5] },
          "components": { "sindri.sprite": { "texture": "tiles", "space": "world" } } }"#,
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

    assert_eq!(
        frame.passes().len(),
        1,
        "one texture and one layer is one batch"
    );
    let FrameCommand::SpriteBatch { instances, .. } = &frame.passes()[0].command else {
        panic!("expected a sprite batch");
    };
    assert_eq!(instances.len(), 3, "two tiles and the prop");
}

/// A component that names a texture and is not in `TEXTURE_NAMING_COMPONENTS`
/// never has that texture requested, so it draws as the magenta checker while
/// everything else about it works. `sindri.tilemap` did exactly that.
///
/// This holds the list against the registry rather than against a second list,
/// so the way to pass it is to add the component, not to update the test.
#[test]
fn every_component_that_names_a_texture_is_one_hosts_load_from() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    for metadata in extractor.components().registered_components() {
        let Some(default) = extractor.components().default_payload(&metadata.type_name) else {
            continue;
        };
        if default.get("texture").is_none() {
            continue;
        }
        assert!(
            TEXTURE_NAMING_COMPONENTS.contains(&metadata.type_name.as_str()),
            "{} names a texture but hosts never load it, so it draws magenta",
            metadata.type_name
        );
    }
}

/// The same guard for fonts, where the symptom is quieter still: a component
/// that names a font and is not in `FONT_NAMING_COMPONENTS` never has it
/// requested, so nothing binds it and the text draws as nothing at all — a
/// blank where a label should be, with no failed frame to point at it.
///
/// This cannot be held against default payloads the way the list above is: a
/// component that names a font has no sensible blank, because it cannot invent
/// a font, so it is registered without a default and would never reach the
/// assert. It is held against the *validator* instead. A component reads a
/// `font` field exactly when handing it a wrongly-typed one changes what
/// validation says, so the probe below finds every such component whether or
/// not it can be created from nothing.
///
/// Both directions are checked, so the list cannot go stale in either: a
/// component that reads a font has to be named, and a name that no longer reads
/// one has to go.
#[test]
fn every_component_that_names_a_font_is_one_hosts_load_from() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    let reads_a_font = |type_name: &str| {
        let complaint = |payload: &serde_json::Value| {
            extractor
                .components()
                .validate_payload(type_name, payload)
                .err()
                .and_then(|error| error.source().map(ToString::to_string))
        };
        complaint(&serde_json::json!({})) != complaint(&serde_json::json!({ "font": 0 }))
    };

    for metadata in extractor.components().registered_components() {
        assert_eq!(
            reads_a_font(&metadata.type_name),
            FONT_NAMING_COMPONENTS.contains(&metadata.type_name.as_str()),
            "{} reads a font field without being listed, or is listed without \
             reading one. The first draws no text at all, because hosts never \
             load a font for it; the second is a name that loads nothing.",
            metadata.type_name
        );
    }
}

/// An animated sprite needs its sheet even though its own reference names no
/// part of one.
///
/// The trap this guards: which part an animated sprite draws is its clip's
/// business, so its `texture` carries no fragment — and a host that asks for
/// sheets by looking at fragments alone never asks for this one. The sprite
/// then resolves its whole texture, which is every frame of the sheet at once.
#[test]
fn an_animated_sprite_asks_for_the_sheet_its_clips_read() {
    let world = world_from(&scene(
        r#",
        { "id": "runner", "transform_3d": {},
          "components": {
            "sindri.sprite": { "texture": "textures/walk.png" },
            "sindri.animation.sprite": {
              "clips": { "walk": { "frames": ["0", "1"], "seconds_per_frame": 0.1 } },
              "playing": "walk"
            }
          } }"#,
    ));
    assert!(
        sindri_scene::referenced_sheets(&world).contains("textures/walk.sheet.json"),
        "a sprite whose clips name parts of a sheet needs that sheet loaded, \
         even though its own reference names no part of one"
    );
}
