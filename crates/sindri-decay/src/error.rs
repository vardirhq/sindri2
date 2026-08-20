use decay_runtime::RuntimeError;
use sindri_core::EntityId;
use thiserror::Error;

/// What went wrong for one script, in one frame.
///
/// Every variant names the entity, because "a script failed" is not something an
/// author can act on and "the script on entity 4 divided by zero" is.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ScriptFailure {
    #[error("a frame cannot be {0} seconds long")]
    BadDelta(f32),
    #[error("scripts need sindri.script registered: {0}")]
    Registry(String),
    #[error("entity {entity:?} names script source '{asset}', which is not loaded")]
    MissingSource { entity: EntityId, asset: String },
    // `source` is reserved by thiserror for a wrapped error, and these carry an
    // asset ID rather than a cause.
    #[error("'{asset}' did not compile: {}", diagnostics.join("; "))]
    Compile {
        asset: String,
        diagnostics: Vec<String>,
    },
    #[error("entity {entity:?} names script '{script}', which '{asset}' does not declare")]
    UnknownScript {
        entity: EntityId,
        asset: String,
        script: String,
    },
    #[error("entity {entity:?} sets '{script}.{property}', but {reason}")]
    Property {
        entity: EntityId,
        script: String,
        property: String,
        reason: String,
    },
    #[error("entity {entity:?} failed in {script}.{function}: {error}")]
    Runtime {
        entity: EntityId,
        script: String,
        function: String,
        error: String,
    },
}

impl ScriptFailure {
    pub(crate) fn runtime(
        entity: EntityId,
        script: &str,
        function: &str,
        error: &RuntimeError,
    ) -> Self {
        Self::Runtime {
            entity,
            script: script.to_owned(),
            function: function.to_owned(),
            error: format!("{error:?}"),
        }
    }
}
