use sindri_core::EntityId;

use crate::ScriptFailure;

/// Something a script said, and which script said it.
///
/// The entity is carried because "player moved" is not something an author can
/// act on when six entities run the same script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptMessage {
    pub entity: EntityId,
    pub message: String,
}

/// What a frame of scripts produced.
///
/// One value rather than two channels: what was printed and what went wrong are
/// both "what the scripts had to say this frame", and a caller that had to
/// remember to drain a second place would eventually not.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptReport {
    /// What scripts printed, in the order they ran.
    pub printed: Vec<ScriptMessage>,
    /// What went wrong, per script. One failing script does not stop the rest.
    pub failures: Vec<ScriptFailure>,
}

impl ScriptReport {
    /// Whether the frame was uneventful, which is the common case and the one
    /// a caller should be able to check without looking at two fields.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.printed.is_empty() && self.failures.is_empty()
    }
}
