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

/// Play runs the same fixed-step loop a shipped game runs.
///
/// A scene that behaves differently in the editor than in the build is a scene
/// nobody can trust a play-test of, and before this the editor stepped once per
/// *rendered* frame — so a scene simulated as fast as the machine happened to
/// draw.
mod fixed_stepping {
    use sindri_core::{FixedStepClock, FixedStepConfig};
    use std::time::Duration;

    fn clock() -> FixedStepClock {
        FixedStepClock::new(FixedStepConfig::default()).expect("the default config is valid")
    }

    /// The number of steps a second is the same however fast the display is.
    #[test]
    fn a_second_is_the_same_number_of_steps_at_any_frame_rate() {
        for (label, frame, frames) in [
            ("30 Hz", Duration::from_micros(33_333), 30_u32),
            ("60 Hz", Duration::from_micros(16_667), 60),
            ("144 Hz", Duration::from_micros(6_944), 144),
        ] {
            let mut clock = clock();
            let mut steps = 0;
            for _ in 0..frames {
                steps += clock.advance(frame).fixed_steps;
            }
            // A default step of 1/60 s, so a second is sixty of them, give or
            // take what the last partial step has not earned yet.
            assert!(
                (59..=61).contains(&steps),
                "{label} ran {steps} steps in a second"
            );
        }
    }

    /// A slow frame earns several steps rather than one long one.
    #[test]
    fn a_slow_frame_earns_several_steps() {
        let mut clock = clock();
        let steps = clock.advance(Duration::from_millis(50)).fixed_steps;
        assert!(steps >= 2, "a 50 ms frame ran {steps} step(s)");
    }

    /// A fast frame earns none, and the leftover is carried rather than lost.
    #[test]
    fn a_fast_frame_earns_none_and_nothing_is_lost() {
        let mut clock = clock();
        // A step is 1/60 s, so four 4 ms frames are 16 ms and still short of it.
        let quick = Duration::from_millis(4);
        for frame in 0..4 {
            assert_eq!(
                clock.advance(quick).fixed_steps,
                0,
                "frame {frame} earned a step it had not paid for"
            );
        }
        assert_eq!(
            clock.advance(quick).fixed_steps,
            1,
            "five quick frames did not add up to a step"
        );
    }

    /// A step taken by hand is the same length a frame would have handed out,
    /// so a scene single-stepped sixty times is a scene that played a second.
    #[test]
    fn a_hand_taken_step_is_the_same_length() {
        let clock = clock();
        assert_eq!(clock.fixed_delta(), FixedStepConfig::default().step);
    }

    /// A frame long enough to bankrupt the simulation is capped rather than
    /// spiralling: a machine that stalled must not then run a thousand steps.
    #[test]
    fn a_stalled_frame_does_not_run_away() {
        let mut clock = clock();
        let steps = clock.advance(Duration::from_secs(30)).fixed_steps;
        assert!(
            steps <= FixedStepConfig::default().max_steps_per_frame,
            "a stall ran {steps} steps"
        );
    }
}
