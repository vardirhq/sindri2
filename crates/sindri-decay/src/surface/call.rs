//! What a script can call, and what each call answers with.
//!
//! One list per host type. A new call is an entry in the matching list
//! and an arm where the host dispatches it; nothing else moves.

/// A host function, by how many numbers it takes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HostFunction {
    Unary(fn(f64) -> f64),
    Binary(fn(f64, f64) -> f64),
}

/// The maths a script can do beyond arithmetic.
///
/// Decay has no modules and no imports, so each of these is a bare global name,
/// and every one added is a name a script can no longer use for its own.
pub(crate) const FUNCTIONS: &[(&str, HostFunction)] = &[
    ("abs", HostFunction::Unary(f64::abs)),
    ("sqrt", HostFunction::Unary(f64::sqrt)),
    ("sin", HostFunction::Unary(f64::sin)),
    ("cos", HostFunction::Unary(f64::cos)),
    ("min", HostFunction::Binary(f64::min)),
    ("max", HostFunction::Binary(f64::max)),
];

/// The name a script calls to say something into the host's log.
pub(crate) const PRINT: &str = "print";

/// A question a script asks about the keyboard.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InputQuery {
    /// Two opposing keys as -1, 0, or 1.
    Axis,
    Down,
    Pressed,
    Released,
}

impl InputQuery {
    /// How many key names it takes.
    pub(crate) const fn keys(self) -> usize {
        match self {
            Self::Axis => 2,
            Self::Down | Self::Pressed | Self::Released => 1,
        }
    }

    /// Whether it answers with a number rather than a truth.
    pub(crate) const fn is_number(self) -> bool {
        matches!(self, Self::Axis)
    }
}

pub(crate) const INPUT_QUERIES: &[(&str, InputQuery)] = &[
    ("axis", InputQuery::Axis),
    ("is_down", InputQuery::Down),
    ("just_pressed", InputQuery::Pressed),
    ("just_released", InputQuery::Released),
];

/// A note left on the board shared by every script in the world.
///
/// The smallest thing that lets two scripts cooperate. Decay has no value that
/// can hold an entity, so a script cannot name another one — but it can leave a
/// number under a name and another can read it. That is enough for a player to
/// publish where it is, a collectible to notice, and a score to be counted by
/// nobody in particular.
///
/// Deliberately a stopgap with a shape that admits it: names are strings and
/// nothing checks them, which typed cross-entity access would fix. It is here
/// because it is small and it unblocks a game, and a game is what tells us
/// which of the bigger answers is worth building.
#[derive(Clone, Copy, Debug)]
pub(crate) enum GameCall {
    /// `Game.get(name, fallback)`.
    ///
    /// The fallback is not optional, because a note nobody has left yet is the
    /// ordinary case on the first frame — and a `get` that silently answered
    /// zero would be a typo that reads as a legitimate value.
    Get,
    /// `Game.set(name, value)`.
    Set,
}

pub(crate) const GAME_CALLS: &[(&str, GameCall)] =
    &[("get", GameCall::Get), ("set", GameCall::Set)];

/// What a script can ask of the world it is in, as opposed to of one entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorldCall {
    /// Finds an entity by the name the scene gave it, or `null`.
    ///
    /// By name rather than by scene ID because a name is what an author typed
    /// and can see in the hierarchy, and because a runtime-spawned entity has
    /// no scene ID at all.
    Find,
    /// Removes an entity and everything under it, through `WorldCommand` so it
    /// can be undone.
    Despawn,
    /// Whether a reference still names something. A reference outlives what it
    /// names — that is what generation checking is for — so a script that holds
    /// one across frames needs to be able to ask.
    Exists,
    /// Creates the entities an authored prefab describes and answers with its
    /// root.
    ///
    /// By asset ID rather than by a value, because a prefab is a file the
    /// project holds and Decay has no literal for one. A name the host has not
    /// loaded is refused at the call: a spawn that silently produced nothing is
    /// a bug report nobody can reproduce.
    Spawn,
    /// Puts one entity under another, or at the root when given `null`.
    ///
    /// Separate from `spawn` rather than an argument to it, because reparenting
    /// is a thing a script wants to do to entities it did not create.
    SetParent,
    /// Authors an `@export` property on an entity whose script has not started.
    ///
    /// The one thing a spawner cannot do with the paths it already has. A
    /// script's own fields are not on the surface — the analyzer cannot know
    /// which container another entity runs — so a per-instance starting value
    /// is set the way the scene sets one, by writing the authored property the
    /// instance is built from.
    ///
    /// Refused once the script is running. Properties are applied when the
    /// instance is created, so a later write would land in the payload and
    /// change nothing a script could see, which is the silent no-op this whole
    /// surface is arranged to avoid.
    SetProperty,
}

pub(crate) const WORLD_CALLS: &[(&str, WorldCall)] = &[
    ("find", WorldCall::Find),
    ("despawn", WorldCall::Despawn),
    ("exists", WorldCall::Exists),
    ("spawn", WorldCall::Spawn),
    ("set_parent", WorldCall::SetParent),
    ("set_property", WorldCall::SetProperty),
];

/// A conversion between an entity's world position and a tilemap's logical
/// coordinate space.
///
/// Decay has no structured vector or grid-coordinate value yet, so continuous
/// X and Y are read separately and written together. Both entity arguments are
/// typed references: the first is what moves, and the second is the tilemap
/// whose projection and transform define the grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GridCall {
    PositionX,
    PositionY,
    Place,
    /// Whether an authored occupant has a route to another entity's cell.
    CanReach,
    /// Move an authored occupant one deterministic A* node toward a target.
    StepToward,
}

pub(crate) const GRID_CALLS: &[(&str, GridCall)] = &[
    ("position_x", GridCall::PositionX),
    ("position_y", GridCall::PositionY),
    ("place", GridCall::Place),
    ("can_reach", GridCall::CanReach),
    ("step_toward", GridCall::StepToward),
];

/// What a script can ask about the frame it is in.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TimeValue {
    /// How long this frame is, which `update` also receives as its argument.
    Delta,
    /// How long the script instance has been running.
    Elapsed,
}

pub(crate) const TIME_VALUES: &[(&str, TimeValue)] =
    &[("delta", TimeValue::Delta), ("elapsed", TimeValue::Elapsed)];
