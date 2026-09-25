use std::time::Duration;

use super::{GamepadAxis, GamepadButton, PadId};
use crate::input::{InputEvent, InputState};

const A: PadId = PadId(10);
const B: PadId = PadId(20);

fn frame(input: &mut InputState) {
    input.begin_frame(Duration::from_millis(16));
}

fn press(input: &mut InputState, pad: PadId, button: GamepadButton) {
    input.apply(InputEvent::GamepadPressed { pad, button });
}

fn release(input: &mut InputState, pad: PadId, button: GamepadButton) {
    input.apply(InputEvent::GamepadReleased { pad, button });
}

fn connected(pads: &[PadId]) -> InputState {
    let mut input = InputState::default();
    for pad in pads {
        input.apply(InputEvent::GamepadConnected(*pad));
    }
    input
}

#[test]
fn a_pad_claims_the_first_free_slot_with_a_face_button() {
    let mut input = connected(&[A, B]);
    press(&mut input, B, GamepadButton::LeftBumper);
    assert_eq!(input.gamepads().joined(), None, "a bumper does not join");
    press(&mut input, B, GamepadButton::North);
    assert_eq!(input.gamepads().joined(), Some(1));
    assert!(input.gamepads().is_claimed(1));
    assert!(!input.gamepads().is_claimed(2));
    // The press that joined is not the slot's first action, though any-pad
    // readers still see it.
    assert!(!input.gamepads().pressed(1, GamepadButton::North));
    assert!(input.gamepads().pressed(0, GamepadButton::North));
    assert!(input.gamepads().down(1, GamepadButton::North));

    frame(&mut input);
    assert_eq!(input.gamepads().joined(), None);
    release(&mut input, B, GamepadButton::North);
    press(&mut input, B, GamepadButton::North);
    assert!(input.gamepads().pressed(1, GamepadButton::North));
    assert_eq!(
        input.gamepads().joined(),
        None,
        "a held slot does not rejoin"
    );
}

#[test]
fn two_pads_joining_in_one_frame_are_reported_one_frame_apart() {
    let mut input = connected(&[A, B]);
    press(&mut input, A, GamepadButton::South);
    press(&mut input, B, GamepadButton::South);
    assert_eq!(input.gamepads().joined(), Some(1));
    assert_eq!(input.gamepads().count(), 1);
    frame(&mut input);
    assert_eq!(input.gamepads().joined(), Some(2));
    assert_eq!(input.gamepads().count(), 2);
    assert!(input.gamepads().down(2, GamepadButton::South));
    frame(&mut input);
    assert_eq!(input.gamepads().joined(), None);
}

#[test]
fn unplugging_gives_the_slot_back_and_the_next_pad_takes_it() {
    let mut input = connected(&[A, B]);
    press(&mut input, A, GamepadButton::South);
    frame(&mut input);
    press(&mut input, B, GamepadButton::South);
    frame(&mut input);
    input.apply(InputEvent::GamepadDisconnected(A));
    assert_eq!(input.gamepads().left(), Some(1));
    assert!(!input.gamepads().is_claimed(1));
    assert!(
        input.gamepads().is_claimed(2),
        "the other player keeps theirs"
    );
    frame(&mut input);
    assert_eq!(input.gamepads().left(), None);
    input.apply(InputEvent::GamepadConnected(PadId(30)));
    press(&mut input, PadId(30), GamepadButton::Start);
    assert_eq!(input.gamepads().joined(), Some(1));
}

#[test]
fn a_slot_reads_only_its_own_pad() {
    let mut input = connected(&[A, B]);
    press(&mut input, A, GamepadButton::South);
    frame(&mut input);
    press(&mut input, B, GamepadButton::South);
    frame(&mut input);
    press(&mut input, B, GamepadButton::RightBumper);
    input.apply(InputEvent::GamepadAxisMoved {
        pad: A,
        axis: GamepadAxis::LeftX,
        value: 1.0,
    });
    assert!(!input.gamepads().pressed(1, GamepadButton::RightBumper));
    assert!(input.gamepads().pressed(2, GamepadButton::RightBumper));
    assert!((input.gamepads().axis(1, GamepadAxis::LeftX) - 1.0).abs() < 1e-6);
    assert!(input.gamepads().axis(2, GamepadAxis::LeftX).abs() < 1e-6);
    assert!((input.gamepads().axis(0, GamepadAxis::LeftX) - 1.0).abs() < 1e-6);
    assert!(
        !input.gamepads().down(3, GamepadButton::South),
        "an empty slot"
    );
    frame(&mut input);
    release(&mut input, B, GamepadButton::RightBumper);
    assert!(input.gamepads().released(2, GamepadButton::RightBumper));
}

#[test]
fn a_stick_at_rest_reads_zero_and_the_dead_zone_is_round() {
    let mut input = connected(&[A]);
    let mut stick = |x: f32, y: f32| {
        input.apply(InputEvent::GamepadAxisMoved {
            pad: A,
            axis: GamepadAxis::LeftX,
            value: x,
        });
        input.apply(InputEvent::GamepadAxisMoved {
            pad: A,
            axis: GamepadAxis::LeftY,
            value: y,
        });
        (
            input.gamepads().axis(0, GamepadAxis::LeftX),
            input.gamepads().axis(0, GamepadAxis::LeftY),
        )
    };
    assert_eq!(stick(0.1, -0.1), (0.0, 0.0), "drift");
    // Past the dead zone on the diagonal, though each half alone is inside it.
    let (x, y) = stick(0.18, 0.18);
    assert!(x > 0.0 && y > 0.0);
    let (x, _) = stick(1.0, 0.0);
    assert!((x - 1.0).abs() < 1e-6, "full deflection still reads one");
    let (x, _) = stick(f32::NAN, 0.0);
    assert!(x.abs() < 1e-6, "a broken reading is a stick at rest");
}

#[test]
fn starting_over_forgets_players_but_not_pads() {
    let mut input = connected(&[A]);
    press(&mut input, A, GamepadButton::South);
    frame(&mut input);
    input.forget_players();
    assert_eq!(input.gamepads().count(), 0);
    assert_eq!(input.gamepads().left(), None, "not a pad leaving");
    assert_eq!(input.gamepads().connected(), 1);
    release(&mut input, A, GamepadButton::South);
    press(&mut input, A, GamepadButton::South);
    assert_eq!(input.gamepads().joined(), Some(1));
}

#[test]
fn losing_focus_lets_go_of_every_button() {
    let mut input = connected(&[A]);
    press(&mut input, A, GamepadButton::South);
    input.apply(InputEvent::GamepadAxisMoved {
        pad: A,
        axis: GamepadAxis::RightTrigger,
        value: 1.0,
    });
    frame(&mut input);
    input.apply(InputEvent::FocusChanged(false));
    assert!(!input.gamepads().down(1, GamepadButton::South));
    assert!(input.gamepads().released(1, GamepadButton::South));
    assert!(input.gamepads().axis(1, GamepadAxis::RightTrigger).abs() < 1e-6);
    assert!(
        input.gamepads().is_claimed(1),
        "the player is still playing"
    );
}
