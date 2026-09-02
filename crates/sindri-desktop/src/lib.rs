//! The `winit` host: a window, an event loop, and the frame that runs in it.
//!
//! This is the only place in the engine that knows what a `winit` key or window
//! is. Everything above it — gameplay, the host loop, tests — sees
//! [`sindri_platform::InputEvent`], so the same game runs unchanged against a
//! browser adapter or a scripted test.
//!
//! Despite the name, this crate serves the browser too. `winit` presents a
//! canvas through the same event loop it presents a desktop window through, so
//! splitting the two would duplicate a host to change six lines of it. What is
//! genuinely target-specific — attaching a canvas, how a future is spawned,
//! where time comes from — is confined to this crate rather than pushed out to
//! every application.

mod clock;
mod host;

pub use clock::WindowClock;
pub use host::{AppContext, DesktopApp, DesktopError, Flow, WindowConfig, run};

use sindri_platform::{InputEvent, Key, MouseButton};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseScrollDelta, TouchPhase, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

/// Logical pixels one line of wheel scroll is worth.
///
/// Wheels report lines and touchpads report pixels; normalising here means
/// gameplay sees one unit regardless of the device.
pub const PIXELS_PER_SCROLL_LINE: f32 = 50.0;

/// Converts a window event into an input event, if it carries one.
///
/// `scale_factor` is the window's DPI scale, used to report pointer positions
/// in logical pixels so behaviour matches across displays.
///
/// This is a thin shim: `winit`'s `KeyEvent` cannot be constructed off a real
/// window, so every decision it makes lives in the functions below, which are
/// directly tested.
pub fn input_event(event: &WindowEvent, scale_factor: f64) -> Option<InputEvent> {
    match event {
        WindowEvent::KeyboardInput { event, .. } => {
            keyboard_input(event.physical_key, event.state, event.repeat)
        }
        WindowEvent::MouseInput { state, button, .. } => mouse_input(*state, *button),
        WindowEvent::CursorMoved { position, .. } => Some(pointer_moved(*position, scale_factor)),
        WindowEvent::CursorLeft { .. } => Some(InputEvent::PointerLeft),
        WindowEvent::MouseWheel { delta, .. } => Some(scrolled(*delta, scale_factor)),
        WindowEvent::Touch(touch) => {
            Some(touched(touch.phase, touch.id, touch.location, scale_factor))
        }
        WindowEvent::Focused(focused) => Some(InputEvent::FocusChanged(*focused)),
        _ => None,
    }
}

/// Translates a key press or release.
///
/// Operating-system key repeat is dropped: a held key produces one press, and
/// gameplay decides for itself whether to act on the hold.
pub fn keyboard_input(
    physical: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<InputEvent> {
    if repeat {
        return None;
    }
    let key = key(physical)?;
    Some(match state {
        ElementState::Pressed => InputEvent::KeyPressed(key),
        ElementState::Released => InputEvent::KeyReleased(key),
    })
}

pub fn mouse_input(state: ElementState, button: winit::event::MouseButton) -> Option<InputEvent> {
    let button = mouse_button(button)?;
    Some(match state {
        ElementState::Pressed => InputEvent::ButtonPressed(button),
        ElementState::Released => InputEvent::ButtonReleased(button),
    })
}

pub fn pointer_moved(position: PhysicalPosition<f64>, scale_factor: f64) -> InputEvent {
    let logical = position.to_logical::<f32>(scale_factor);
    InputEvent::PointerMoved {
        x: logical.x,
        y: logical.y,
    }
}

/// Translates one finger's arrival, movement, or departure.
///
/// A cancelled touch is an ended one: the finger is gone either way, and a
/// game that had to tell them apart would be a game that leaves a finger down
/// when the system takes over the gesture.
/// Takes the parts rather than the `Touch`, for the reason the module header
/// gives about `KeyEvent`: a `winit` event carrying a `DeviceId` cannot be
/// constructed off a real window, so the decision lives somewhere a test can
/// reach it.
pub fn touched(
    phase: TouchPhase,
    id: u64,
    location: PhysicalPosition<f64>,
    scale_factor: f64,
) -> InputEvent {
    let logical = location.to_logical::<f32>(scale_factor);
    match phase {
        TouchPhase::Started => InputEvent::TouchStarted {
            id,
            x: logical.x,
            y: logical.y,
        },
        TouchPhase::Moved => InputEvent::TouchMoved {
            id,
            x: logical.x,
            y: logical.y,
        },
        TouchPhase::Ended | TouchPhase::Cancelled => InputEvent::TouchEnded { id },
    }
}

/// Normalises wheel lines and touchpad pixels to logical pixels.
pub fn scrolled(delta: MouseScrollDelta, scale_factor: f64) -> InputEvent {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => InputEvent::Scrolled {
            x: x * PIXELS_PER_SCROLL_LINE,
            y: y * PIXELS_PER_SCROLL_LINE,
        },
        MouseScrollDelta::PixelDelta(position) => {
            let logical = position.to_logical::<f32>(scale_factor);
            InputEvent::Scrolled {
                x: logical.x,
                y: logical.y,
            }
        }
    }
}

/// Maps a physical key position to Sindri's key set.
///
/// Returns `None` for keys the engine does not model yet, so an unmapped key is
/// ignored rather than mistaken for another one.
pub fn key(physical: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    Some(match code {
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::Digit0 => Key::Digit0,
        KeyCode::Digit1 => Key::Digit1,
        KeyCode::Digit2 => Key::Digit2,
        KeyCode::Digit3 => Key::Digit3,
        KeyCode::Digit4 => Key::Digit4,
        KeyCode::Digit5 => Key::Digit5,
        KeyCode::Digit6 => Key::Digit6,
        KeyCode::Digit7 => Key::Digit7,
        KeyCode::Digit8 => Key::Digit8,
        KeyCode::Digit9 => Key::Digit9,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::Space => Key::Space,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::ShiftLeft => Key::ShiftLeft,
        KeyCode::ShiftRight => Key::ShiftRight,
        KeyCode::ControlLeft => Key::ControlLeft,
        KeyCode::ControlRight => Key::ControlRight,
        KeyCode::AltLeft => Key::AltLeft,
        KeyCode::AltRight => Key::AltRight,
        _ => return None,
    })
}

pub fn mouse_button(button: winit::event::MouseButton) -> Option<MouseButton> {
    Some(match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Right => MouseButton::Right,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;
    use winit::event::TouchPhase;

    use super::*;

    fn press(code: KeyCode) -> Option<InputEvent> {
        keyboard_input(PhysicalKey::Code(code), ElementState::Pressed, false)
    }

    #[test]
    fn key_presses_and_releases_translate() {
        assert_eq!(
            press(KeyCode::ArrowRight),
            Some(InputEvent::KeyPressed(Key::ArrowRight))
        );
        assert_eq!(
            keyboard_input(
                PhysicalKey::Code(KeyCode::KeyW),
                ElementState::Released,
                false
            ),
            Some(InputEvent::KeyReleased(Key::W))
        );
    }

    #[test]
    fn operating_system_key_repeat_is_dropped() {
        assert_eq!(
            keyboard_input(
                PhysicalKey::Code(KeyCode::KeyW),
                ElementState::Pressed,
                true
            ),
            None
        );
    }

    #[test]
    fn unmapped_keys_are_ignored_rather_than_guessed_at() {
        assert_eq!(key(PhysicalKey::Code(KeyCode::F13)), None);
        assert_eq!(press(KeyCode::F13), None);
    }

    #[test]
    fn both_enter_keys_map_to_one_key() {
        assert_eq!(key(PhysicalKey::Code(KeyCode::Enter)), Some(Key::Enter));
        assert_eq!(
            key(PhysicalKey::Code(KeyCode::NumpadEnter)),
            Some(Key::Enter)
        );
    }

    #[test]
    fn every_mapped_key_is_distinct() {
        let codes = [
            KeyCode::KeyA,
            KeyCode::KeyZ,
            KeyCode::Digit0,
            KeyCode::Digit9,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::Space,
            KeyCode::Escape,
            KeyCode::Tab,
            KeyCode::Backspace,
            KeyCode::ShiftLeft,
            KeyCode::ShiftRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
        ];
        let mut mapped: Vec<Key> = codes
            .iter()
            .map(|code| key(PhysicalKey::Code(*code)).expect("code is mapped"))
            .collect();
        let total = mapped.len();
        mapped.sort_unstable();
        mapped.dedup();
        assert_eq!(mapped.len(), total, "two key codes collapsed onto one key");
    }

    #[test]
    fn mouse_buttons_translate_and_extras_are_ignored() {
        assert_eq!(
            mouse_input(ElementState::Pressed, winit::event::MouseButton::Left),
            Some(InputEvent::ButtonPressed(MouseButton::Left))
        );
        assert_eq!(
            mouse_input(ElementState::Released, winit::event::MouseButton::Right),
            Some(InputEvent::ButtonReleased(MouseButton::Right))
        );
        assert_eq!(
            mouse_input(ElementState::Pressed, winit::event::MouseButton::Other(9)),
            None
        );
    }

    #[test]
    fn pointer_positions_are_reported_in_logical_pixels() {
        assert_eq!(
            pointer_moved(PhysicalPosition::new(200.0, 100.0), 2.0),
            InputEvent::PointerMoved { x: 100.0, y: 50.0 }
        );
    }

    #[test]
    fn wheel_lines_and_touchpad_pixels_normalise_to_one_unit() {
        assert_eq!(
            scrolled(MouseScrollDelta::LineDelta(0.0, 1.0), 1.0),
            InputEvent::Scrolled {
                x: 0.0,
                y: PIXELS_PER_SCROLL_LINE
            }
        );
        assert_eq!(
            scrolled(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 100.0)),
                2.0
            ),
            InputEvent::Scrolled { x: 0.0, y: 50.0 }
        );
    }

    #[test]
    fn focus_and_pointer_exit_translate_from_window_events() {
        assert_eq!(
            input_event(&WindowEvent::Focused(false), 1.0),
            Some(InputEvent::FocusChanged(false))
        );
        assert_eq!(input_event(&WindowEvent::RedrawRequested, 1.0), None);
        assert_eq!(input_event(&WindowEvent::CloseRequested, 1.0), None);
    }

    #[test]
    fn a_finger_is_reported_in_logical_pixels() {
        assert_eq!(
            touched(
                TouchPhase::Started,
                3,
                PhysicalPosition::new(200.0, 100.0),
                2.0
            ),
            InputEvent::TouchStarted {
                id: 3,
                x: 100.0,
                y: 50.0
            }
        );
    }

    /// A finger is gone either way, and a game that had to tell the two apart
    /// would be a game that leaves one down when the system takes the gesture.
    #[test]
    fn a_cancelled_finger_is_an_ended_one() {
        for phase in [TouchPhase::Ended, TouchPhase::Cancelled] {
            assert_eq!(
                touched(phase, 1, PhysicalPosition::new(0.0, 0.0), 1.0),
                InputEvent::TouchEnded { id: 1 }
            );
        }
    }
}
