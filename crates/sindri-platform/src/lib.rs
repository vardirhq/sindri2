//! The boundary between Sindri and whatever is hosting it.
//!
//! A host owns the window, the surface, and the event loop. This crate defines
//! what it must supply — a clock, input events, and frame deltas — and the loop
//! that turns those into gameplay calls. Nothing here knows about `winit`, the
//! DOM, or a GPU, which is what lets the same game run on a desktop, in a
//! browser, and in a headless test.

mod clock;
mod host;
mod input;

#[cfg(not(target_arch = "wasm32"))]
pub use clock::SystemClock;
pub use clock::{Clock, FrameTimer, ManualClock};
pub use host::{EngineHost, FrameContext, FramePhase, FrameTime, Game, HostError};
pub use input::{InputEvent, InputState, Key, MouseButton};

pub mod prelude {
    pub use crate::{
        Clock, EngineHost, FrameContext, FrameTime, FrameTimer, Game, InputEvent, InputState, Key,
        ManualClock, MouseButton,
    };
}
