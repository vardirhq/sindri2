//! Play, pause, and stop, and what they let move.

use super::super::{console_view::lifecycle_label, *};

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
