//! What a person means, separately from what they pressed.
//!
//! A game that reads `Key::W` has written the keyboard into its rules. It
//! cannot be rebound, it cannot be played on a pad, and the same intent --
//! "forward" -- is spelled differently in every file that needs it. An action
//! is the other half: gameplay asks for `move`, a binding says what `move` is
//! made of, and the two are changed independently.
//!
//! # What this is not
//!
//! It is not a callback system. Nothing here calls into gameplay; gameplay asks
//! what an action is worth this frame, and gets both the value and the edges.
//! A subscription model makes the order in which two systems hear about one
//! press a matter of registration order, which is the sort of thing that
//! becomes a bug you cannot reproduce.
//!
//! It is also not a string lookup at the point of use. Names exist because a
//! binding lives in a file, but an action is resolved once, when the map is
//! built, so a name that matches nothing is a fault at load rather than a
//! control that silently does nothing for ever. That difference is the whole
//! reason for the [`ActionId`] type.

mod binding;
mod map;
mod state;

pub use binding::{Binding, Source};
pub use map::{ActionId, ActionKind, ActionMap, ActionMapError};
pub use state::{ActionState, Actions};
