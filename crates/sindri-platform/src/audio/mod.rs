//! Playing a sound, whatever is doing the playing.
//!
//! [`AudioBackend`] is the whole contract; one file per backend behind
//! it. A new platform is a file here and its impl, and nothing that
//! asks for a sound has to learn about it.

use thiserror::Error;

// The backends are gated where they are declared rather than inside each file,
// so a file is either compiled whole or not at all.
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod silent;

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserAudioBackend;
#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeAudioBackend;
pub use silent::SilentAudioBackend;

/// One decoded asset handed to the platform audio boundary.
///
/// The asset pipeline identifies the container; the device backend owns codec
/// decoding because native Rodio and browser media elements already implement
/// that work, including streaming compressed music without expanding it to PCM
/// in engine memory first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioClip {
    pub id: String,
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
}

impl AudioClip {
    #[must_use]
    pub fn new(id: impl Into<String>, bytes: Vec<u8>, mime_type: &'static str) -> Self {
        Self {
            id: id.into(),
            bytes,
            mime_type,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AudioVoiceId(u64);

impl AudioVoiceId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackMode {
    #[default]
    Once,
    Loop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackSettings {
    pub mode: PlaybackMode,
    pub volume: f32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            mode: PlaybackMode::Once,
            volume: 1.0,
        }
    }
}

impl PlaybackSettings {
    #[must_use]
    pub const fn once(volume: f32) -> Self {
        Self {
            mode: PlaybackMode::Once,
            volume,
        }
    }

    #[must_use]
    pub const fn looping(volume: f32) -> Self {
        Self {
            mode: PlaybackMode::Loop,
            volume,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AudioError {
    #[error("audio clip '{0}' has not been registered")]
    MissingClip(String),
    #[error("browser audio is locked until a keyboard or pointer interaction")]
    Locked,
    #[error("could not initialize audio output: {0}")]
    Output(String),
    #[error("could not decode audio clip '{id}': {message}")]
    Decode { id: String, message: String },
    #[error("browser audio failed: {0}")]
    Browser(String),
}

/// Device-independent audio operations available to gameplay.
///
/// Implementations own voices and encoded clip storage. The engine only names
/// sounds and lifecycle operations, which keeps headless tests free of an audio
/// device and prevents the core simulation crate from learning about CPAL or
/// browser APIs.
pub trait AudioBackend {
    fn register(&mut self, clip: AudioClip) -> Result<(), AudioError>;
    fn play(&mut self, clip: &str, settings: PlaybackSettings) -> Result<AudioVoiceId, AudioError>;
    fn stop(&mut self, voice: AudioVoiceId);
    fn pause_all(&mut self);
    fn resume_all(&mut self);
    fn stop_all(&mut self);

    /// Called from a real user gesture. Native and silent backends need no
    /// unlock; browsers do, and refusing playback before this is intentional.
    fn unlock(&mut self) -> Result<(), AudioError> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioEvent {
    Registered(String),
    Played {
        voice: AudioVoiceId,
        clip: String,
        settings: PlaybackSettings,
    },
    Stopped(AudioVoiceId),
    PausedAll,
    ResumedAll,
    StoppedAll,
    Unlocked,
}
