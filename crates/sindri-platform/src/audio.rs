use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

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

/// Headless backend used by tests and tools that deliberately make no sound.
///
/// It is not a no-op: every request is recorded, so an integration test can
/// prove a pickup asked for exactly one sound without CI needing a sound card.
#[derive(Clone, Debug, Default)]
pub struct SilentAudioBackend {
    clips: BTreeSet<String>,
    events: Vec<AudioEvent>,
    next_voice: u64,
}

impl SilentAudioBackend {
    #[must_use]
    pub fn events(&self) -> &[AudioEvent] {
        &self.events
    }

    pub fn take_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.events)
    }
}

impl AudioBackend for SilentAudioBackend {
    fn register(&mut self, clip: AudioClip) -> Result<(), AudioError> {
        self.clips.insert(clip.id.clone());
        self.events.push(AudioEvent::Registered(clip.id));
        Ok(())
    }

    fn play(&mut self, clip: &str, settings: PlaybackSettings) -> Result<AudioVoiceId, AudioError> {
        if !self.clips.contains(clip) {
            return Err(AudioError::MissingClip(clip.to_owned()));
        }
        let voice = AudioVoiceId(self.next_voice);
        self.next_voice = self.next_voice.wrapping_add(1);
        self.events.push(AudioEvent::Played {
            voice,
            clip: clip.to_owned(),
            settings,
        });
        Ok(voice)
    }

    fn stop(&mut self, voice: AudioVoiceId) {
        self.events.push(AudioEvent::Stopped(voice));
    }

    fn pause_all(&mut self) {
        self.events.push(AudioEvent::PausedAll);
    }

    fn resume_all(&mut self) {
        self.events.push(AudioEvent::ResumedAll);
    }

    fn stop_all(&mut self) {
        self.events.push(AudioEvent::StoppedAll);
    }

    fn unlock(&mut self) -> Result<(), AudioError> {
        self.events.push(AudioEvent::Unlocked);
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeAudioBackend {
    device: rodio::MixerDeviceSink,
    clips: BTreeMap<String, std::sync::Arc<[u8]>>,
    voices: BTreeMap<AudioVoiceId, rodio::Player>,
    next_voice: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeAudioBackend {
    pub fn new() -> Result<Self, AudioError> {
        let device = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| AudioError::Output(error.to_string()))?;
        Ok(Self {
            device,
            clips: BTreeMap::new(),
            voices: BTreeMap::new(),
            next_voice: 0,
        })
    }

    fn reap(&mut self) {
        self.voices.retain(|_, voice| !voice.empty());
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioBackend for NativeAudioBackend {
    fn register(&mut self, clip: AudioClip) -> Result<(), AudioError> {
        self.clips.insert(clip.id, clip.bytes.into());
        Ok(())
    }

    fn play(&mut self, clip: &str, settings: PlaybackSettings) -> Result<AudioVoiceId, AudioError> {
        use rodio::Source as _;
        use std::io::Cursor;

        self.reap();
        let bytes = self
            .clips
            .get(clip)
            .cloned()
            .ok_or_else(|| AudioError::MissingClip(clip.to_owned()))?;
        let source =
            rodio::Decoder::try_from(Cursor::new(bytes)).map_err(|error| AudioError::Decode {
                id: clip.to_owned(),
                message: error.to_string(),
            })?;
        let player = rodio::Player::connect_new(self.device.mixer());
        player.set_volume(settings.volume.clamp(0.0, 1.0));
        match settings.mode {
            PlaybackMode::Once => player.append(source),
            PlaybackMode::Loop => player.append(source.repeat_infinite()),
        }
        let voice = AudioVoiceId(self.next_voice);
        self.next_voice = self.next_voice.wrapping_add(1);
        self.voices.insert(voice, player);
        Ok(voice)
    }

    fn stop(&mut self, voice: AudioVoiceId) {
        if let Some(player) = self.voices.remove(&voice) {
            player.stop();
        }
    }

    fn pause_all(&mut self) {
        for voice in self.voices.values() {
            voice.pause();
        }
    }

    fn resume_all(&mut self) {
        for voice in self.voices.values() {
            voice.play();
        }
    }

    fn stop_all(&mut self) {
        for (_, voice) in std::mem::take(&mut self.voices) {
            voice.stop();
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub struct BrowserAudioBackend {
    clips: BTreeMap<String, String>,
    voices: BTreeMap<AudioVoiceId, web_sys::HtmlAudioElement>,
    next_voice: u64,
    unlocked: bool,
}

#[cfg(target_arch = "wasm32")]
impl BrowserAudioBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            clips: BTreeMap::new(),
            voices: BTreeMap::new(),
            next_voice: 0,
            unlocked: false,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for BrowserAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BrowserAudioBackend {
    fn drop(&mut self) {
        for url in self.clips.values() {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl AudioBackend for BrowserAudioBackend {
    fn register(&mut self, clip: AudioClip) -> Result<(), AudioError> {
        use wasm_bindgen::JsValue;

        let array = js_sys::Uint8Array::from(clip.bytes.as_slice());
        let parts = js_sys::Array::new();
        parts.push(&JsValue::from(array));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(clip.mime_type);
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(|error| {
                AudioError::Browser(format!("could not create audio blob: {error:?}"))
            })?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|error| {
            AudioError::Browser(format!("could not create audio URL: {error:?}"))
        })?;
        if let Some(previous) = self.clips.insert(clip.id, url) {
            let _ = web_sys::Url::revoke_object_url(&previous);
        }
        Ok(())
    }

    fn play(&mut self, clip: &str, settings: PlaybackSettings) -> Result<AudioVoiceId, AudioError> {
        if !self.unlocked {
            return Err(AudioError::Locked);
        }
        let url = self
            .clips
            .get(clip)
            .ok_or_else(|| AudioError::MissingClip(clip.to_owned()))?;
        let element = web_sys::HtmlAudioElement::new_with_src(url).map_err(|error| {
            AudioError::Browser(format!("could not create audio element: {error:?}"))
        })?;
        element.set_loop(settings.mode == PlaybackMode::Loop);
        element.set_volume(f64::from(settings.volume.clamp(0.0, 1.0)));
        let _playback = element
            .play()
            .map_err(|error| AudioError::Browser(format!("playback was rejected: {error:?}")))?;
        let voice = AudioVoiceId(self.next_voice);
        self.next_voice = self.next_voice.wrapping_add(1);
        self.voices.insert(voice, element);
        Ok(voice)
    }

    fn stop(&mut self, voice: AudioVoiceId) {
        if let Some(element) = self.voices.remove(&voice) {
            let _ = element.pause();
            element.set_current_time(0.0);
        }
    }

    fn pause_all(&mut self) {
        for voice in self.voices.values() {
            let _ = voice.pause();
        }
    }

    fn resume_all(&mut self) {
        if !self.unlocked {
            return;
        }
        for voice in self.voices.values() {
            let _ = voice.play();
        }
    }

    fn stop_all(&mut self) {
        for (_, voice) in std::mem::take(&mut self.voices) {
            let _ = voice.pause();
            voice.set_current_time(0.0);
        }
    }

    fn unlock(&mut self) -> Result<(), AudioError> {
        self.unlocked = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioBackend, AudioClip, AudioEvent, PlaybackMode, PlaybackSettings, SilentAudioBackend,
    };

    #[test]
    fn silent_backend_records_playback_without_a_device() {
        let mut audio = SilentAudioBackend::default();
        audio
            .register(AudioClip::new(
                "audio/pickup.wav",
                vec![1, 2, 3],
                "audio/wav",
            ))
            .expect("register");
        let voice = audio
            .play("audio/pickup.wav", PlaybackSettings::once(0.75))
            .expect("play");

        assert!(audio.events().iter().any(|event| matches!(
            event,
            AudioEvent::Played { voice: found, clip, settings }
                if *found == voice
                    && clip == "audio/pickup.wav"
                    && settings.mode == PlaybackMode::Once
                    && (settings.volume - 0.75).abs() < f32::EPSILON
        )));
    }

    #[test]
    fn missing_silent_clip_is_an_error() {
        let mut audio = SilentAudioBackend::default();
        assert!(audio.play("missing", PlaybackSettings::default()).is_err());
    }
}
