//! A clip playing, and what the scene still says while it does.

use sindri_core::World;
use sindri_render::UvRect;
use sindri_scene::{CameraView, SceneExtractor, SpriteAnimations};

use crate::support::{
    VIEWPORT, animated_bindings, animated_sheet, animated_sheet_with_rect, only_instance_rect,
    scene, world_from,
};

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
