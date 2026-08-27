//! Hearing a clip before naming it in a component.
//!
//! You could not, and picking a sound by filename is picking a sound by
//! guessing. The engine already plays audio through
//! `sindri_platform::audio::AudioBackend`; what was missing is a way to ask it
//! for one clip, now, without a running scene.
//!
//! Deliberately separate from the scene's audio. This owns its own backend, so
//! auditioning a clip while a scene is stopped needs no lifecycle, and a
//! preview cannot leave a voice running in the world someone then presses Play
//! on.

use std::path::Path;

use sindri_platform::{AudioBackend, AudioClip, AudioError, NativeAudioBackend, PlaybackSettings};

/// The editor's own way of playing a sound.
///
/// The device is opened on the first clip rather than at startup: an editor
/// that grabs the audio device before anyone asks for a sound is an editor that
/// argues with whatever else is playing.
#[derive(Default)]
pub struct Audition {
    backend: Option<NativeAudioBackend>,
    /// Which clips the backend has already been given, so a second play of the
    /// same file does not re-read and re-register it.
    registered: Vec<String>,
}

impl Audition {
    /// Plays a file once, at full volume.
    ///
    /// The clip is named by its path, which is unique within a project and is
    /// what the browser already has in hand.
    pub fn play(&mut self, path: &Path) -> Result<(), AuditionError> {
        let id = path.display().to_string();
        let backend = match &mut self.backend {
            Some(backend) => backend,
            None => self
                .backend
                .insert(NativeAudioBackend::new().map_err(AuditionError::Audio)?),
        };
        if !self.registered.iter().any(|known| known == &id) {
            let bytes = std::fs::read(path).map_err(|source| AuditionError::Read {
                path: id.clone(),
                source,
            })?;
            let mime = mime_of(path).ok_or_else(|| AuditionError::Unplayable(id.clone()))?;
            backend
                .register(AudioClip::new(id.clone(), bytes, mime))
                .map_err(AuditionError::Audio)?;
            self.registered.push(id.clone());
        }
        backend
            .play(&id, PlaybackSettings::once(1.0))
            .map(|_| ())
            .map_err(AuditionError::Audio)
    }

    /// Silences whatever is playing.
    ///
    /// Its own control rather than only a second press of Play: a two-minute
    /// music bed auditioned by accident should be stoppable without waiting.
    pub fn stop(&mut self) {
        if let Some(backend) = &mut self.backend {
            backend.stop_all();
        }
    }
}

/// What the engine calls this container, or `None` for one it does not play.
///
/// The three every Sindri host promises to understand. A file the browser lists
/// as audio by an extension nothing decodes is not offered a play button, for
/// the reason a `.txt` is not offered a picture: a control that cannot do what
/// it says is worse than no control.
fn mime_of(path: &Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase());
    match extension.as_deref() {
        Some("wav") => Some("audio/wav"),
        Some("ogg") => Some("audio/ogg"),
        Some("mp3") => Some("audio/mpeg"),
        _ => None,
    }
}

/// Whether the editor can play this file at all.
pub fn is_audible(path: &Path) -> bool {
    mime_of(path).is_some()
}

/// Why a clip did not play.
#[derive(Debug, thiserror::Error)]
pub enum AuditionError {
    #[error("{path} could not be read: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("'{0}' is not a container this engine plays")]
    Unplayable(String),
    #[error(transparent)]
    Audio(AudioError),
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_audible, mime_of};

    /// The three containers every Sindri host promises to understand.
    #[test]
    fn the_containers_the_engine_plays_are_the_ones_offered() {
        assert_eq!(mime_of(Path::new("audio/pickup.wav")), Some("audio/wav"));
        assert_eq!(mime_of(Path::new("audio/music.OGG")), Some("audio/ogg"));
        assert_eq!(mime_of(Path::new("audio/theme.mp3")), Some("audio/mpeg"));
    }

    /// A control that cannot do what it says is worse than no control, so a
    /// file nothing decodes is not offered a play button.
    #[test]
    fn a_container_nothing_decodes_is_not_offered() {
        for name in ["audio/theme.flac", "textures/orb.png", "README"] {
            assert!(!is_audible(Path::new(name)), "{name} is not playable");
        }
    }
}
