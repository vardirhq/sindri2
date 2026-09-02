//! Input, from a host's event to what a frame of gameplay reads.
//!
//! One file per thing that has a name of its own — a key, a pointer button —
//! and one for the state they accumulate into. Hosts translate their native
//! events into [`InputEvent`]; nothing above this layer knows whether the
//! input came from `winit`, the DOM, or a test.

mod button;
mod key;
mod state;

pub use button::MouseButton;
pub use key::Key;
pub use state::{InputEvent, InputState};
