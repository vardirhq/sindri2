//! Running the scripts a world holds.
//!
//! Mirrors [`sindri_scene::SpriteAnimations`] on purpose: authored facts live in
//! the scene, and what a script has become halfway through a run lives here,
//! beside the world rather than in it. A script instance's fields drift as it
//! runs, and if they were the component, watching a scene play would rewrite the
//! file it was opened from.

use std::collections::{BTreeMap, BTreeSet};

use decay_ir::{IrContainer, IrProgram, lower_with_environment};
use decay_runtime::{Runtime, ScriptInstance, Value};
use decay_semantic::{Environment, FunctionType, Type};
use sindri_core::{ComponentSchemaRegistry, EntityId, World};

use crate::{ScriptComponent, ScriptFailure, WorldHost};

/// The lifecycle function called once, before the first update.
const START: &str = "start";
/// The lifecycle function called every frame, with the frame's delta.
const UPDATE: &str = "update";

/// The `.decay` sources a world's scripts refer to, by asset ID.
///
/// This crate does no I/O — it has no more business opening a file than
/// `sindri-core` does, and staying out of it is what lets every test here run
/// with no filesystem and no browser. The host fills this the same way the
/// editor fills [`sindri_scene::TextureBindings`]: through `sindri-assets`,
/// which already knows how to fetch a logical ID on either target.
#[derive(Clone, Debug, Default)]
pub struct ScriptSources {
    sources: BTreeMap<String, String>,
}

impl ScriptSources {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, source: impl Into<String>) {
        self.sources.insert(id.into(), source.into());
    }

    pub fn remove(&mut self, id: &str) -> Option<String> {
        self.sources.remove(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&str> {
        self.sources.get(id).map(String::as_str)
    }
}

/// The host globals a Decay script may name.
///
/// Registered here rather than being builtins of the language, which is the
/// boundary Decay is built around: `decay-semantic` knows that `sin` exists
/// only because this said so, and knows nothing about what it does.
#[must_use]
pub fn environment() -> Environment {
    let mut environment = Environment::new();
    for name in ["abs", "sqrt", "sin", "cos"] {
        environment.add_function(
            name,
            FunctionType {
                params: vec![Type::F32],
                return_type: Type::F32,
            },
        );
    }
    for name in ["min", "max"] {
        environment.add_function(
            name,
            FunctionType {
                params: vec![Type::F32, Type::F32],
                return_type: Type::F32,
            },
        );
    }
    environment
}

/// Every `.decay` source the world's scripts name.
///
/// The mirror of `sindri_scene::referenced_textures`: a world's references are
/// the statement of what it needs, and whoever owns the asset pipeline asks
/// this what to fetch. Disabled scripts are included — an author toggling one
/// back on should not then wait for a load.
pub fn referenced_sources(world: &World, components: &ComponentSchemaRegistry) -> BTreeSet<String> {
    components
        .query::<ScriptComponent>(world)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, component)| component.source)
        .collect()
}

/// A lowered source, kept with the text it came from so a changed file
/// recompiles and an unchanged one does not.
struct Compiled {
    source: String,
    program: IrProgram,
}

struct Running {
    /// Which source and container this instance came from, so that repointing
    /// the component at another script starts a new instance rather than
    /// feeding the old one someone else's fields.
    source: String,
    script: String,
    instance: ScriptInstance,
}

/// Every script instance in a world, and the programs behind them.
#[derive(Default)]
pub struct Scripts {
    programs: BTreeMap<String, Compiled>,
    running: BTreeMap<EntityId, Running>,
}

impl Scripts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether an entity currently has a live script instance.
    #[must_use]
    pub fn is_running(&self, entity: EntityId) -> bool {
        self.running.contains_key(&entity)
    }

    /// A field of an entity's live instance, which is how a debugger or an
    /// inspector would watch a script's state without stopping it.
    #[must_use]
    pub fn field(&self, entity: EntityId, name: &str) -> Option<&Value> {
        self.running.get(&entity)?.instance.field(name)
    }

    /// Moves every enabled script in `world` on by `delta_seconds`.
    ///
    /// Returns what went wrong rather than stopping at the first failure. One
    /// script must not be able to silence the others: in the editor that would
    /// mean a typo in one object freezing every other, and the author would be
    /// looking for the wrong bug.
    ///
    /// Instances are created here, on first sight of a scripted entity, and
    /// their `start` runs before their first `update`. An entity that stops
    /// being scripted — despawned, component removed, script disabled — loses
    /// its instance, so what survives a call is exactly what the world still
    /// justifies.
    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        sources: &ScriptSources,
        delta_seconds: f32,
    ) -> Vec<ScriptFailure> {
        let mut failures = Vec::new();
        if !delta_seconds.is_finite() || delta_seconds < 0.0 {
            failures.push(ScriptFailure::BadDelta(delta_seconds));
            return failures;
        }

        let scripted = match components.query::<ScriptComponent>(world) {
            Ok(scripted) => scripted,
            Err(error) => {
                failures.push(ScriptFailure::Registry(error.to_string()));
                return failures;
            }
        };

        // Disjoint field borrows: the programs are read while the instances are
        // written, and a helper taking `&mut self` could not do both.
        let Self { programs, running } = self;
        let mut live = BTreeSet::new();

        for (entity, component) in scripted {
            if !component.enabled {
                continue;
            }
            live.insert(entity);

            if let Err(failure) = tick(
                programs,
                running,
                world,
                sources,
                entity,
                &component,
                delta_seconds,
            ) {
                // A script that failed this frame keeps its instance: a runtime
                // error is not a reason to throw away the state the author is
                // trying to inspect, and restarting it would hide the failure
                // behind a fresh `start` every frame.
                failures.push(failure);
            }
        }

        running.retain(|entity, _| live.contains(entity));
        failures
    }

    /// Drops every instance and every compiled program.
    ///
    /// A script instance belongs to the world it was started against, and a
    /// freshly loaded world reuses entity slots from the beginning — so keeping
    /// them would attach one entity's running state to another.
    pub fn clear(&mut self) {
        self.programs.clear();
        self.running.clear();
    }
}

fn tick(
    programs: &mut BTreeMap<String, Compiled>,
    running: &mut BTreeMap<EntityId, Running>,
    world: &mut World,
    sources: &ScriptSources,
    entity: EntityId,
    component: &ScriptComponent,
    delta_seconds: f32,
) -> Result<(), ScriptFailure> {
    ensure_compiled(programs, sources, entity, component)?;
    let compiled = &programs[&component.source];

    let container = compiled
        .program
        .containers
        .iter()
        .find(|container| container.name == component.script)
        .ok_or_else(|| ScriptFailure::UnknownScript {
            entity,
            asset: component.source.clone(),
            script: component.script.clone(),
        })?;

    let mut runtime = Runtime::new(&compiled.program, WorldHost::new(world, entity));

    let started = match running.get(&entity) {
        // Repointing the component at another script starts a new instance
        // rather than feeding the old one someone else's fields.
        Some(current)
            if current.source == component.source && current.script == component.script =>
        {
            false
        }
        _ => {
            let mut instance = runtime.instantiate(&component.script).map_err(|error| {
                ScriptFailure::runtime(entity, &component.script, START, &error)
            })?;
            apply_properties(&mut instance, container, entity, component)?;
            running.insert(
                entity,
                Running {
                    source: component.source.clone(),
                    script: component.script.clone(),
                    instance,
                },
            );
            true
        }
    };

    let Some(current) = running.get_mut(&entity) else {
        return Ok(());
    };

    if started && container.functions.iter().any(|f| f.name == START) {
        runtime
            .call_instance(&mut current.instance, START, vec![])
            .map_err(|error| ScriptFailure::runtime(entity, &component.script, START, &error))?;
    }

    if container.functions.iter().any(|f| f.name == UPDATE) {
        runtime
            .call_instance(
                &mut current.instance,
                UPDATE,
                vec![Value::Number(f64::from(delta_seconds))],
            )
            .map_err(|error| ScriptFailure::runtime(entity, &component.script, UPDATE, &error))?;
    }

    Ok(())
}

/// Lowers a source if it is new or has changed, and leaves it otherwise.
///
/// Comparing the text is all hot reload needs from this side: the editor
/// replaces the source, and the next frame is running the new program.
fn ensure_compiled(
    programs: &mut BTreeMap<String, Compiled>,
    sources: &ScriptSources,
    entity: EntityId,
    component: &ScriptComponent,
) -> Result<(), ScriptFailure> {
    let Some(source) = sources.get(&component.source) else {
        return Err(ScriptFailure::MissingSource {
            entity,
            asset: component.source.clone(),
        });
    };
    if programs
        .get(&component.source)
        .is_some_and(|compiled| compiled.source == source)
    {
        return Ok(());
    }

    let lowered = lower_with_environment(source, &environment());
    let program = lowered.program.ok_or_else(|| ScriptFailure::Compile {
        asset: component.source.clone(),
        diagnostics: lowered
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "{}:{}: {}",
                    diagnostic.line, diagnostic.column, diagnostic.message
                )
            })
            .collect(),
    })?;
    programs.insert(
        component.source.clone(),
        Compiled {
            source: source.to_owned(),
            program,
        },
    );
    Ok(())
}

/// Puts the scene's authored values into the instance, before `start` runs, so
/// a script's first line sees what the scene gave it rather than its default.
///
/// A property is refused rather than ignored in every failing case -- not
/// declared, not `@export`, or not a value Decay has. An authored number that
/// silently goes nowhere is the exact shape of bug this whole component exists
/// to make visible.
fn apply_properties(
    instance: &mut ScriptInstance,
    container: &IrContainer,
    entity: EntityId,
    component: &ScriptComponent,
) -> Result<(), ScriptFailure> {
    let refuse = |property: &str, reason: &str| ScriptFailure::Property {
        entity,
        script: component.script.clone(),
        property: property.to_owned(),
        reason: reason.to_owned(),
    };

    for (name, value) in &component.properties {
        let Some(field) = container.fields.iter().find(|field| field.name == *name) else {
            return Err(refuse(name, "the script declares no such field"));
        };
        if !field.exported {
            return Err(refuse(name, "the field is not @export"));
        }
        let value = to_value(value).ok_or_else(|| {
            refuse(
                name,
                &format!("{value} is not a number, string, or boolean"),
            )
        })?;
        instance
            .set_field(name, value)
            .map_err(|error| refuse(name, &format!("{error:?}")))?;
    }
    Ok(())
}

/// A JSON property as a Decay value.
///
/// Arrays and objects are absent because Decay has no such values yet. Adding
/// them here before the language has them would be authoring something no
/// script can read.
fn to_value(value: &serde_json::Value) -> Option<Value> {
    Some(match value {
        serde_json::Value::Number(number) => Value::Number(number.as_f64()?),
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Null => Value::Null,
        _ => return None,
    })
}
