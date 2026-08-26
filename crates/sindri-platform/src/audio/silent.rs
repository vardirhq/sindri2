//! The backend for a machine with no audio device, and for tests.

use std::collections::BTreeSet;

use super::{AudioBackend, AudioClip, AudioError, AudioEvent, AudioVoiceId, PlaybackSettings};

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
