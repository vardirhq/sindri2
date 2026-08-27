//! Play, pause, and stop, and what they let move.

use sindri_core::{EngineState, FixedStepConfig};

use super::super::console_view::lifecycle_label;
use super::super::runtime::{Transport, animation_delta, authoring_allowed, initialized_lifecycle};

/// The transport decides whether an animation moves, and nothing else does.
#[test]
fn only_a_running_engine_moves_an_animation_on() {
    let cap = FixedStepConfig::default().max_frame_delta.as_secs_f32();
    assert_eq!(
        animation_delta(EngineState::Running, 0.016).to_bits(),
        0.016_f32.to_bits(),
        "a running frame is worth its own length"
    );
    for state in [
        EngineState::Created,
        EngineState::Initialized,
        EngineState::Paused,
        EngineState::Stopped,
    ] {
        assert_eq!(
            animation_delta(state, 0.016).to_bits(),
            0.0_f32.to_bits(),
            "{state:?} does not move an animation on"
        );
    }
    assert_eq!(
        animation_delta(EngineState::Running, 60.0).to_bits(),
        cap.to_bits(),
        "and a minute behind another window is capped, not caught up on"
    );
    assert_eq!(
        animation_delta(EngineState::Running, f32::NAN).to_bits(),
        0.0_f32.to_bits(),
        "a frame time that is not a length of time is worth nothing"
    );
}

#[test]
fn the_lifecycle_drives_play_pause_and_stop() {
    let mut lifecycle = initialized_lifecycle();
    assert_eq!(lifecycle_label(lifecycle.state()), "ready");
    lifecycle.start().unwrap();
    assert_eq!(lifecycle_label(lifecycle.state()), "running");
    lifecycle.pause().unwrap();
    assert_eq!(lifecycle_label(lifecycle.state()), "paused");
    lifecycle.resume().unwrap();
    lifecycle.stop().unwrap();
    assert_eq!(lifecycle_label(lifecycle.state()), "stopped");
}

/// Every engine state is one of the three the transport can show, and the Play
/// button is labelled with what pressing it does rather than with where the
/// editor already is.
#[test]
fn the_transport_says_which_of_three_states_the_editor_is_in() {
    assert_eq!(Transport::of(EngineState::Running), Transport::Playing);
    assert_eq!(Transport::of(EngineState::Paused), Transport::Paused);
    for state in [
        EngineState::Created,
        EngineState::Initialized,
        EngineState::Stopped,
        EngineState::Destroyed,
    ] {
        assert_eq!(
            Transport::of(state),
            Transport::Editing,
            "{state:?} is not play mode"
        );
    }

    assert_eq!(Transport::Editing.play_label(), "Play");
    assert_eq!(
        Transport::Playing.play_label(),
        "Stop",
        "the button says what pressing it does"
    );
    assert_eq!(
        Transport::Paused.play_label(),
        "Stop",
        "including from a paused scene, which is still in play mode"
    );
    assert!(!Transport::Editing.is_playing());
    assert!(Transport::Playing.is_playing() && Transport::Paused.is_playing());
}

/// A running scene is not the document, so nothing may write to it.
///
/// The bug this holds shut: `save` writes the *live* world, so Ctrl+S while a
/// scene was playing replaced the authored file with wherever the scripts had
/// pushed everything — and Stop then restored a world the file no longer
/// matched, with the status bar reporting no unsaved work. Every guard in the
/// editor asks this one question, which is why it is worth a test of its own.
#[test]
fn nothing_may_be_authored_while_the_scene_is_playing() {
    assert!(
        !authoring_allowed(EngineState::Running),
        "a running scene is thrown away by Stop"
    );
    assert!(
        !authoring_allowed(EngineState::Paused),
        "a paused scene is still a run in progress, and Stop still throws it away"
    );
    for state in [
        EngineState::Created,
        EngineState::Initialized,
        EngineState::Stopped,
        EngineState::Destroyed,
    ] {
        assert!(
            authoring_allowed(state),
            "{state:?} is not play mode, so the world is the document"
        );
    }
}
