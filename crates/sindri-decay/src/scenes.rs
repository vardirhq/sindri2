//! What scene is being played, and what a script asked for instead.

/// The scene channel a frame runs against.
///
/// Carries the name of the scene being played in, and the name a script asked
/// to move to out. A host that plays exactly one scene passes none of this, and
/// `Scene.go` then says so rather than accepting a request that goes nowhere —
/// a game whose doors silently never open should be heard about on the first
/// frame.
///
/// The scene being played is a plain name and not an optional one. A host that
/// hands this over is playing something, and "playing nothing" would be a value
/// `Scene.current` had to answer with: null, which a script cannot compare a
/// string against, or an empty string, which is a sentinel every caller has to
/// know about. Neither is worth having for a state only the host's own startup
/// can be in.
#[derive(Clone, Debug)]
pub struct SceneChannel {
    /// Which scene the frame is running in, as the host names it.
    pub playing: String,
    /// Which scene a script asked to move to, if any asked.
    pub wanted: Option<String>,
}

impl SceneChannel {
    /// A channel playing the named scene, with nothing requested.
    #[must_use]
    pub fn playing(name: impl Into<String>) -> Self {
        Self {
            playing: name.into(),
            wanted: None,
        }
    }

    /// Takes the request, leaving the channel ready for the next frame.
    pub fn take(&mut self) -> Option<String> {
        self.wanted.take()
    }

    /// Records that the move happened, so the next frame reads the new scene.
    pub fn now_playing(&mut self, name: impl Into<String>) {
        self.playing = name.into();
        self.wanted = None;
    }
}
