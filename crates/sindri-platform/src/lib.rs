//! The boundary between Sindri and whatever is hosting it.
//!
//! A host owns the window, the surface, and the event loop. This crate defines
//! what it must supply — a clock, input events, frame deltas, and audio output —
//! and the loop that turns those into gameplay calls. Nothing here knows about
//! a GPU, and platform-specific audio stays behind the same boundary as input.

mod audio;
mod clock;
mod host;
mod input;

pub use audio::{
    AudioBackend, AudioClip, AudioError, AudioEvent, AudioVoiceId, PlaybackMode, PlaybackSettings,
    SilentAudioBackend,
};
#[cfg(target_arch = "wasm32")]
pub use audio::BrowserAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
pub use audio::NativeAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
pub use clock::SystemClock;
pub use clock::{Clock, FrameTimer, ManualClock};
pub use host::{EngineHost, FrameContext, FramePhase, FrameTime, Game, HostError};
pub use input::{InputEvent, InputState, Key, MouseButton};

pub mod prelude {
    pub use crate::{
        AudioBackend, AudioClip, AudioError, AudioVoiceId, Clock, EngineHost, FrameContext,
        FrameTime, FrameTimer, Game, InputEvent, InputState, Key, ManualClock, MouseButton,
        PlaybackMode, PlaybackSettings, SilentAudioBackend,
    };
}
