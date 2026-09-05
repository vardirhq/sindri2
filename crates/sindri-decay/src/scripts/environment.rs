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
        EFFECTS, EFFECTS_CALLS, ENTITY, EffectsCall, FUNCTIONS, GAME, GAME_CALLS, GRID, GRID_CALLS,
        GameCall, GridCall, HostFunction, INPUT, INPUT_QUERIES, Node, PHYSICS, PHYSICS_CALLS,
        POINTER, POINTER_QUERIES, POINTER_VALUES, PREFAB, PRINT, PhysicsCall, PointerValue, RANDOM,
        RANDOM_CALLS, RandomCall, SAVE, SAVE_CALLS, STICK, STICK_VALUES, SaveCall, StickValue,
        THIS, THROUGH_REFERENCE, TIME, TIME_VALUES, TOUCH, TOUCH_CALLS, TOUCH_COUNT, UI, UI_CALLS,
        UiCall, VIEWPORT, VIEWPORT_VALUES, WORLD, WORLD_CALLS, WorldCall,
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

    add_world_surface(&mut environment);

    add_pointer_surface(&mut environment);
    add_viewport_surface(&mut environment);
    add_physics_surface(&mut environment);
    add_ui_surface(&mut environment);
    add_random_surface(&mut environment);
    add_save_surface(&mut environment);
    add_effects_surface(&mut environment);
    add_grid_surface(&mut environment);
    add_audio_surface(&mut environment);

    environment
}

/// The shape of the screen the host is drawing into.
pub(super) fn add_viewport_surface(environment: &mut Environment) {
    let mut viewport = HostType::new();
    for (name, _) in VIEWPORT_VALUES {
        viewport = viewport.with_value(*name, Type::F32);
    }
    environment.add_type(VIEWPORT, viewport);
    environment.add_value(VIEWPORT, Type::Named(VIEWPORT.to_owned()));
}

/// What a script can do to the world it is in: find, spawn, despawn, reparent,
/// and write an exported field on another script.
///
/// Its own function like every other namespace, rather than inline in
/// `environment`, so that adding a call does not grow one function towards the
/// limit the others were split out to stay under.
pub(super) fn add_world_surface(environment: &mut Environment) {
    let mut world = HostType::new();
    for (name, call) in WORLD_CALLS {
        world = world.with_function(
            *name,
            FunctionType {
                params: match call {
                    WorldCall::Find | WorldCall::WithTag => vec![Type::String],
                    WorldCall::Spawn => vec![Type::Named(PREFAB.to_owned())],
                    WorldCall::Despawn | WorldCall::Exists | WorldCall::IsActive => {
                        vec![Type::Named(ENTITY.to_owned())]
                    }
                    WorldCall::SetActive => {
                        vec![Type::Named(ENTITY.to_owned()), Type::Bool]
                    }
                    WorldCall::HasTag => {
                        vec![Type::Named(ENTITY.to_owned()), Type::String]
                    }
                    WorldCall::SetParent => vec![
                        Type::Named(ENTITY.to_owned()),
                        Type::Named(ENTITY.to_owned()),
                    ],
                    WorldCall::SetShapePoint => vec![Type::F32, Type::F32, Type::F32],
                    // The value is `Unknown` because an exported field may be a
                    // number, a truth, or text, and Decay has no union. The
                    // host checks what it was given, and the instance refuses a
                    // value the field cannot hold when it is built.
                    WorldCall::SetProperty => {
                        vec![Type::Named(ENTITY.to_owned()), Type::String, Type::Unknown]
                    }
                    WorldCall::PropertyNumber => {
                        vec![Type::Named(ENTITY.to_owned()), Type::String, Type::F32]
                    }
                },
                return_type: match call {
                    WorldCall::Find | WorldCall::Spawn => Type::Named(ENTITY.to_owned()),
                    WorldCall::WithTag => Type::array_of(Type::Named(ENTITY.to_owned())),
                    WorldCall::Despawn
                    | WorldCall::SetParent
                    | WorldCall::SetShapePoint
                    | WorldCall::SetProperty
                    | WorldCall::SetActive => Type::Unit,
                    WorldCall::Exists | WorldCall::IsActive | WorldCall::HasTag => Type::Bool,
                    WorldCall::PropertyNumber => Type::F32,
                },
            },
        );
    }
    environment.add_type(WORLD, world);
    environment.add_value(WORLD, Type::Named(WORLD.to_owned()));
}

/// Where the person is pointing, and the fingers behind it.
pub(super) fn add_pointer_surface(environment: &mut Environment) {
    let mut pointer = HostType::new();
    for (name, value) in POINTER_VALUES {
        pointer = pointer.with_value(
            *name,
            match value {
                PointerValue::X
                | PointerValue::Y
                | PointerValue::OverlayX
                | PointerValue::OverlayY => Type::F32,
                PointerValue::Inside | PointerValue::OverUi => Type::Bool,
            },
        );
    }
    for (name, _) in POINTER_QUERIES {
        pointer = pointer.with_function(
            *name,
            FunctionType {
                params: vec![Type::String],
                return_type: Type::Bool,
            },
        );
    }
    environment.add_type(POINTER, pointer);
    environment.add_value(POINTER, Type::Named(POINTER.to_owned()));

    let mut touch = HostType::new().with_value(TOUCH_COUNT, Type::F32);
    for (name, _) in TOUCH_CALLS {
        touch = touch.with_function(
            *name,
            FunctionType {
                params: vec![Type::F32],
                return_type: Type::F32,
            },
        );
    }
    environment.add_type(TOUCH, touch);
    environment.add_value(TOUCH, Type::Named(TOUCH.to_owned()));

    let mut stick = HostType::new();
    for (name, value) in STICK_VALUES {
        stick = stick.with_value(
            *name,
            match value {
                StickValue::Held => Type::Bool,
                StickValue::X | StickValue::Y | StickValue::AnchorX | StickValue::AnchorY => {
                    Type::F32
                }
            },
        );
    }
    environment.add_type(STICK, stick);
    environment.add_value(STICK, Type::Named(STICK.to_owned()));
}

/// What a script can do to a body, and ask about what it touched.
pub(super) fn add_physics_surface(environment: &mut Environment) {
    let entity = || Type::Named(ENTITY.to_owned());
    let mut physics = HostType::new();
    for (name, call) in PHYSICS_CALLS {
        physics = physics.with_function(
            *name,
            FunctionType {
                params: match call {
                    PhysicsCall::VelocityX | PhysicsCall::VelocityY => vec![entity()],
                    PhysicsCall::SetVelocity | PhysicsCall::ApplyImpulse => {
                        vec![entity(), Type::F32, Type::F32]
                    }
                    // An event query is about the entity the script is on, so
                    // it takes nothing: an event is about a pair, and the pair
                    // a script cares about is the one it is half of.
                    _ => Vec::new(),
                },
                return_type: match call {
                    PhysicsCall::VelocityX | PhysicsCall::VelocityY => Type::F32,
                    PhysicsCall::SetVelocity | PhysicsCall::ApplyImpulse => Type::Unit,
                    _ => Type::array_of(entity()),
                },
            },
        );
    }
    environment.add_type(PHYSICS, physics);
    environment.add_value(PHYSICS, Type::Named(PHYSICS.to_owned()));
}

/// What a script can change about a screen element.
///
/// The words stay in the scene and the numbers come from here: Decay cannot
/// build a string, so a HUD's template is authored and a script fills its
/// slots.
pub(super) fn add_ui_surface(environment: &mut Environment) {
    let entity = || Type::Named(ENTITY.to_owned());
    let mut ui = HostType::new();
    for (name, call) in UI_CALLS {
        ui = ui.with_function(
            *name,
            FunctionType {
                params: match call {
                    UiCall::Text => vec![entity(), Type::String],
                    UiCall::Numbers => vec![entity(), Type::F32, Type::F32],
                    UiCall::Number | UiCall::Fill => vec![entity(), Type::F32],
                    _ => vec![entity()],
                },
                return_type: if call.is_query() {
                    Type::Bool
                } else {
                    Type::Unit
                },
            },
        );
    }
    environment.add_type(UI, ui);
    environment.add_value(UI, Type::Named(UI.to_owned()));
}

/// What a script can draw from the run's stream.
pub(super) fn add_random_surface(environment: &mut Environment) {
    let mut random = HostType::new();
    for (name, call) in RANDOM_CALLS {
        random = random.with_function(
            *name,
            FunctionType {
                params: match call {
                    RandomCall::Value => Vec::new(),
                    RandomCall::Range | RandomCall::Int => vec![Type::F32, Type::F32],
                    RandomCall::Pick => vec![Type::array_of(Type::Named(ENTITY.to_owned()))],
                    RandomCall::Seed => vec![Type::F32],
                },
                return_type: match call {
                    RandomCall::Value | RandomCall::Range | RandomCall::Int => Type::F32,
                    RandomCall::Pick => Type::Named(ENTITY.to_owned()),
                    RandomCall::Seed => Type::Unit,
                },
            },
        );
    }
    environment.add_type(RANDOM, random);
    environment.add_value(RANDOM, Type::Named(RANDOM.to_owned()));
}

/// What a game remembers between runs.
pub(super) fn add_save_surface(environment: &mut Environment) {
    let mut save = HostType::new();
    for (name, call) in SAVE_CALLS {
        save = save.with_function(
            *name,
            FunctionType {
                params: match call {
                    SaveCall::Number | SaveCall::SetNumber => vec![Type::String, Type::F32],
                    SaveCall::Flag | SaveCall::SetFlag => vec![Type::String, Type::Bool],
                    SaveCall::Has => vec![Type::String],
                    _ => Vec::new(),
                },
                return_type: match call {
                    SaveCall::Number => Type::F32,
                    SaveCall::Flag
                    | SaveCall::Has
                    | SaveCall::IsNew
                    | SaveCall::IsDamaged
                    | SaveCall::IsFromNewer => Type::Bool,
                    SaveCall::SetNumber | SaveCall::SetFlag | SaveCall::Clear => Type::Unit,
                },
            },
        );
    }
    environment.add_type(SAVE, save);
    environment.add_value(SAVE, Type::Named(SAVE.to_owned()));
}

/// Short-lived visual flecks a script can throw.
pub(super) fn add_effects_surface(environment: &mut Environment) {
    let mut effects = HostType::new();
    for (name, call) in EFFECTS_CALLS {
        effects = effects.with_function(
            *name,
            FunctionType {
                params: match call {
                    EffectsCall::Burst => vec![Type::Named(ENTITY.to_owned())],
                    EffectsCall::BurstAt => {
                        vec![Type::Named(ENTITY.to_owned()), Type::F32, Type::F32]
                    }
                    EffectsCall::Live => Vec::new(),
                },
                // How many flecks were made, which is fewer than asked for when
                // the pool is full. A game can watch it and turn itself down.
                return_type: Type::F32,
            },
        );
    }
    environment.add_type(EFFECTS, effects);
    environment.add_value(EFFECTS, Type::Named(EFFECTS.to_owned()));
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
