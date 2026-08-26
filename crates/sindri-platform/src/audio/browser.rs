//! The browser backend, over the Web Audio API.

use std::collections::BTreeMap;

use super::{AudioBackend, AudioClip, AudioError, AudioVoiceId, PlaybackMode, PlaybackSettings};

/// One playing element and the handler watching it fail.
///
/// The rejection handler has to outlive the promise it is attached to, and
/// nothing else here would keep it alive, so the voice owns it and drops it.
struct BrowserVoice {
    element: web_sys::HtmlAudioElement,
    /// Reused rather than rebuilt when a voice resumes, so one handler covers
    /// every promise this element hands back and none is left dangling.
    on_rejected: wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>,
}

pub struct BrowserAudioBackend {
    clips: BTreeMap<String, String>,
    voices: BTreeMap<AudioVoiceId, BrowserVoice>,
    next_voice: u64,
    unlocked: bool,
}

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

impl BrowserAudioBackend {
    /// Forgets voices that have finished or failed.
    ///
    /// The native backend does the same, and for the same reason: without it a
    /// game playing a footstep every few frames accumulates an element and a
    /// rejection handler per sound for as long as it runs. A refused clip
    /// leaves an element that is paused with an error set and will never play,
    /// so it goes the same way a finished one does.
    fn reap(&mut self) {
        self.voices
            .retain(|_, voice| !voice.element.ended() && voice.element.error().is_none());
    }
}

impl Default for BrowserAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BrowserAudioBackend {
    fn drop(&mut self) {
        for url in self.clips.values() {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}

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
        use wasm_bindgen::{JsValue, closure::Closure};

        if !self.unlocked {
            return Err(AudioError::Locked);
        }
        self.reap();
        let url = self
            .clips
            .get(clip)
            .ok_or_else(|| AudioError::MissingClip(clip.to_owned()))?;
        let element = web_sys::HtmlAudioElement::new_with_src(url).map_err(|error| {
            AudioError::Browser(format!("could not create audio element: {error:?}"))
        })?;
        element.set_loop(settings.mode == PlaybackMode::Loop);
        element.set_volume(f64::from(settings.volume.clamp(0.0, 1.0)));
        // `play()` hands back a Promise, and a browser refusing to play rejects
        // it rather than throwing: an unsupported sample rate, an unreadable
        // container, or a missing gesture all arrive later. Dropping it means
        // the failure surfaces only as an unhandled rejection in the console
        // while this returns a voice the caller believes is playing, which is
        // exactly how three unplayable clips shipped. There is nowhere
        // synchronous to report it to, so it is logged where a browser failure
        // is looked for, and the element it failed on is left carrying the
        // error that `reap` collects it by.
        let playback = element
            .play()
            .map_err(|error| AudioError::Browser(format!("playback was refused: {error:?}")))?;
        let failed = clip.to_owned();
        let on_rejected = Closure::<dyn FnMut(JsValue)>::new(move |error: JsValue| {
            log::error!("audio clip '{failed}' did not play: {error:?}");
        });
        let _ = playback.catch(&on_rejected);
        let voice = AudioVoiceId(self.next_voice);
        self.next_voice = self.next_voice.wrapping_add(1);
        self.voices.insert(
            voice,
            BrowserVoice {
                element,
                on_rejected,
            },
        );
        Ok(voice)
    }

    fn stop(&mut self, voice: AudioVoiceId) {
        if let Some(voice) = self.voices.remove(&voice) {
            let _ = voice.element.pause();
            voice.element.set_current_time(0.0);
        }
    }

    fn pause_all(&mut self) {
        for voice in self.voices.values() {
            let _ = voice.element.pause();
        }
    }

    fn resume_all(&mut self) {
        if !self.unlocked {
            return;
        }
        for voice in self.voices.values() {
            // Resuming hands back a promise exactly as starting does, and a
            // browser can refuse it exactly as readily. Dropping this one would
            // reopen the silent failure through a second door, so the voice's
            // own handler — which outlives every promise its element makes —
            // watches this one too.
            if let Ok(playback) = voice.element.play() {
                let _ = playback.catch(&voice.on_rejected);
            }
        }
    }

    fn stop_all(&mut self) {
        for (_, voice) in std::mem::take(&mut self.voices) {
            let _ = voice.element.pause();
            voice.element.set_current_time(0.0);
        }
    }

    fn unlock(&mut self) -> Result<(), AudioError> {
        self.unlocked = true;
        Ok(())
    }
}
