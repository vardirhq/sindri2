use std::collections::BTreeSet;

/// A physical key, identified by position rather than by the character it
/// produces, so bindings survive a change of keyboard layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// A single change reported by a platform host.
///
/// Hosts translate their native events into these; nothing above this layer
/// knows whether the input came from `winit`, the DOM, or a test.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum InputEvent {
    KeyPressed(Key),
    KeyReleased(Key),
    ButtonPressed(MouseButton),
    ButtonReleased(MouseButton),
    /// Pointer position in logical pixels, with the origin at the top left.
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerLeft,
    Scrolled {
        x: f32,
        y: f32,
    },
    /// Window focus gained or lost.
    FocusChanged(bool),
}

/// Accumulated input for the current frame.
///
/// Holds both level state (is this key down?) and edge state (was it pressed
/// this frame?). Edges are cleared once per frame by the host, so gameplay sees
/// each press exactly once regardless of how many events arrived.
#[derive(Clone, Debug)]
pub struct InputState {
    keys_held: BTreeSet<Key>,
    keys_pressed: BTreeSet<Key>,
    keys_released: BTreeSet<Key>,
    buttons_held: BTreeSet<MouseButton>,
    buttons_pressed: BTreeSet<MouseButton>,
    buttons_released: BTreeSet<MouseButton>,
    pointer: Option<[f32; 2]>,
    pointer_delta: [f32; 2],
    scroll_delta: [f32; 2],
    focused: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys_held: BTreeSet::new(),
            keys_pressed: BTreeSet::new(),
            keys_released: BTreeSet::new(),
            buttons_held: BTreeSet::new(),
            buttons_pressed: BTreeSet::new(),
            buttons_released: BTreeSet::new(),
            pointer: None,
            pointer_delta: [0.0, 0.0],
            scroll_delta: [0.0, 0.0],
            focused: true,
        }
    }
}

impl InputState {
    /// Folds one host event into the current frame.
    ///
    /// A repeated press of an already-held key is ignored, so operating-system
    /// key repeat cannot make `key_pressed` fire more than once per physical
    /// press.
    pub fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyPressed(key) => {
                if self.keys_held.insert(key) {
                    self.keys_pressed.insert(key);
                }
            }
            InputEvent::KeyReleased(key) => {
                if self.keys_held.remove(&key) {
                    self.keys_released.insert(key);
                }
            }
            InputEvent::ButtonPressed(button) => {
                if self.buttons_held.insert(button) {
                    self.buttons_pressed.insert(button);
                }
            }
            InputEvent::ButtonReleased(button) => {
                if self.buttons_held.remove(&button) {
                    self.buttons_released.insert(button);
                }
            }
            InputEvent::PointerMoved { x, y } => {
                if let Some([previous_x, previous_y]) = self.pointer {
                    self.pointer_delta[0] += x - previous_x;
                    self.pointer_delta[1] += y - previous_y;
                }
                self.pointer = Some([x, y]);
            }
            InputEvent::PointerLeft => self.pointer = None,
            InputEvent::Scrolled { x, y } => {
                self.scroll_delta[0] += x;
                self.scroll_delta[1] += y;
            }
            InputEvent::FocusChanged(focused) => {
                self.focused = focused;
                if !focused {
                    // Key-up events are not delivered while unfocused, so
                    // anything still held would stick down forever.
                    self.release_everything();
                }
            }
        }
    }

    /// Clears edge state and per-frame deltas, keeping what is still held.
    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.pointer_delta = [0.0, 0.0];
        self.scroll_delta = [0.0, 0.0];
    }

    fn release_everything(&mut self) {
        for key in std::mem::take(&mut self.keys_held) {
            self.keys_released.insert(key);
        }
        for button in std::mem::take(&mut self.buttons_held) {
            self.buttons_released.insert(button);
        }
    }

    pub fn key_down(&self, key: Key) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn key_released(&self, key: Key) -> bool {
        self.keys_released.contains(&key)
    }

    pub fn button_down(&self, button: MouseButton) -> bool {
        self.buttons_held.contains(&button)
    }

    pub fn button_pressed(&self, button: MouseButton) -> bool {
        self.buttons_pressed.contains(&button)
    }

    pub fn button_released(&self, button: MouseButton) -> bool {
        self.buttons_released.contains(&button)
    }

    /// A -1, 0, or 1 axis from two opposing keys.
    ///
    /// Holding both returns zero, which keeps opposed movement keys from
    /// cancelling into a direction that depends on event order.
    pub fn axis(&self, negative: Key, positive: Key) -> f32 {
        f32::from(self.key_down(positive)) - f32::from(self.key_down(negative))
    }

    pub const fn pointer(&self) -> Option<[f32; 2]> {
        self.pointer
    }

    pub const fn pointer_delta(&self) -> [f32; 2] {
        self.pointer_delta
    }

    pub const fn scroll_delta(&self) -> [f32; 2] {
        self.scroll_delta
    }

    pub const fn is_focused(&self) -> bool {
        self.focused
    }
}
