//! What one frame of input adds up to.
//!
//! Kept beside the state rather than inside it because the two together run
//! past the file-size cap, and what is checked here is a policy -- what a frame
//! reports as held, pressed and released -- that reads better as a list.

use super::MouseButton;
use super::state::{InputEvent, InputState};
use std::time::Duration;

fn down(state: &mut InputState, id: u64, x: f32, y: f32) {
    state.apply(InputEvent::TouchStarted { id, x, y });
}

const FRAME: Duration = Duration::from_millis(16);

/// Positions are compared by nearness, not equality: they are arrived at
/// by arithmetic, and the repository lints exact float comparison for the
/// reason that arithmetic does not promise exact answers.
#[track_caller]
fn assert_at(actual: [f32; 2], expected: [f32; 2]) {
    assert!(
        (actual[0] - expected[0]).abs() < f32::EPSILON
            && (actual[1] - expected[1]).abs() < f32::EPSILON,
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn a_tap_and_a_click_arrive_as_the_same_shape() {
    // The point of the press model: one kind of thing, whatever made it.
    // Every difference that used to leak into gameplay -- a finger with no
    // position at release, a mouse that keeps one -- is settled here.
    let mut touch = InputState::default();
    touch.apply(InputEvent::TouchStarted {
        id: 7,
        x: 4.0,
        y: 9.0,
    });
    let began = *touch.presses().primary().expect("a finger is a press");
    assert_at(began.position(), [4.0, 9.0]);

    let mut click = InputState::default();
    click.apply(InputEvent::PointerMoved { x: 4.0, y: 9.0 });
    click.apply(InputEvent::ButtonPressed(MouseButton::Left));
    let pressed = *click.presses().primary().expect("a button is a press");
    assert_at(pressed.position(), began.position());
    assert_eq!(pressed.phase(), began.phase());

    // And both end where they were, which is the fact a button needs.
    touch.begin_frame(FRAME);
    touch.apply(InputEvent::TouchEnded { id: 7 });
    click.begin_frame(FRAME);
    click.apply(InputEvent::ButtonReleased(MouseButton::Left));
    assert_eq!(touch.presses().ended().count(), 1);
    assert_eq!(click.presses().ended().count(), 1);
    assert_at(touch.presses().focus().expect("a focus"), [4.0, 9.0]);
    assert_at(click.presses().focus().expect("a focus"), [4.0, 9.0]);
}

#[test]
fn a_press_is_aged_by_the_frames_it_survives() {
    let mut state = InputState::default();
    state.apply(InputEvent::TouchStarted {
        id: 1,
        x: 0.0,
        y: 0.0,
    });
    state.begin_frame(FRAME);
    state.begin_frame(FRAME);
    assert_eq!(
        state.presses().primary().map(sindri_core::Press::held_for),
        Some(FRAME * 2)
    );
}

#[test]
fn losing_the_window_cancels_a_press_rather_than_completing_it() {
    let mut state = InputState::default();
    state.apply(InputEvent::TouchStarted {
        id: 1,
        x: 1.0,
        y: 1.0,
    });
    state.begin_frame(FRAME);
    state.apply(InputEvent::FocusChanged(false));
    assert_eq!(
        state.presses().ended().count(),
        0,
        "a window lost mid-drag must not click anything"
    );
}

#[test]
fn a_tap_reports_where_the_finger_left() {
    // The frame a finger lifts is the frame a press is completed, and
    // completing one asks where the pointer is. A mouse is still wherever
    // it was let go; a finger is gone. Reporting nothing there means a tap
    // releases over no element, so it never lands on the one it began on
    // -- which is every button on a touch device.
    let mut state = InputState::default();
    down(&mut state, 1, 10.0, 20.0);
    assert_eq!(state.pointer_position(), Some([10.0, 20.0]));

    state.begin_frame(FRAME);
    state.apply(InputEvent::TouchEnded { id: 1 });
    assert!(state.pointer_released(MouseButton::Left));
    assert_eq!(
        state.pointer_position(),
        Some([10.0, 20.0]),
        "the release frame has to know where the finger was"
    );

    // And once the frame is over the finger really is gone.
    state.begin_frame(FRAME);
    assert_eq!(state.pointer_position(), None);
}

#[test]
fn a_finger_is_where_the_host_put_it() {
    let mut state = InputState::default();
    down(&mut state, 1, 10.0, 20.0);
    assert_eq!(state.touch_count(), 1);
    assert_eq!(state.touch_at(0), Some([10.0, 20.0]));

    state.apply(InputEvent::TouchMoved {
        id: 1,
        x: 30.0,
        y: 40.0,
    });
    assert_eq!(state.touch_at(0), Some([30.0, 40.0]));

    state.apply(InputEvent::TouchEnded { id: 1 });
    assert_eq!(state.touch_count(), 0);
    assert_eq!(state.touch_at(0), None);
}

/// A game that aims at a point should not have to ask which device the
/// person is using.
#[test]
fn the_pointer_is_the_mouse_or_the_first_finger() {
    let mut state = InputState::default();
    assert_eq!(state.pointer_position(), None);

    down(&mut state, 7, 5.0, 6.0);
    assert_eq!(state.pointer_position(), Some([5.0, 6.0]));

    // A machine with both is a machine someone is using the mouse on.
    state.apply(InputEvent::PointerMoved { x: 1.0, y: 2.0 });
    assert_eq!(state.pointer_position(), Some([1.0, 2.0]));
}

#[test]
fn a_tap_reads_as_the_left_button() {
    let mut state = InputState::default();
    down(&mut state, 1, 0.0, 0.0);
    assert!(state.pointer_down(MouseButton::Left));
    assert!(state.pointer_pressed(MouseButton::Left));
    // And as nothing else: a finger is not a right-click.
    assert!(!state.pointer_down(MouseButton::Right));

    state.begin_frame(FRAME);
    assert!(state.pointer_down(MouseButton::Left), "still held");
    assert!(
        !state.pointer_pressed(MouseButton::Left),
        "pressed is an edge"
    );

    state.apply(InputEvent::TouchEnded { id: 1 });
    assert!(state.pointer_released(MouseButton::Left));
    assert!(!state.pointer_down(MouseButton::Left));
}

/// A second finger lifting while one is still down is not the pointer
/// coming up, any more than releasing the right mouse button releases the
/// left.
#[test]
fn the_pointer_comes_up_when_the_last_finger_does() {
    let mut state = InputState::default();
    down(&mut state, 1, 0.0, 0.0);
    down(&mut state, 2, 1.0, 1.0);
    state.begin_frame(FRAME);

    state.apply(InputEvent::TouchEnded { id: 2 });
    assert!(!state.pointer_released(MouseButton::Left));
    assert!(state.pointer_down(MouseButton::Left));

    state.apply(InputEvent::TouchEnded { id: 1 });
    assert!(state.pointer_released(MouseButton::Left));
}

/// The order fingers are reported in has to be the same order next frame,
/// or a drag would jump from one finger to another.
#[test]
fn fingers_keep_their_order_between_frames() {
    let mut state = InputState::default();
    down(&mut state, 9, 90.0, 0.0);
    down(&mut state, 2, 20.0, 0.0);
    assert_eq!(state.touch_at(0), Some([20.0, 0.0]));
    assert_eq!(state.touch_at(1), Some([90.0, 0.0]));

    state.begin_frame(FRAME);
    assert_eq!(state.touch_at(0), Some([20.0, 0.0]));
}

/// A finger cannot be reported as lifted once the window has stopped
/// hearing about it, so one still down would stay down for ever.
#[test]
fn losing_focus_lets_go_of_every_finger() {
    let mut state = InputState::default();
    down(&mut state, 1, 0.0, 0.0);
    state.apply(InputEvent::FocusChanged(false));
    assert_eq!(state.touch_count(), 0);
    assert!(!state.pointer_down(MouseButton::Left));
}

/// A move for a finger that never started is a host bug, and inventing the
/// touch would hide it behind a finger that never arrived.
#[test]
fn a_finger_that_never_started_does_not_appear_by_moving() {
    let mut state = InputState::default();
    state.apply(InputEvent::TouchMoved {
        id: 3,
        x: 1.0,
        y: 1.0,
    });
    assert_eq!(state.touch_count(), 0);
}

#[test]
fn a_host_reporting_more_fingers_than_anyone_has_is_bounded() {
    let mut state = InputState::default();
    for id in 0..64 {
        down(&mut state, id, 0.0, 0.0);
    }
    assert_eq!(state.touch_count(), super::state::TOUCH_LIMIT);
}
