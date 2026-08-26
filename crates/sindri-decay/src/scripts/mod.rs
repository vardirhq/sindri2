//! Running the scripts a world holds.
//!
//! Mirrors [`sindri_scene::SpriteAnimations`] on purpose: authored facts live
//! in the scene, and what a script has become halfway through a run lives here,
//! beside the world rather than in it. A script instance's fields drift as it
//! runs, and if they were the component, watching a scene play would rewrite
//! the file it was opened from.

mod environment;
mod run;
mod sources;

use std::collections::{BTreeMap, BTreeSet};

use decay_ir::IrProgram;
use decay_runtime::{ScriptInstance, Value};
use sindri_core::{ComponentSchemaRegistry, EntityId, World};
use sindri_platform::InputState;

use self::run::{ensure_compiled, tick};
use crate::{
    Blackboard, ScriptComponent, ScriptExport, ScriptFailure, ScriptMessage, ScriptReport,
    audio_host::AudioCommand, exports::exports_of,
};

pub use environment::{environment, referenced_sources};
pub use sources::ScriptSources;

struct Compiled {
    source: String,
    program: IrProgram,
}

struct Running {
    elapsed_seconds: f32,
    source: String,
    script: String,
    instance: ScriptInstance,
}

#[derive(Default)]
pub struct Scripts {
    programs: BTreeMap<String, Compiled>,
    running: BTreeMap<EntityId, Running>,
    blackboard: Blackboard,
    /// What scripts asked to play, for whoever owns an audio device to perform.
    audio: Vec<AudioCommand>,
}

impl Scripts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_running(&self, entity: EntityId) -> bool {
        self.running.contains_key(&entity)
    }

    /// Takes what scripts asked to play since the last call.
    ///
    /// A caller with no audio device — the editor, a headless test — simply
    /// never calls this, and the requests are dropped with the runner rather
    /// than accumulating somewhere global.
    pub fn take_audio_commands(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.audio)
    }

    #[must_use]
    pub fn field(&self, entity: EntityId, name: &str) -> Option<&Value> {
        self.running.get(&entity)?.instance.field(name)
    }

    pub fn compile(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
    ) -> Vec<ScriptFailure> {
        let scripted = match components.query::<ScriptComponent>(world) {
            Ok(scripted) => scripted,
            Err(error) => return vec![ScriptFailure::Registry(error.to_string())],
        };
        let mut failures = Vec::new();
        for (entity, component) in scripted {
            if let Err(failure) = ensure_compiled(&mut self.programs, sources, entity, &component) {
                failures.push(failure);
            }
        }
        failures
    }

    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
        input: &InputState,
        delta_seconds: f32,
    ) -> ScriptReport {
        let mut report = ScriptReport::default();
        if !delta_seconds.is_finite() || delta_seconds < 0.0 {
            report.failures.push(ScriptFailure::BadDelta(delta_seconds));
            return report;
        }

        let scripted = match components.query::<ScriptComponent>(world) {
            Ok(scripted) => scripted,
            Err(error) => {
                report
                    .failures
                    .push(ScriptFailure::Registry(error.to_string()));
                return report;
            }
        };

        let Self {
            programs,
            running,
            blackboard,
            audio,
        } = self;
        let mut live = BTreeSet::new();

        for (entity, component) in scripted {
            if !component.enabled {
                continue;
            }
            live.insert(entity);

            match tick(
                programs,
                running,
                blackboard,
                audio,
                world,
                sources,
                input,
                entity,
                &component,
                delta_seconds,
            ) {
                Ok(printed) => report.printed.extend(
                    printed
                        .into_iter()
                        .map(|message| ScriptMessage { entity, message }),
                ),
                Err(failure) => report.failures.push(failure),
            }
        }

        running.retain(|entity, _| live.contains(entity));
        report
    }

    #[must_use]
    pub fn exports(&self, source: &str, script: &str) -> Option<Vec<ScriptExport>> {
        exports_of(&self.programs.get(source)?.program, script)
    }

    pub fn clear(&mut self) {
        self.programs.clear();
        self.running.clear();
        self.blackboard.clear();
        // Requests from the run being cleared belong to it. Carrying them over
        // would play the previous session's sounds into the next one.
        self.audio.clear();
    }

    #[must_use]
    pub const fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }
}

#[cfg(test)]
mod tests {
    use decay_ir::lower_with_environment;

    use super::environment;

    #[test]
    fn audio_calls_are_type_checked() {
        let source = r#"
            script Sound {
                fn start() {
                    Audio.play("audio/pickup.wav", 0.8);
                    Audio.loop("audio/music.ogg", 0.4);
                    Audio.pause_all();
                    Audio.resume_all();
                    Audio.stop_all();
                }
            }
        "#;
        let lowered = lower_with_environment(source, &environment());
        assert!(
            lowered.program.is_some(),
            "{:?}",
            lowered.analysis.diagnostics
        );
    }
}
