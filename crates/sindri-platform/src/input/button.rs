//! The pointer's buttons, and the names a binding refers to them by.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    /// Every button, so a host can enumerate them and a test can prove the
    /// name table covers the enum.
    pub const ALL: &'static [Self] = &[Self::Left, Self::Middle, Self::Right];

    /// The name a binding, a config file, or a script refers to this button by.
    ///
    /// Exhaustive on purpose, exactly as [`Key::name`] is: a new button cannot
    /// be added without being given a name here.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Middle => "Middle",
            Self::Right => "Right",
        }
    }

    /// The button a name refers to, or `None` when nothing does.
    ///
    /// Case-insensitive for the reason key names are: the name is typed by a
    /// person, and one that works in one casing and silently does nothing in
    /// the other is a bug report nobody can reproduce.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|button| button.name().eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::MouseButton;

    #[test]
    fn every_button_has_a_unique_name_and_comes_back_from_it() {
        let mut seen = std::collections::BTreeSet::new();
        for button in MouseButton::ALL {
            let name = button.name();
            assert!(seen.insert(name), "two buttons are both called `{name}`");
            assert_eq!(MouseButton::from_name(name), Some(*button));
        }
        assert_eq!(seen.len(), MouseButton::ALL.len());
        assert_eq!(MouseButton::from_name("left"), Some(MouseButton::Left));
        assert_eq!(MouseButton::from_name("Wheel"), None);
    }
}
