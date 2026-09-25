//! What the players' pads are doing.
//!
//! A pad is read through a player slot rather than a device: slot 1 is the
//! first pad a face button or Start was pressed on, slot 2 the next, and a
//! slot is given back when its pad is unplugged. Slot 0 reads every pad at
//! once, for a game with one player who could pick up any of them.
//!
//! A slot is a number because Decay has no integers; it is refused unless it
//! is a whole one.

use decay_semantic::{FunctionType, HostType, Type};

/// A question a script asks about the pads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GamepadQuery {
    /// The slot claimed this frame, or 0.
    Joined,
    /// The slot given back this frame, or 0.
    Left,
    /// How many slots are held.
    Count,
    /// Whether a pad holds this slot.
    IsConnected,
    /// A button, held.
    Down,
    /// A button, gone down this frame.
    Pressed,
    /// A button, come up this frame.
    Released,
    /// A stick or trigger, dead zone taken out.
    Axis,
}

pub(crate) const GAMEPAD_QUERIES: &[(&str, GamepadQuery)] = &[
    ("joined", GamepadQuery::Joined),
    ("left", GamepadQuery::Left),
    ("count", GamepadQuery::Count),
    ("is_connected", GamepadQuery::IsConnected),
    ("is_down", GamepadQuery::Down),
    ("just_pressed", GamepadQuery::Pressed),
    ("just_released", GamepadQuery::Released),
    ("axis", GamepadQuery::Axis),
];

impl GamepadQuery {
    /// What it takes and what it answers, which the analyzer checks and the
    /// host relies on.
    fn signature(self) -> FunctionType {
        let (params, return_type) = match self {
            Self::Joined | Self::Left | Self::Count => (vec![], Type::F32),
            Self::IsConnected => (vec![Type::F32], Type::Bool),
            Self::Down | Self::Pressed | Self::Released => {
                (vec![Type::F32, Type::String], Type::Bool)
            }
            Self::Axis => (vec![Type::F32, Type::String], Type::F32),
        };
        FunctionType {
            params,
            return_type,
        }
    }
}

/// The `Gamepad` type, as the analyzer sees it.
pub(crate) fn gamepad_type() -> HostType {
    GAMEPAD_QUERIES
        .iter()
        .fold(HostType::new(), |host, (name, query)| {
            host.with_function(*name, query.signature())
        })
}
