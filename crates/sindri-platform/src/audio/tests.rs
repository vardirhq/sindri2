//! What the silent backend still has to get right.

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
