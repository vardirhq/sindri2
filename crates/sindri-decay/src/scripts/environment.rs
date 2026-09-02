//! Building the environment the analyzer checks a script against.
//!
//! Derived from [`crate::surface`] rather than written out, so what a
//! script may say and what the host will answer cannot drift apart.

use std::collections::BTreeSet;

use decay_semantic::{Environment, FunctionType, HostType, Type};
use sindri_core::{ComponentSchemaRegistry, World};

use crate::{
    ScriptComponent,
    audio_host::AUDIO,
    surface::{
        ENTITY, FUNCTIONS, GAME, GAME_CALLS, GRID, GRID_CALLS, GameCall, GridCall, HostFunction,
        INPUT, INPUT_QUERIES, Node, PREFAB, PRINT, THIS, THROUGH_REFERENCE, TIME, TIME_VALUES,
        WORLD, WORLD_CALLS, WorldCall,
    },
};

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
                    WorldCall::Find | WorldCall::WithTag => vec![Type::String],
                    WorldCall::Spawn => vec![Type::Named(PREFAB.to_owned())],
                    WorldCall::Despawn | WorldCall::Exists => {
                        vec![Type::Named(ENTITY.to_owned())]
                    }
                    WorldCall::SetParent => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                    // The value is `Unknown` because an exported field may be a
                    // number, a truth, or text, and Decay has no union. The
                    // host checks what it was given, and the instance refuses a
                    // value the field cannot hold when it is built.
                    WorldCall::SetProperty => {
                        vec![Type::Named(ENTITY.to_owned()), Type::String, Type::Unknown]
                    }
                },
                return_type: match call {
                    WorldCall::Find | WorldCall::Spawn => Type::Named(ENTITY.to_owned()),
                    WorldCall::WithTag => Type::array_of(Type::Named(ENTITY.to_owned())),
                    WorldCall::Despawn | WorldCall::SetParent | WorldCall::SetProperty => {
                        Type::Unit
                    }
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

pub(super) fn add_grid_surface(environment: &mut Environment) {
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
                    GridCall::CanReach | GridCall::StepToward => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                },
                return_type: match call {
                    GridCall::PositionX | GridCall::PositionY => Type::F32,
                    GridCall::Place => Type::Unit,
                    GridCall::CanReach | GridCall::StepToward => Type::Bool,
                },
            },
        );
    }
    environment.add_type(GRID, grid);
    environment.add_value(GRID, Type::Named(GRID.to_owned()));
}

pub(super) fn add_audio_surface(environment: &mut Environment) {
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
pub(super) fn describe_node(node: &Node) -> Type {
    match node {
        Node::Group(name, _) => Type::Named((*name).to_owned()),
        Node::Leaf(_) => Type::F32,
        Node::Handle(_) => Type::Named(ENTITY.to_owned()),
    }
}

/// Every named type the surface tree mentions, with its members.
pub(super) fn collect_types() -> Vec<(String, HostType)> {
    pub(super) fn walk(
        members: &'static [(&'static str, Node)],
        into: &mut Vec<(String, HostType)>,
    ) {
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
