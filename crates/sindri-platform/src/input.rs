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

impl Key {
    /// Every key, which is what lets a host enumerate them and a test prove the
    /// name table covers the enum.
    pub const ALL: &'static [Self] = &[
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
        Self::M,
        Self::N,
        Self::O,
        Self::P,
        Self::Q,
        Self::R,
        Self::S,
        Self::T,
        Self::U,
        Self::V,
        Self::W,
        Self::X,
        Self::Y,
        Self::Z,
        Self::Digit0,
        Self::Digit1,
        Self::Digit2,
        Self::Digit3,
        Self::Digit4,
        Self::Digit5,
        Self::Digit6,
        Self::Digit7,
        Self::Digit8,
        Self::Digit9,
        Self::ArrowLeft,
        Self::ArrowRight,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::Space,
        Self::Enter,
        Self::Escape,
        Self::Tab,
        Self::Backspace,
        Self::ShiftLeft,
        Self::ShiftRight,
        Self::ControlLeft,
        Self::ControlRight,
        Self::AltLeft,
        Self::AltRight,
    ];

    /// The name a binding, a config file, or a script refers to this key by.
    ///
    /// Physical names rather than characters, matching the enum: a key is
    /// identified by where it is, so a binding survives a change of layout.
    /// The match is exhaustive on purpose — a new key cannot be added without
    /// being given a name here.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Digit0 => "Digit0",
            Self::Digit1 => "Digit1",
            Self::Digit2 => "Digit2",
            Self::Digit3 => "Digit3",
            Self::Digit4 => "Digit4",
            Self::Digit5 => "Digit5",
            Self::Digit6 => "Digit6",
            Self::Digit7 => "Digit7",
            Self::Digit8 => "Digit8",
            Self::Digit9 => "Digit9",
            Self::ArrowLeft => "ArrowLeft",
            Self::ArrowRight => "ArrowRight",
            Self::ArrowUp => "ArrowUp",
            Self::ArrowDown => "ArrowDown",
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Escape => "Escape",
            Self::Tab => "Tab",
            Self::Backspace => "Backspace",
            Self::ShiftLeft => "ShiftLeft",
            Self::ShiftRight => "ShiftRight",
            Self::ControlLeft => "ControlLeft",
            Self::ControlRight => "ControlRight",
            Self::AltLeft => "AltLeft",
            Self::AltRight => "AltRight",
        }
    }

    /// The key a name refers to, or `None` when nothing does.
    ///
    /// Case-insensitive, because a name typed into a script or a config is
    /// typed by a person: `"space"` and `"Space"` are the same key, and a
    /// binding that works in one casing and silently does nothing in the other
    /// is a bug report nobody can reproduce.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|key| key.name().eq_ignore_ascii_case(name))
    }
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

#[cfg(test)]
mod name_tests {
    use super::Key;

    /// `ALL` and the enum must not drift. `name` is an exhaustive match, so a
    /// new key cannot be added without being named; this is the other half —
    /// it cannot be added without being listed either, or a host enumerating
    /// keys would silently offer one fewer than exists.
    #[test]
    fn every_key_has_a_unique_name_and_comes_back_from_it() {
        let mut seen = std::collections::BTreeSet::new();
        for key in Key::ALL {
            let name = key.name();
            assert!(seen.insert(name), "two keys are both called `{name}`");
            assert_eq!(Key::from_name(name), Some(*key));
        }
        assert_eq!(seen.len(), Key::ALL.len());
    }

    /// A name is typed by a person, so casing must not decide whether a
    /// binding works.
    #[test]
    fn a_name_is_matched_whatever_its_casing() {
        assert_eq!(Key::from_name("space"), Some(Key::Space));
        assert_eq!(Key::from_name("SPACE"), Some(Key::Space));
        assert_eq!(Key::from_name("arrowleft"), Some(Key::ArrowLeft));
        assert_eq!(Key::from_name("a"), Some(Key::A));
    }

    #[test]
    fn a_name_nothing_answers_to_is_none() {
        assert_eq!(Key::from_name("Joystick"), None);
        assert_eq!(Key::from_name(""), None);
    }
}
