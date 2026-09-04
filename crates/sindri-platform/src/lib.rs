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
mod saves;

#[cfg(target_arch = "wasm32")]
pub use audio::BrowserAudioBackend;
#[cfg(all(not(target_arch = "wasm32"), feature = "audio"))]
pub use audio::NativeAudioBackend;
pub use audio::{
    AudioBackend, AudioClip, AudioError, AudioEvent, AudioVoiceId, PlaybackMode, PlaybackSettings,
    SilentAudioBackend,
};
#[cfg(not(target_arch = "wasm32"))]
pub use clock::SystemClock;
pub use clock::{Clock, FrameTimer, ManualClock};
pub use host::{EngineHost, FrameContext, FramePhase, FrameTime, Game, HostError};
pub use input::{
    ActionId, ActionKind, ActionMap, ActionMapError, ActionState, Actions, Binding, InputEvent,
    InputState, Key, MouseButton, Source,
};
#[cfg(target_arch = "wasm32")]
pub use saves::BrowserSaves;
#[cfg(not(target_arch = "wasm32"))]
pub use saves::FileSaves;
pub use saves::{DamagedSaves, MemorySaves, SaveBackend, SaveWriteError};

pub mod prelude {
    pub use crate::{
        AudioBackend, AudioClip, AudioError, AudioVoiceId, Clock, EngineHost, FrameContext,
        FrameTime, FrameTimer, Game, InputEvent, InputState, Key, ManualClock, MouseButton,
        PlaybackMode, PlaybackSettings, SilentAudioBackend,
    };
}
