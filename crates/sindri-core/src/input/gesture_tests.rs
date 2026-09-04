//! What the recogniser will and will not claim.
//!
//! Kept beside the recogniser rather than inside it because the two together
//! run past the file-size cap, and what is being checked here is a policy --
//! which press counts as which gesture -- that reads better as a list.

use super::gesture::{Gesture, GestureLimits, Gestures};
use super::press::{PointerDevice, PressId, PressPhase};
use super::set::Presses;
use std::time::Duration;

const FRAME: Duration = Duration::from_millis(16);

fn finger(raw: u64) -> PressId {
    PressId::new(PointerDevice::Touch, raw)
}

/// Positions compare by nearness: they are arrived at by arithmetic, and the
/// repository lints exact float comparison for the reason arithmetic does not
/// promise exact answers.
#[track_caller]
fn assert_at(actual: [f32; 2], expected: [f32; 2]) {
    assert!(
        (actual[0] - expected[0]).abs() < 1.0e-5 && (actual[1] - expected[1]).abs() < 1.0e-5,
        "expected {expected:?}, got {actual:?}"
    );
}

#[track_caller]
fn assert_somewhere(actual: Option<[f32; 2]>, expected: [f32; 2]) {
    assert_at(actual.expect("a gesture"), expected);
}

/// A recogniser and the presses it reads, driven a frame at a time.
struct Hand {
    presses: Presses,
    gestures: Gestures,
}

impl Hand {
    fn new() -> Self {
        Self {
            presses: Presses::default(),
            gestures: Gestures::new(GestureLimits::default()),
        }
    }

    fn press(&mut self, id: PressId, at: [f32; 2]) -> &mut Self {
        self.presses.begin(id, at);
        self.gestures.update(&self.presses);
        self
    }

    fn move_to(&mut self, id: PressId, at: [f32; 2]) -> &mut Self {
        self.presses.move_to(id, at);
        self.gestures.update(&self.presses);
        self
    }

    fn release(&mut self, id: PressId) -> &mut Self {
        self.presses.finish(id, PressPhase::Ended);
        self.gestures.update(&self.presses);
        self
    }

    fn cancel(&mut self) -> &mut Self {
        self.presses.cancel_all();
        self.gestures.update(&self.presses);
        self
    }

    /// Holds everything still for a while, a frame at a time.
    fn wait(&mut self, how_long: Duration) -> &mut Self {
        let frames = how_long.as_millis() / FRAME.as_millis();
        for _ in 0..frames {
            self.presses.advance(FRAME);
            self.gestures.update(&self.presses);
        }
        self
    }

    fn frame(&mut self) -> &mut Self {
        self.wait(FRAME)
    }
}

#[test]
fn a_press_that_stays_still_and_leaves_is_a_tap() {
    let mut hand = Hand::new();
    hand.press(finger(1), [10.0, 10.0])
        .frame()
        .release(finger(1));
    assert_somewhere(hand.gestures.tap(), [10.0, 10.0]);
}

#[test]
fn a_finger_never_lands_perfectly_still_and_a_tap_allows_for_it() {
    // Demanding no movement at all would fail for every real hand.
    let mut hand = Hand::new();
    hand.press(finger(1), [10.0, 10.0])
        .frame()
        .move_to(finger(1), [12.0, 13.0])
        .release(finger(1));
    assert_somewhere(hand.gestures.tap(), [12.0, 13.0]);
}

#[test]
fn a_press_that_wandered_is_a_drag_and_never_a_tap_again() {
    let mut hand = Hand::new();
    hand.press(finger(1), [0.0, 0.0])
        .frame()
        .move_to(finger(1), [40.0, 0.0]);
    assert_somewhere(hand.gestures.drag(), [40.0, 0.0]);

    // Back to where it started and let go: still not a tap. It moved.
    hand.frame()
        .move_to(finger(1), [0.0, 0.0])
        .frame()
        .release(finger(1));
    assert_eq!(hand.gestures.tap(), None);
}

#[test]
fn a_drag_reports_the_frames_movement_and_the_whole_journey() {
    let mut hand = Hand::new();
    hand.press(finger(1), [0.0, 0.0])
        .frame()
        .move_to(finger(1), [30.0, 0.0])
        .frame()
        .move_to(finger(1), [50.0, 0.0]);

    let drag = hand
        .gestures
        .iter()
        .find_map(|gesture| match gesture {
            Gesture::Drag { from, to, delta } => Some((*from, *to, *delta)),
            _ => None,
        })
        .expect("a drag");
    assert_at(drag.0, [0.0, 0.0]);
    assert_at(drag.1, [50.0, 0.0]);
    assert_at(drag.2, [20.0, 0.0]);
}

#[test]
fn a_still_press_becomes_a_long_press_once_and_then_stops_saying_so() {
    let mut hand = Hand::new();
    hand.press(finger(1), [5.0, 5.0])
        .wait(Duration::from_millis(512));
    assert_somewhere(hand.gestures.long_press(), [5.0, 5.0]);

    hand.frame();
    assert_eq!(
        hand.gestures.long_press(),
        None,
        "reported once, not every frame after"
    );
}

#[test]
fn a_long_press_that_is_let_go_is_not_also_a_tap() {
    let mut hand = Hand::new();
    hand.press(finger(1), [5.0, 5.0])
        .wait(Duration::from_millis(512))
        .release(finger(1));
    assert_eq!(hand.gestures.tap(), None);
}

#[test]
fn a_press_held_too_long_is_not_a_tap_even_without_a_long_press() {
    // Between the two limits: too slow to be a tap, and the long press was
    // never claimed because this recogniser was given a longer one.
    let mut hand = Hand::new();
    hand.gestures = Gestures::new(GestureLimits {
        tap: Duration::from_millis(100),
        long_press: Duration::from_secs(10),
        ..GestureLimits::default()
    });
    hand.press(finger(1), [5.0, 5.0])
        .wait(Duration::from_millis(320))
        .release(finger(1));
    assert_eq!(hand.gestures.tap(), None);
}

#[test]
fn a_press_taken_away_is_no_gesture_at_all() {
    let mut hand = Hand::new();
    hand.press(finger(1), [5.0, 5.0]).frame().cancel();
    assert_eq!(hand.gestures.tap(), None);
    assert_eq!(hand.gestures.iter().count(), 0);
}

#[test]
fn two_fingers_moving_apart_are_a_pinch_outward() {
    let mut hand = Hand::new();
    hand.press(finger(1), [0.0, 0.0])
        .press(finger(2), [10.0, 0.0]);
    // The first frame of a pair has nothing to compare against.
    assert_eq!(hand.gestures.pinch(), None);

    hand.frame().move_to(finger(2), [20.0, 0.0]);
    let scale = hand.gestures.pinch().expect("a pinch");
    assert!(
        (scale - 2.0).abs() < 1.0e-5,
        "twice as far apart, so twice: {scale}"
    );
}

#[test]
fn a_pinch_reports_the_point_between_the_fingers() {
    let mut hand = Hand::new();
    hand.press(finger(1), [0.0, 0.0])
        .press(finger(2), [10.0, 0.0])
        .frame()
        .move_to(finger(2), [30.0, 0.0]);

    let centre = hand
        .gestures
        .iter()
        .find_map(|gesture| match gesture {
            Gesture::Pinch { centre, .. } => Some(*centre),
            _ => None,
        })
        .expect("a pinch");
    assert!((centre[0] - 15.0).abs() < 1.0e-5, "centre was {centre:?}");
}

#[test]
fn one_finger_is_not_a_pinch_and_neither_are_three() {
    let mut hand = Hand::new();
    hand.press(finger(1), [0.0, 0.0]).frame();
    assert_eq!(hand.gestures.pinch(), None, "one finger");

    hand.press(finger(2), [10.0, 0.0])
        .frame()
        .press(finger(3), [20.0, 0.0])
        .frame()
        .move_to(finger(3), [40.0, 0.0]);
    assert_eq!(
        hand.gestures.pinch(),
        None,
        "three fingers is not a pinch either"
    );
}

#[test]
fn what_was_recognised_does_not_survive_the_frame() {
    let mut hand = Hand::new();
    hand.press(finger(1), [1.0, 1.0]).frame().release(finger(1));
    assert!(hand.gestures.tap().is_some());
    hand.frame();
    assert_eq!(hand.gestures.tap(), None, "a tap is news, not a state");
}
