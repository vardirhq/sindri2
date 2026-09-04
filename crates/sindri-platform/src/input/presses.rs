//! Turning a host's events into presses.
//!
//! Separate from the state they land beside because it is a different job:
//! [`InputState`](super::InputState) keeps a record of devices -- which keys
//! are down, where the mouse is -- while this reads the same events as
//! *interactions*, which is the shape anything asking "what did the person
//! just do" wants. See the header of `sindri_core::input` for why the second
//! shape has to exist.

use sindri_core::{PointerDevice, PressId, PressPhase, Presses};

use super::{InputEvent, MouseButton};

/// The identity of the press a mouse button makes.
///
/// Left is zero so that, among mouse presses, it is the one
/// [`Presses::primary`] picks: a right-drag should not take an interface's
/// attention away from a left one.
const fn press_id(button: MouseButton) -> PressId {
    let raw = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    PressId::new(PointerDevice::Mouse, raw)
}

/// The identity of the press a finger makes.
const fn touch_id(id: u64) -> PressId {
    PressId::new(PointerDevice::Touch, id)
}

/// Folds one host event into the presses.
///
/// `pointer` is where the mouse was known to be before this event, which is
/// what places a button press: a host reports the position and the button
/// separately, and a press has to happen somewhere.
pub(super) fn apply(presses: &mut Presses, event: InputEvent, pointer: Option<[f32; 2]>) {
    match event {
        InputEvent::ButtonPressed(button) => {
            // A host that has never said where the mouse is has not given us a
            // place to put this. Desktop hosts report a move before any click,
            // so this is a host bug rather than a case to invent a position
            // for -- and inventing one would put a click in the screen corner.
            if let Some(at) = pointer {
                presses.begin(press_id(button), at);
            }
        }
        InputEvent::ButtonReleased(button) => {
            presses.finish(press_id(button), PressPhase::Ended);
        }
        InputEvent::PointerMoved { x, y } => {
            presses.set_hover(Some([x, y]));
            for button in MouseButton::ALL {
                presses.move_to(press_id(*button), [x, y]);
            }
        }
        InputEvent::PointerLeft => {
            presses.set_hover(None);
            // Cancelled, not ended: the button was never reported as coming
            // up, and a press dragged out of the window and released outside
            // it is not a click on anything inside.
            for button in MouseButton::ALL {
                presses.finish(press_id(*button), PressPhase::Cancelled);
            }
        }
        InputEvent::TouchStarted { id, x, y } => presses.begin(touch_id(id), [x, y]),
        InputEvent::TouchMoved { id, x, y } => presses.move_to(touch_id(id), [x, y]),
        InputEvent::TouchEnded { id } => presses.finish(touch_id(id), PressPhase::Ended),
        InputEvent::FocusChanged(focused) => {
            if !focused {
                // For the reason `Presses::cancel_all` gives: a window lost
                // mid-drag is not a click.
                presses.cancel_all();
            }
        }
        InputEvent::KeyPressed(_) | InputEvent::KeyReleased(_) | InputEvent::Scrolled { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{apply, press_id, touch_id};
    use crate::{InputEvent, MouseButton};
    use sindri_core::{PressPhase, Presses};

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
    fn a_button_press_with_nowhere_to_be_is_dropped_rather_than_placed_at_the_origin() {
        let mut presses = Presses::default();
        apply(
            &mut presses,
            InputEvent::ButtonPressed(MouseButton::Left),
            None,
        );
        assert!(
            presses.is_empty(),
            "a press in the corner of the screen is worse than no press"
        );
    }

    #[test]
    fn the_left_button_outranks_the_others() {
        let mut presses = Presses::default();
        let at = Some([1.0, 1.0]);
        apply(
            &mut presses,
            InputEvent::ButtonPressed(MouseButton::Right),
            at,
        );
        apply(
            &mut presses,
            InputEvent::ButtonPressed(MouseButton::Left),
            at,
        );
        assert_eq!(
            presses.primary().map(sindri_core::Press::id),
            Some(press_id(MouseButton::Left))
        );
    }

    #[test]
    fn a_pointer_leaving_cancels_rather_than_completes() {
        let mut presses = Presses::default();
        apply(
            &mut presses,
            InputEvent::ButtonPressed(MouseButton::Left),
            Some([5.0, 5.0]),
        );
        apply(&mut presses, InputEvent::PointerLeft, None);
        assert_eq!(presses.ended().count(), 0);
        assert_eq!(
            presses
                .get(press_id(MouseButton::Left))
                .map(sindri_core::Press::phase),
            Some(PressPhase::Cancelled)
        );
    }

    #[test]
    fn a_finger_is_followed_by_its_own_identifier() {
        let mut presses = Presses::default();
        apply(
            &mut presses,
            InputEvent::TouchStarted {
                id: 3,
                x: 1.0,
                y: 2.0,
            },
            None,
        );
        apply(
            &mut presses,
            InputEvent::TouchMoved {
                id: 3,
                x: 4.0,
                y: 6.0,
            },
            None,
        );
        apply(
            &mut presses,
            InputEvent::TouchMoved {
                id: 9,
                x: 0.0,
                y: 0.0,
            },
            None,
        );

        let press = presses.get(touch_id(3)).expect("the finger that started");
        assert_at(press.position(), [4.0, 6.0]);
        assert_eq!(
            presses.live(),
            1,
            "a move for a finger that never began is not one"
        );
    }
}
