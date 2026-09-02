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

use self::run::{TickWorld, ensure_compiled, tick};
use crate::{
    Blackboard, Physics2d, PrefabSources, ScriptComponent, ScriptExport, ScriptFailure,
    ScriptMessage, ScriptReport, audio_host::AudioCommand, exports::exports_of, surface::PREFAB,
};

pub use environment::{environment, referenced_sources};
pub use sources::ScriptSources;

/// How many times a pass will start what the previous round spawned.
///
/// A script started by a spawn may spawn in turn, and settling that in one
/// frame is what makes "a bullet moves on the frame it is fired" true for a
/// bullet fired by something that was itself just created. A cascade that does
/// not settle is a bug in the scripts, and it is reported with the round count
/// rather than being run until the frame is gone.
const SPAWN_ROUNDS: usize = 8;

/// How many entities one pass of scripts may create.
///
/// Decay's operation budget already stops a loop that never ends, but it stops
/// it after a million instructions — long after a spawn loop with a mistaken
/// bound has put a hundred thousand entities in the world and taken the editor
/// with it. This is the same protection stated in the units the mistake is
/// made in.
pub(crate) const SPAWN_LIMIT_PER_PASS: usize = 4096;

/// Everything one pass of scripts needs from the frame around it.
///
/// A struct rather than parameters, because the list had reached six and every
/// capability the scripting surface grows adds another: the input, the prefabs
/// it may spawn, the physics it may drive. A caller that does not offer one of
/// them says so by leaving a field out of the literal rather than by passing
/// something empty in the right position.
pub struct ScriptFrame<'a> {
    pub sources: &'a ScriptSources,
    pub prefabs: &'a PrefabSources,
    pub input: &'a InputState,
    /// The physics a script may read and drive, when the host runs any.
    ///
    /// `None` for a host with no physics — a headless test, a scene that never
    /// authored a collider — and then a script calling `Physics.*` is told so
    /// rather than quietly doing nothing.
    pub physics: Option<Physics2d<'a>>,
    /// Where the screen elements are and what the pointer is doing to them.
    ///
    /// `None` for a host that draws no UI, and then `Ui.is_pressed` says so
    /// rather than answering that nothing was clicked — a menu whose buttons
    /// never respond because nothing is laying them out should be heard about
    /// on the first frame, not mistaken for a person who has not clicked yet.
    pub screen_ui: Option<&'a sindri_scene::ScreenUi>,
    pub delta_seconds: f32,
}

impl<'a> ScriptFrame<'a> {
    /// A frame with sources and nothing else, for a caller that only runs
    /// scripts.
    #[must_use]
    pub fn new(sources: &'a ScriptSources, input: &'a InputState, delta_seconds: f32) -> Self {
        Self {
            sources,
            prefabs: PrefabSources::none(),
            input,
            physics: None,
            screen_ui: None,
            delta_seconds,
        }
    }

    /// The same frame, with prefabs a script may spawn.
    #[must_use]
    pub fn with_prefabs(mut self, prefabs: &'a PrefabSources) -> Self {
        self.prefabs = prefabs;
        self
    }

    /// The same frame, with the screen elements a script may ask about.
    #[must_use]
    pub const fn with_screen_ui(mut self, screen_ui: &'a sindri_scene::ScreenUi) -> Self {
        self.screen_ui = Some(screen_ui);
        self
    }

    /// The same frame, with physics a script may read and drive.
    #[must_use]
    pub fn with_physics(mut self, physics: Physics2d<'a>) -> Self {
        self.physics = Some(physics);
        self
    }
}

struct Compiled {
    source: String,
    program: IrProgram,
}

/// Files one tick's outcome into the report.
fn collect(
    report: &mut ScriptReport,
    entity: EntityId,
    outcome: Result<Vec<String>, ScriptFailure>,
) {
    match outcome {
        Ok(printed) => report.printed.extend(
            printed
                .into_iter()
                .map(|message| ScriptMessage { entity, message }),
        ),
        Err(failure) => report.failures.push(failure),
    }
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

    /// Runs one pass of every enabled script, then starts what that pass
    /// spawned.
    ///
    /// A spawned entity's script starts **in the same pass**, so a bullet
    /// created during an update moves during that update rather than standing
    /// still for a frame. It cannot start during the call that created it:
    /// building an instance runs the container's field initializers, which is
    /// Decay code, and the world is already lent to the call in progress.
    /// So spawning creates the entity now and starting it happens after —
    /// which is also why `World.set_property` is the way a spawner authors a
    /// starting value, and why it is refused once an instance exists.
    ///
    /// A script started this way may spawn in turn. Those rounds are bounded:
    /// a cascade that does not settle is stopped and reported rather than
    /// taking the frame with it.
    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        frame: ScriptFrame<'_>,
    ) -> ScriptReport {
        let ScriptFrame {
            sources,
            prefabs,
            input,
            physics,
            screen_ui,
            delta_seconds,
        } = frame;
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
        let mut at = TickWorld {
            programs,
            running,
            blackboard,
            audio,
            world,
            sources,
            prefabs,
            input,
            physics,
            screen_ui,
            started: BTreeSet::new(),
            spawned: Vec::new(),
        };
        at.started.extend(at.running.keys().copied());
        let mut live = BTreeSet::new();

        for (entity, component) in scripted {
            if !component.enabled {
                continue;
            }
            live.insert(entity);
            collect(
                &mut report,
                entity,
                tick(&mut at, entity, &component, delta_seconds),
            );
        }

        Self::start_spawned(&mut report, &mut live, &mut at, components, delta_seconds);

        at.running.retain(|entity, _| live.contains(entity));
        report
    }

    /// Starts the scripts on entities this pass created, and on entities those
    /// created, until nothing new appears.
    fn start_spawned(
        report: &mut ScriptReport,
        live: &mut BTreeSet<EntityId>,
        at: &mut TickWorld<'_>,
        components: &ComponentSchemaRegistry,
        delta_seconds: f32,
    ) {
        let mut pending: Vec<EntityId> = std::mem::take(&mut at.spawned);
        for round in 0..SPAWN_ROUNDS {
            if pending.is_empty() {
                return;
            }
            for entity in std::mem::take(&mut pending) {
                // Read through the registry rather than the payload, so a
                // spawned script is validated exactly as an authored one is.
                let component = match components.get::<ScriptComponent>(&*at.world, entity) {
                    Ok(Some(component)) => component,
                    Ok(None) => continue,
                    Err(error) => {
                        report
                            .failures
                            .push(ScriptFailure::Registry(error.to_string()));
                        continue;
                    }
                };
                if !component.enabled {
                    continue;
                }
                live.insert(entity);
                collect(report, entity, tick(at, entity, &component, delta_seconds));
            }
            pending = std::mem::take(&mut at.spawned);
            if !pending.is_empty() && round + 1 == SPAWN_ROUNDS {
                report.failures.push(ScriptFailure::SpawnCascade {
                    rounds: SPAWN_ROUNDS,
                    pending: pending.len(),
                });
            }
        }
    }

    /// Every prefab asset the world's scripts could spawn.
    ///
    /// Found through the *declared* type of each `@export` field rather than by
    /// looking for strings that resemble asset IDs: a field declared `Prefab`
    /// is a prefab reference, and one declared `String` is text however much it
    /// looks like a path. That distinction is the whole reason `World.spawn`
    /// refuses text — it is what lets a host load a scene's prefabs before the
    /// first frame instead of discovering them when a script spawns.
    ///
    /// Empty for a source that has not compiled yet, because the declared types
    /// are not known until it has. A host asks again once it has.
    pub fn referenced_prefabs(
        &self,
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> BTreeSet<String> {
        let mut referenced = BTreeSet::new();
        for (_, component) in components
            .query::<ScriptComponent>(world)
            .unwrap_or_default()
        {
            let Some(exports) = self.exports(&component.source, &component.script) else {
                continue;
            };
            for export in exports {
                if export.type_name.as_deref() != Some(PREFAB) {
                    continue;
                }
                if let Some(serde_json::Value::String(id)) = component.properties.get(&export.name)
                {
                    referenced.insert(id.clone());
                }
            }
        }
        referenced
    }

    #[must_use]
    pub fn exports(&self, source: &str, script: &str) -> Option<Vec<ScriptExport>> {
        exports_of(&self.programs.get(source)?.program, script)
    }

    /// The scripts one compiled source declares.
    ///
    /// What an editor offers when asking which script of a file an entity runs.
    /// Empty for a source that has not compiled, which is not the same as a
    /// source declaring nothing — but both leave a panel with no names to
    /// offer, so both are the same answer here.
    #[must_use]
    pub fn declared(&self, source: &str) -> Vec<String> {
        self.programs
            .get(source)
            .map(|compiled| {
                compiled
                    .program
                    .containers
                    .iter()
                    .map(|container| container.name.clone())
                    .collect()
            })
            .unwrap_or_default()
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
