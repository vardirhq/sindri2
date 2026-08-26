//! Advancing a clip, and what it leaves the sprite showing.

use super::*;

fn walk() -> SpriteAnimationComponent {
    SpriteAnimationComponent {
        clips: BTreeMap::from([(
            "walk".to_owned(),
            AnimationClip {
                frames: vec![
                    "0".to_owned(),
                    "1".to_owned(),
                    "2".to_owned(),
                    "3".to_owned(),
                ],
                seconds_per_frame: 0.1,
                looping: true,
            },
        )]),
        playing: Some("walk".to_owned()),
        speed: 1.0,
    }
}

fn cursor() -> Playback {
    Playback {
        clip: "walk".to_owned(),
        frame: 0,
        elapsed: 0.0,
        finished: false,
        sprite: "0".to_owned(),
    }
}

#[test]
fn a_clip_advances_one_frame_per_its_own_duration() {
    let animation = walk();
    let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
    let mut playback = cursor();

    step(&mut playback, clip, 0.05);
    assert_eq!(playback.frame, 0, "half a frame is not a frame");
    step(&mut playback, clip, 0.05);
    assert_eq!(playback.frame, 1, "and the other half is");
    step(&mut playback, clip, 0.25);
    assert_eq!(
        playback.frame, 3,
        "two and a half more frames land on three"
    );
}

/// A stall does not spin: the step is arithmetic, so a delta covering a
/// million frames costs the same as one covering two.
#[test]
fn a_looping_clip_wraps_however_far_time_jumped() {
    let animation = walk();
    let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
    let mut playback = cursor();

    step(&mut playback, clip, 0.4);
    assert_eq!(playback.frame, 0, "four frames is a whole loop");
    step(&mut playback, clip, 100_000.0);
    assert_eq!(
        playback.frame, 0,
        "and a million frames is a whole lot of them"
    );
    assert!(!playback.finished, "a looping clip never finishes");
}

#[test]
fn a_clip_that_does_not_loop_holds_its_last_frame() {
    let mut animation = walk();
    animation
        .clips
        .get_mut("walk")
        .expect("walk is authored")
        .looping = false;
    let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
    let mut playback = cursor();

    step(&mut playback, clip, 10.0);
    assert_eq!(playback.frame, 3, "the end of the clip is where it stops");
    assert!(playback.finished);
    step(&mut playback, clip, 10.0);
    assert_eq!(playback.frame, 3, "and it stays stopped");
}

#[test]
fn no_time_passing_moves_nothing() {
    let animation = walk();
    let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
    let mut playback = cursor();

    step(&mut playback, clip, 0.0);
    assert_eq!(playback.frame, 0);
    assert_eq!(
        playback.elapsed.to_bits(),
        0.0_f32.to_bits(),
        "and no time is banked towards the next frame either"
    );
}

/// Every way of authoring a clip that cannot be played, named rather than
/// approximated.
#[test]
fn a_clip_that_cannot_be_played_says_why() {
    let mut animation = walk();
    animation.playing = Some("run".to_owned());
    assert!(matches!(
        animation.playing_clip(),
        Err(AnimationError::UnknownClip(_))
    ));

    let mut animation = walk();
    animation
        .clips
        .get_mut("walk")
        .expect("walk is authored")
        .frames
        .clear();
    assert!(matches!(
        animation.playing_clip(),
        Err(AnimationError::EmptyClip(_))
    ));

    let mut animation = walk();
    animation
        .clips
        .get_mut("walk")
        .expect("walk is authored")
        .seconds_per_frame = 0.0;
    assert!(matches!(
        animation.playing_clip(),
        Err(AnimationError::FrameHasNoDuration { .. })
    ));

    let mut animation = walk();
    animation.speed = -1.0;
    assert!(matches!(
        animation.playing_clip(),
        Err(AnimationError::BadSpeed(_))
    ));

    let mut animation = walk();
    animation.playing = None;
    assert!(
        animation.playing_clip().unwrap().is_none(),
        "and nothing playing is not a failure"
    );
}

#[test]
fn a_frame_names_the_sprite_its_clip_names() {
    let animation = SpriteAnimationComponent {
        clips: BTreeMap::from([(
            "walk".to_owned(),
            AnimationClip {
                frames: vec!["lift".to_owned(), "plant".to_owned()],
                seconds_per_frame: 0.1,
                looping: true,
            },
        )]),
        ..walk()
    };
    let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
    assert_eq!(
        SpriteAnimationComponent::frame_sprite(clip, 0).unwrap(),
        "lift",
        "a clip's frames are sprites of the sheet, in the order it names them"
    );
    assert_eq!(
        SpriteAnimationComponent::frame_sprite(clip, 1).unwrap(),
        "plant"
    );
    assert!(matches!(
        SpriteAnimationComponent::frame_sprite(clip, 2),
        Err(AnimationError::OutsideClip { .. })
    ));
}
