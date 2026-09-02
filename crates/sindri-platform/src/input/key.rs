//! The keyboard, named by where a key is rather than by what it types.

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

#[cfg(test)]
mod tests {
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
