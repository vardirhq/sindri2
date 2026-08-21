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
use decay_semantic::{Environment, FunctionType, HostType, Type};
use sindri_core::{ComponentSchemaRegistry, EntityId, World};
use sindri_platform::InputState;

use crate::{
    ScriptComponent, ScriptContext, ScriptFailure, ScriptMessage, ScriptReport, WorldHost,
    surface::{
        FUNCTIONS, HostFunction, INPUT, INPUT_QUERIES, Node, PRINT, THIS, TIME, TIME_VALUES,
    },
};

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

/// What a Decay script may name, as types the analyzer can check.
///
/// Registered here rather than being builtins of the language, which is the
/// boundary Decay is built around: `decay-semantic` knows that `sin` exists
/// only because this said so, and knows nothing about what it does.
///
/// Every entry is derived from [`crate::surface`], the same description
/// [`crate::WorldHost`] reaches the world through. A path the analyzer accepts
/// and the host cannot answer is a clean compile followed by a runtime failure,
/// so the two are not allowed to be written separately.
#[must_use]
pub fn environment() -> Environment {
    let mut environment = Environment::new();

    for (name, node) in THIS {
        environment.add_this_value(*name, describe_node(node));
    }
    for (name, ty) in collect_types() {
        environment.add_type(name, ty);
    }

    for (name, function) in FUNCTIONS {
        environment.add_function(
            *name,
            FunctionType {
                params: match function {
                    HostFunction::Unary(_) => vec![Type::F32],
                    HostFunction::Binary(_) => vec![Type::F32, Type::F32],
                },
                return_type: Type::F32,
            },
        );
    }

    // `print` takes anything, because a script has no way to turn a number into
    // a string -- Decay has no conversions and `+` does not concatenate -- so a
    // print that only took text could not report a value.
    environment.add_function(
        PRINT,
        FunctionType {
            params: vec![Type::Unknown],
            return_type: Type::Unit,
        },
    );

    let mut input = HostType::new();
    for (name, query) in INPUT_QUERIES {
        input = input.with_function(
            *name,
            FunctionType {
                params: vec![Type::String; query.keys()],
                return_type: if query.is_number() {
                    Type::F32
                } else {
                    Type::Bool
                },
            },
        );
    }
    environment.add_type(INPUT, input);
    environment.add_value(INPUT, Type::Named(INPUT.to_owned()));

    let mut time = HostType::new();
    for (name, _) in TIME_VALUES {
        time = time.with_value(*name, Type::F32);
    }
    environment.add_type(TIME, time);
    environment.add_value(TIME, Type::Named(TIME.to_owned()));

    environment
}

/// The type a node has: a group is its name, a leaf is a number.
fn describe_node(node: &Node) -> Type {
    match node {
        Node::Group(name, _) => Type::Named((*name).to_owned()),
        Node::Leaf(_) => Type::F32,
    }
}

/// Every named type the surface tree mentions, with its members.
///
/// A type may appear more than once with different accessors -- `position` and
/// `scale` are both a `Vec3` -- and that is fine, because a type is a shape and
/// the accessors are the host's business. Collected rather than listed so a new
/// group in the tree is described without a second edit.
fn collect_types() -> Vec<(String, HostType)> {
    fn walk(members: &'static [(&'static str, Node)], into: &mut Vec<(String, HostType)>) {
        for (_, node) in members {
            let Node::Group(name, nested) = node else {
                continue;
            };
            let mut ty = HostType::new();
            for (field, child) in *nested {
                ty = ty.with_value(*field, describe_node(child));
            }
            into.push(((*name).to_owned(), ty));
            walk(nested, into);
        }
    }
    let mut types = Vec::new();
    walk(THIS, &mut types);
    types
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
    /// How long this instance has been running, which is per instance rather
    /// than per world: a script spawned later has not been going as long.
    elapsed_seconds: f32,
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

        // Disjoint field borrows: the programs are read while the instances are
        // written, and a helper taking `&mut self` could not do both.
        let Self { programs, running } = self;
        let mut live = BTreeSet::new();

        for (entity, component) in scripted {
            if !component.enabled {
                continue;
            }
            live.insert(entity);

            match tick(
                programs,
                running,
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
                // A script that failed this frame keeps its instance: a runtime
                // error is not a reason to throw away the state the author is
                // trying to inspect, and restarting it would hide the failure
                // behind a fresh `start` every frame.
                Err(failure) => report.failures.push(failure),
            }
        }

        running.retain(|entity, _| live.contains(entity));
        report
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

#[allow(clippy::too_many_arguments)]
fn tick(
    programs: &mut BTreeMap<String, Compiled>,
    running: &mut BTreeMap<EntityId, Running>,
    world: &mut World,
    sources: &ScriptSources,
    input: &InputState,
    entity: EntityId,
    component: &ScriptComponent,
    delta_seconds: f32,
) -> Result<Vec<String>, ScriptFailure> {
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

    let elapsed_seconds = running
        .get(&entity)
        .map_or(0.0, |current| current.elapsed_seconds)
        + delta_seconds;
    let context = ScriptContext {
        input,
        delta_seconds,
        elapsed_seconds,
    };
    let mut runtime = Runtime::new(&compiled.program, WorldHost::new(world, entity, context));

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
                    elapsed_seconds,
                    source: component.source.clone(),
                    script: component.script.clone(),
                    instance,
                },
            );
            true
        }
    };

    let Some(current) = running.get_mut(&entity) else {
        return Ok(Vec::new());
    };
    current.elapsed_seconds = elapsed_seconds;

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

    Ok(runtime.into_host().take_printed())
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
