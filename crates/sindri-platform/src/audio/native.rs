//! The desktop backend, over Rodio.

use std::collections::BTreeMap;

use super::{AudioBackend, AudioClip, AudioError, AudioVoiceId, PlaybackMode, PlaybackSettings};

pub struct NativeAudioBackend {
    device: rodio::MixerDeviceSink,
    clips: BTreeMap<String, std::sync::Arc<[u8]>>,
    voices: BTreeMap<AudioVoiceId, rodio::Player>,
    next_voice: u64,
}

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
