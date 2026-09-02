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
    #[error(
        "scripts kept spawning after {rounds} rounds of starting what the last \
         round made; {pending} entities were left unstarted"
    )]
    SpawnCascade { rounds: usize, pending: usize },
    #[error("entity {entity:?} failed in {script}.{function}: {error}")]
    Runtime {
        entity: EntityId,
        script: String,
        function: String,
        error: String,
    },
}

impl ScriptFailure {
    /// The entity this is about, when it is about one.
    ///
    /// A failure that names an entity is one an author can act on, and acting
    /// on it means going to the entity. The runtime has only a handle, so
    /// saying which one is as far as this can go; a host that holds the world
    /// turns it into a selection.
    pub const fn entity(&self) -> Option<EntityId> {
        match self {
            Self::BadDelta(_)
            | Self::Registry(_)
            | Self::SpawnCascade { .. }
            | Self::Compile { .. } => None,
            Self::MissingSource { entity, .. }
            | Self::UnknownScript { entity, .. }
            | Self::Property { entity, .. }
            | Self::Runtime { entity, .. } => Some(*entity),
        }
    }

    /// What went wrong, without naming the entity.
    ///
    /// `Display` names it as a handle, because a handle is all the runtime has.
    /// A host that knows the world can say "Wisp" instead, and this is the
    /// other half of that sentence: `EntityId { index: 4, generation: 0 }` is
    /// not something anyone can look for in a hierarchy.
    pub fn detail(&self) -> String {
        match self {
            Self::MissingSource { asset, .. } => {
                format!("names script source '{asset}', which is not loaded")
            }
            Self::UnknownScript { asset, script, .. } => {
                format!("names script '{script}', which '{asset}' does not declare")
            }
            Self::Property {
                script,
                property,
                reason,
                ..
            } => format!("sets '{script}.{property}', but {reason}"),
            Self::Runtime {
                script,
                function,
                error,
                ..
            } => format!("failed in {script}.{function}: {error}"),
            // The three that are about no entity read the same either way.
            other => other.to_string(),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> EntityId {
        sindri_core::World::default().next_handle()
    }

    /// A failure that names an entity says which, so a host holding the world
    /// can turn it into a selection.
    #[test]
    fn a_failure_about_an_entity_names_it() {
        let entity = entity();
        let about = [
            ScriptFailure::MissingSource {
                entity,
                asset: "scripts/spin.decay".to_owned(),
            },
            ScriptFailure::UnknownScript {
                entity,
                asset: "scripts/spin.decay".to_owned(),
                script: "Spin".to_owned(),
            },
            ScriptFailure::Property {
                entity,
                script: "Spin".to_owned(),
                property: "speed".to_owned(),
                reason: "it is not exported".to_owned(),
            },
            ScriptFailure::Runtime {
                entity,
                script: "Spin".to_owned(),
                function: "update".to_owned(),
                error: "divided by zero".to_owned(),
            },
        ];
        for failure in about {
            assert_eq!(failure.entity(), Some(entity), "{failure:?}");
        }
    }

    /// The three that are about the whole run, rather than one entity, name
    /// none — and a host must not claim otherwise by selecting something.
    #[test]
    fn a_failure_about_no_entity_names_none() {
        for failure in [
            ScriptFailure::BadDelta(-1.0),
            ScriptFailure::Registry("missing".to_owned()),
            ScriptFailure::Compile {
                asset: "scripts/spin.decay".to_owned(),
                diagnostics: vec!["line 3: expected }".to_owned()],
            },
        ] {
            assert_eq!(failure.entity(), None, "{failure:?}");
        }
    }

    /// `detail` is the sentence without the handle in it.
    ///
    /// `EntityId { index: 0, generation: 0 }` is what the runtime has and not
    /// something anyone can look for in a hierarchy, so a host that knows the
    /// world says "Wisp" and then this.
    #[test]
    fn detail_leaves_the_naming_to_the_host() {
        let failure = ScriptFailure::Runtime {
            entity: entity(),
            script: "Wisp".to_owned(),
            function: "update".to_owned(),
            error: "divided by zero".to_owned(),
        };
        assert_eq!(failure.detail(), "failed in Wisp.update: divided by zero");
        assert!(
            !failure.detail().contains("EntityId"),
            "the handle is the host's to replace"
        );
        assert!(
            failure.to_string().contains("EntityId"),
            "and Display still says everything the runtime knows"
        );
    }

    /// A failure about no entity reads the same either way, because there is
    /// nothing for a host to put in front of it.
    #[test]
    fn detail_and_display_agree_when_there_is_no_entity() {
        let failure = ScriptFailure::Compile {
            asset: "scripts/spin.decay".to_owned(),
            diagnostics: vec!["line 3: expected }".to_owned()],
        };
        assert_eq!(failure.detail(), failure.to_string());
    }
}
