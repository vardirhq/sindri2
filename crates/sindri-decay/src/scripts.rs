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
    Blackboard, ScriptComponent, ScriptContext, ScriptExport, ScriptFailure, ScriptMessage,
    ScriptReport, WorldHost,
    audio_host::AUDIO,
    exports::exports_of,
    surface::{
        ENTITY, FUNCTIONS, GAME, GAME_CALLS, GRID, GRID_CALLS, GameCall, GridCall, HostFunction,
        INPUT, INPUT_QUERIES, Node, PRINT, THIS, THROUGH_REFERENCE, TIME, TIME_VALUES, WORLD,
        WORLD_CALLS, WorldCall,
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
/// Every entry is derived from the same host surface the runtime implements. A
/// path the analyzer accepts and the host cannot answer is a clean compile
/// followed by a runtime failure, so namespaces are described and implemented
/// as one feature change.
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

    let mut game = HostType::new();
    for (name, call) in GAME_CALLS {
        game = game.with_function(
            *name,
            FunctionType {
                params: vec![Type::String, Type::F32],
                return_type: match call {
                    GameCall::Get => Type::F32,
                    GameCall::Set => Type::Unit,
                },
            },
        );
    }
    environment.add_type(GAME, game);
    environment.add_value(GAME, Type::Named(GAME.to_owned()));

    let mut time = HostType::new();
    for (name, _) in TIME_VALUES {
        time = time.with_value(*name, Type::F32);
    }
    environment.add_type(TIME, time);
    environment.add_value(TIME, Type::Named(TIME.to_owned()));

    let mut world = HostType::new();
    for (name, call) in WORLD_CALLS {
        world = world.with_function(
            *name,
            FunctionType {
                params: match call {
                    WorldCall::Find => vec![Type::String],
                    WorldCall::Despawn | WorldCall::Exists => {
                        vec![Type::Named(ENTITY.to_owned())]
                    }
                },
                return_type: match call {
                    WorldCall::Find => Type::Named(ENTITY.to_owned()),
                    WorldCall::Despawn => Type::Unit,
                    WorldCall::Exists => Type::Bool,
                },
            },
        );
    }
    environment.add_type(WORLD, world);
    environment.add_value(WORLD, Type::Named(WORLD.to_owned()));

    add_grid_surface(&mut environment);
    add_audio_surface(&mut environment);

    environment
}

fn add_grid_surface(environment: &mut Environment) {
    let mut grid = HostType::new();
    for (name, call) in GRID_CALLS {
        grid = grid.with_function(
            *name,
            FunctionType {
                params: match call {
                    GridCall::PositionX | GridCall::PositionY => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                    GridCall::Place => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                        Type::F32,
                        Type::F32,
                    ],
                },
                return_type: match call {
                    GridCall::PositionX | GridCall::PositionY => Type::F32,
                    GridCall::Place => Type::Unit,
                },
            },
        );
    }
    environment.add_type(GRID, grid);
    environment.add_value(GRID, Type::Named(GRID.to_owned()));
}

fn add_audio_surface(environment: &mut Environment) {
    let audio = HostType::new()
        .with_function(
            "play",
            FunctionType {
                params: vec![Type::String, Type::F32],
                return_type: Type::Unit,
            },
        )
        .with_function(
            "loop",
            FunctionType {
                params: vec![Type::String, Type::F32],
                return_type: Type::Unit,
            },
        )
        .with_function(
            "stop_all",
            FunctionType {
                params: Vec::new(),
                return_type: Type::Unit,
            },
        )
        .with_function(
            "pause_all",
            FunctionType {
                params: Vec::new(),
                return_type: Type::Unit,
            },
        )
        .with_function(
            "resume_all",
            FunctionType {
                params: Vec::new(),
                return_type: Type::Unit,
            },
        );
    environment.add_type(AUDIO, audio);
    environment.add_value(AUDIO, Type::Named(AUDIO.to_owned()));
}

/// The type a node has: a group is its name, a leaf is a number.
fn describe_node(node: &Node) -> Type {
    match node {
        Node::Group(name, _) => Type::Named((*name).to_owned()),
        Node::Leaf(_) => Type::F32,
        Node::Handle(_) => Type::Named(ENTITY.to_owned()),
    }
}

/// Every named type the surface tree mentions, with its members.
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

    let mut entity = HostType::new();
    for (field, node) in THROUGH_REFERENCE {
        entity = entity.with_value(*field, describe_node(node));
    }
    types.push((ENTITY.to_owned(), entity));
    types
}

pub fn referenced_sources(world: &World, components: &ComponentSchemaRegistry) -> BTreeSet<String> {
    components
        .query::<ScriptComponent>(world)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, component)| component.source)
        .collect()
}

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
    }

    #[must_use]
    pub const fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }
}

#[allow(clippy::too_many_arguments)]
fn tick(
    programs: &mut BTreeMap<String, Compiled>,
    running: &mut BTreeMap<EntityId, Running>,
    blackboard: &mut Blackboard,
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
    let mut runtime = Runtime::new(
        &compiled.program,
        WorldHost::new(world, entity, context, blackboard),
    );

    let started = match running.get(&entity) {
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

fn to_value(value: &serde_json::Value) -> Option<Value> {
    Some(match value {
        serde_json::Value::Number(number) => Value::Number(number.as_f64()?),
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Null => Value::Null,
        _ => return None,
    })
}

#[cfg(test)]
mod audio_surface_tests {
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
