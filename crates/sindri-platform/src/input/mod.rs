//! Input, from a host's event to what a frame of gameplay reads.
//!
//! One file per thing that has a name of its own — a key, a pointer button —
//! and one for the state they accumulate into. Hosts translate their native
//! events into [`InputEvent`]; nothing above this layer knows whether the
//! input came from `winit`, the DOM, or a test.

pub mod action;
mod button;
mod key;
mod presses;
mod state;

pub use action::{
    ACTIONS_FORMAT_VERSION, ACTIONS_SUFFIX, ActionId, ActionKind, ActionMap, ActionMapError,
    ActionState, Actions, ActionsDocumentError, Binding, Source,
};
pub use button::MouseButton;
pub use key::Key;
pub use state::{InputEvent, InputState};
