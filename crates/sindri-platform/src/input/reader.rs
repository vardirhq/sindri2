//! Reading real pads, and turning what they did into [`InputEvent`]s.
//!
//! A window system says nothing about pads: neither `winit` nor a browser's
//! key and pointer events carry them. So a host asks this once a frame, and
//! gets the same kind of event it already hands its `InputState`, which is all
//! anything above has to know about where a pad's press came from.

use super::InputEvent;
#[cfg(feature = "gamepad")]
use super::PadId;

/// The pads the machine has, asked once a frame.
///
/// Built without the `gamepad` feature, or where pads cannot be read at all, it
/// reports nothing, which is a machine with no pads plugged in: a game reading
/// slots sees nobody join rather than failing to start.
pub struct GamepadReader {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    /// Whether the pads present at start have been announced.
    announced: bool,
}

impl std::fmt::Debug for GamepadReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GamepadReader")
            .field("available", &self.is_available())
            .finish()
    }
}

impl Default for GamepadReader {
    fn default() -> Self {
        Self::new()
    }
}

impl GamepadReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "gamepad")]
            gilrs: gilrs::Gilrs::new().ok(),
            announced: false,
        }
    }

    /// Whether pads can be read here at all.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        #[cfg(feature = "gamepad")]
        {
            self.gilrs.is_some()
        }
        #[cfg(not(feature = "gamepad"))]
        {
            false
        }
    }

    /// Everything the pads did since the last call, in order.
    pub fn poll(&mut self, mut emit: impl FnMut(InputEvent)) {
        let announce = !std::mem::replace(&mut self.announced, true);
        #[cfg(feature = "gamepad")]
        if let Some(gilrs) = &mut self.gilrs {
            // A pad already plugged in when the game started never reports
            // arriving, so the first poll says so for it.
            if announce {
                for (id, _) in gilrs.gamepads() {
                    emit(InputEvent::GamepadConnected(pad_id(id)));
                }
            }
            while let Some(event) = gilrs.next_event() {
                if let Some(raw) = Raw::of(event.event) {
                    translate(pad_id(event.id), raw, &mut emit);
                }
            }
        }
        let _ = (announce, &mut emit);
    }
}

#[cfg(feature = "gamepad")]
fn pad_id(id: gilrs::GamepadId) -> PadId {
    PadId(u32::try_from(usize::from(id)).unwrap_or(u32::MAX))
}

/// What a pad did, with what the library carries and the engine does not
/// read taken off.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug)]
enum Raw {
    Connected,
    Disconnected,
    Pressed(gilrs::Button),
    Released(gilrs::Button),
    Changed(gilrs::Button, f32),
    Axis(gilrs::Axis, f32),
}

#[cfg(feature = "gamepad")]
impl Raw {
    const fn of(event: gilrs::EventType) -> Option<Self> {
        use gilrs::EventType;

        Some(match event {
            EventType::Connected => Self::Connected,
            EventType::Disconnected => Self::Disconnected,
            EventType::ButtonPressed(button, _) => Self::Pressed(button),
            EventType::ButtonReleased(button, _) => Self::Released(button),
            EventType::ButtonChanged(button, value, _) => Self::Changed(button, value),
            EventType::AxisChanged(axis, value, _) => Self::Axis(axis, value),
            _ => return None,
        })
    }
}

#[cfg(feature = "gamepad")]
fn translate(pad: PadId, raw: Raw, emit: &mut impl FnMut(InputEvent)) {
    use super::GamepadAxis;

    match raw {
        Raw::Connected => emit(InputEvent::GamepadConnected(pad)),
        Raw::Disconnected => emit(InputEvent::GamepadDisconnected(pad)),
        Raw::Pressed(button) => {
            if let Some(button) = button_of(button) {
                emit(InputEvent::GamepadPressed { pad, button });
            }
        }
        Raw::Released(button) => {
            if let Some(button) = button_of(button) {
                emit(InputEvent::GamepadReleased { pad, button });
            }
        }
        // A trigger is a button that also says how far it is pulled.
        Raw::Changed(button, value) => {
            let axis = match button {
                gilrs::Button::LeftTrigger2 => GamepadAxis::LeftTrigger,
                gilrs::Button::RightTrigger2 => GamepadAxis::RightTrigger,
                _ => return,
            };
            emit(InputEvent::GamepadAxisMoved { pad, axis, value });
        }
        Raw::Axis(axis, value) => {
            // The library measures up as positive; the engine's screen axes
            // measure down.
            let (axis, value) = match axis {
                gilrs::Axis::LeftStickX => (GamepadAxis::LeftX, value),
                gilrs::Axis::LeftStickY => (GamepadAxis::LeftY, -value),
                gilrs::Axis::RightStickX => (GamepadAxis::RightX, value),
                gilrs::Axis::RightStickY => (GamepadAxis::RightY, -value),
                _ => return,
            };
            emit(InputEvent::GamepadAxisMoved { pad, axis, value });
        }
    }
}

#[cfg(feature = "gamepad")]
const fn button_of(button: gilrs::Button) -> Option<super::GamepadButton> {
    use super::GamepadButton;
    use gilrs::Button;

    Some(match button {
        Button::South => GamepadButton::South,
        Button::East => GamepadButton::East,
        Button::West => GamepadButton::West,
        Button::North => GamepadButton::North,
        // The library's first trigger is the one a pad's shoulder carries.
        Button::LeftTrigger => GamepadButton::LeftBumper,
        Button::RightTrigger => GamepadButton::RightBumper,
        Button::LeftTrigger2 => GamepadButton::LeftTrigger,
        Button::RightTrigger2 => GamepadButton::RightTrigger,
        Button::Select => GamepadButton::Select,
        Button::Start => GamepadButton::Start,
        Button::LeftThumb => GamepadButton::LeftStick,
        Button::RightThumb => GamepadButton::RightStick,
        Button::DPadUp => GamepadButton::DPadUp,
        Button::DPadDown => GamepadButton::DPadDown,
        Button::DPadLeft => GamepadButton::DPadLeft,
        Button::DPadRight => GamepadButton::DPadRight,
        _ => return None,
    })
}

#[cfg(all(test, feature = "gamepad"))]
mod tests {
    use super::{InputEvent, PadId, Raw, translate};
    use crate::input::{GamepadAxis, GamepadButton};

    fn events(raw: Raw) -> Vec<InputEvent> {
        let mut out = Vec::new();
        translate(PadId(1), raw, &mut |event| out.push(event));
        out
    }

    #[test]
    fn a_stick_pushed_up_reads_negative_in_screen_axes() {
        assert_eq!(
            events(Raw::Axis(gilrs::Axis::LeftStickY, 1.0)),
            vec![InputEvent::GamepadAxisMoved {
                pad: PadId(1),
                axis: GamepadAxis::LeftY,
                value: -1.0,
            }]
        );
    }

    #[test]
    fn the_shoulder_is_a_bumper_and_the_second_trigger_is_a_trigger() {
        assert_eq!(
            events(Raw::Pressed(gilrs::Button::LeftTrigger)),
            vec![InputEvent::GamepadPressed {
                pad: PadId(1),
                button: GamepadButton::LeftBumper,
            }]
        );
        assert_eq!(
            events(Raw::Changed(gilrs::Button::RightTrigger2, 0.5)),
            vec![InputEvent::GamepadAxisMoved {
                pad: PadId(1),
                axis: GamepadAxis::RightTrigger,
                value: 0.5,
            }]
        );
        assert!(events(Raw::Pressed(gilrs::Button::Mode)).is_empty());
        assert!(events(Raw::Changed(gilrs::Button::South, 1.0)).is_empty());
    }
}
