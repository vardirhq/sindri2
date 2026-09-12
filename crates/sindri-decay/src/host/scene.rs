//! Asking to be somewhere else.
//!
//! A scene change cannot happen inside the call that asks for it. The script
//! making the request is running *in* the scene being left, from a world the
//! change would rearrange underneath it — so the call records an intention and
//! the host performs it between frames, the same way `Audio.play` records a
//! sound for whoever owns a speaker.
//!
//! Which scene is being played is a game's idea rather than the language's, so
//! nothing here loads anything or knows what a scene contains. It knows a name.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};

use crate::surface::SceneCall;

use super::WorldHost;

impl WorldHost<'_> {
    pub(super) fn scene_call(
        &mut self,
        call: SceneCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        match call {
            SceneCall::Go => {
                let Some(Value::String(name)) = args.first() else {
                    return Err(RuntimeError::Host(format!(
                        "{} names the scene to go to, as text",
                        path.dotted()
                    )));
                };
                let Some(requests) = self.scenes.as_deref_mut() else {
                    return Err(RuntimeError::Host(format!(
                        "{} was called on a host that plays one scene and cannot change it",
                        path.dotted()
                    )));
                };
                // First asked wins. Two scripts asking in one frame is a
                // conflict with no right answer, and a frame that has already
                // decided to leave should not be overruled by a later script
                // that did not know -- a door beside a door would otherwise
                // depend on which script the pass reached first.
                if requests.wanted.is_none() {
                    requests.wanted = Some(name.clone());
                }
                Ok(Value::Unit)
            }
            SceneCall::Current => {
                let Some(requests) = self.scenes.as_deref() else {
                    return Err(RuntimeError::Host(format!(
                        "{} was called on a host that plays one scene and does not name it",
                        path.dotted()
                    )));
                };
                // The scene being played, not the one asked for. A script that
                // read back its own request would see the move happen a frame
                // before it did.
                Ok(Value::String(requests.playing.clone()))
            }
        }
    }
}
