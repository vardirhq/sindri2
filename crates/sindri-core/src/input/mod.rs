//! What a person is doing with a pointing device, in terms both a host and a
//! game agree on.
//!
//! This is vocabulary, not machinery: no window, no events, no polling. It sits
//! here because the layer that *produces* it (`sindri-platform`, from a host's
//! events) and the layers that *read* it (`sindri-scene` for hit testing, a
//! game for gameplay) have no other crate in common. Without it each reader
//! invents its own shape and every producer writes the conversion again, which
//! is what happened: the same four-line mapping stood in three files.
//!
//! # Why a press rather than a pointer
//!
//! "Where is the pointer" is a question shaped like a mouse. A mouse is one
//! thing, it is always somewhere, and it stays where it was let go. None of
//! that is true of a finger: there may be several, they arrive and leave, and
//! the instant one lifts it is nowhere at all.
//!
//! Asked of a finger at the moment it lifts, that question has no honest
//! answer -- and the moment it lifts is exactly when a press completes, which
//! is when a button needs to know where the press ended in order to decide it
//! was pressed. Answering "nowhere" there meant every tap on every button in
//! this engine did nothing.
//!
//! So the unit here is a [`Press`]: one interaction, with an identity that
//! lasts, a place it began, and a place it is *now* -- defined on every frame
//! of its life, the frame it ends included. A press that has ended still says
//! where it ended, because that is the fact anyone asking needs. The earlier
//! bug cannot be written against this type: there is no state in which a live
//! or just-ended press has no position.

mod gesture;
mod press;
mod set;
mod stick;

#[cfg(test)]
mod gesture_tests;

pub use gesture::{Gesture, GestureLimits, Gestures};
pub use press::{PointerDevice, Press, PressId, PressPhase};
pub use set::Presses;
pub use stick::{StickSettings, VirtualStick};
