//! The physical things an action can be driven by, and how they combine.

use crate::input::{Key, MouseButton};

/// One physical input, named the way a binding file names it.
///
/// Deliberately small and closed. A source is a thing a host reports, not a
/// thing a game means -- "the W key" rather than "forward" -- and the whole
/// point of an action is that the two are written down separately.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Source {
    Key(Key),
    MouseButton(MouseButton),
    /// Where the pointer is, in viewport pixels.
    PointerX,
    PointerY,
    /// How far the pointer moved this frame.
    PointerDeltaX,
    PointerDeltaY,
    ScrollX,
    ScrollY,
}

impl Source {
    /// The name a binding file refers to this source by.
    ///
    /// Exhaustive on purpose, exactly as `Key::name` is: a new source cannot be
    /// added without being given a name a project can write.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Key(key) => format!("key.{}", key.name()),
            Self::MouseButton(button) => format!("mouse.{}", button.name()),
            Self::PointerX => "pointer.x".to_owned(),
            Self::PointerY => "pointer.y".to_owned(),
            Self::PointerDeltaX => "pointer.dx".to_owned(),
            Self::PointerDeltaY => "pointer.dy".to_owned(),
            Self::ScrollX => "scroll.x".to_owned(),
            Self::ScrollY => "scroll.y".to_owned(),
        }
    }

    /// The source a name refers to, or `None` if nothing does.
    ///
    /// `None` rather than a default, because a binding naming a source this
    /// build has never heard of is a mistake someone should be told about
    /// rather than a control that silently does nothing.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "pointer.x" => return Some(Self::PointerX),
            "pointer.y" => return Some(Self::PointerY),
            "pointer.dx" => return Some(Self::PointerDeltaX),
            "pointer.dy" => return Some(Self::PointerDeltaY),
            "scroll.x" => return Some(Self::ScrollX),
            "scroll.y" => return Some(Self::ScrollY),
            _ => {}
        }
        let (prefix, rest) = name.split_once('.')?;
        match prefix {
            "key" => Key::from_name(rest).map(Self::Key),
            "mouse" => MouseButton::from_name(rest).map(Self::MouseButton),
            _ => None,
        }
    }

    /// Whether this source is one that is pressed rather than measured.
    ///
    /// A key is down or it is not; a pointer's position is a number. The
    /// difference decides how a source reads as an axis: a held key is a full
    /// deflection, while an axis reports whatever it is sitting at.
    #[must_use]
    pub const fn is_digital(self) -> bool {
        matches!(self, Self::Key(_) | Self::MouseButton(_))
    }
}

/// How one or more sources make an action's value.
///
/// Composites are here rather than left to each game because a direction from
/// four keys is the single most re-written piece of input code there is, and
/// every game that writes it again picks its own answer to the two questions
/// that matter: what opposite keys held together mean, and whether a diagonal
/// is faster than a straight line.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Binding {
    /// One source on its own.
    Simple(Source),
    /// Two sources as the ends of one axis -- A and D, Left and Right.
    Axis { negative: Source, positive: Source },
    /// Four sources as a direction. WASD, or the arrow keys.
    Vector {
        up: Source,
        down: Source,
        left: Source,
        right: Source,
    },
}

impl Binding {
    /// Every source this binding reads, for conflict checking and for rebinding
    /// interfaces that want to show what a control is currently on.
    #[must_use]
    pub fn sources(&self) -> Vec<Source> {
        match self {
            Self::Simple(source) => vec![*source],
            Self::Axis { negative, positive } => vec![*negative, *positive],
            Self::Vector {
                up,
                down,
                left,
                right,
            } => vec![*up, *down, *left, *right],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Binding, Source};
    use crate::input::{Key, MouseButton};

    #[test]
    fn every_source_survives_being_written_down_and_read_back() {
        // A binding file is text. A source that cannot make the round trip is a
        // control that silently changes when a project is saved and reopened.
        let sources = [
            Source::Key(Key::W),
            Source::MouseButton(MouseButton::Left),
            Source::PointerX,
            Source::PointerY,
            Source::PointerDeltaX,
            Source::PointerDeltaY,
            Source::ScrollX,
            Source::ScrollY,
        ];
        for source in sources {
            let name = source.name();
            assert_eq!(
                Source::from_name(&name),
                Some(source),
                "{name} did not survive"
            );
        }
    }

    #[test]
    fn a_source_nobody_has_heard_of_is_refused_rather_than_defaulted() {
        assert_eq!(Source::from_name("key.Nonexistent"), None);
        assert_eq!(Source::from_name("gamepad.South"), None);
        assert_eq!(Source::from_name("pointer.z"), None);
        assert_eq!(Source::from_name("nonsense"), None);
    }

    #[test]
    fn a_key_is_pressed_and_a_pointer_is_measured() {
        assert!(Source::Key(Key::W).is_digital());
        assert!(Source::MouseButton(MouseButton::Left).is_digital());
        assert!(!Source::PointerX.is_digital());
        assert!(!Source::ScrollY.is_digital());
    }

    #[test]
    fn a_binding_can_say_everything_it_reads() {
        let wasd = Binding::Vector {
            up: Source::Key(Key::W),
            down: Source::Key(Key::S),
            left: Source::Key(Key::A),
            right: Source::Key(Key::D),
        };
        assert_eq!(wasd.sources().len(), 4);
        assert_eq!(
            Binding::Simple(Source::Key(Key::Space)).sources(),
            vec![Source::Key(Key::Space)]
        );
    }
}
