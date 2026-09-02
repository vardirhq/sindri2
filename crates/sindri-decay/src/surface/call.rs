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

/// A question a script asks about where the person is pointing.
///
/// One namespace for the mouse and the finger, because a game that aims at a
/// point should not have to ask which the person is using — and a game written
/// for a mouse then works on a phone without a second code path. What each
/// unified answer means when both are present is in `docs/scripting.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerQuery {
    Down,
    Pressed,
    Released,
}

pub(crate) const POINTER_QUERIES: &[(&str, PointerQuery)] = &[
    ("is_down", PointerQuery::Down),
    ("just_pressed", PointerQuery::Pressed),
    ("just_released", PointerQuery::Released),
];

/// A value a script reads about where the pointer is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerValue {
    X,
    Y,
    /// Whether there is a pointer at all.
    ///
    /// A mouse outside the window and a screen nobody is touching are the same
    /// answer, and a game drawing a cursor or testing a button needs to know
    /// before it reads a position that would otherwise be the last one.
    Inside,
}

pub(crate) const POINTER_VALUES: &[(&str, PointerValue)] = &[
    ("x", PointerValue::X),
    ("y", PointerValue::Y),
    ("inside", PointerValue::Inside),
];

/// A question about the fingers specifically.
///
/// Separate from `Pointer` because it answers something `Pointer` cannot: how
/// many there are, and where the second one is. A game that only needs "where
/// is the person pointing" never touches this.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TouchCall {
    X,
    Y,
}

pub(crate) const TOUCH_CALLS: &[(&str, TouchCall)] = &[("x", TouchCall::X), ("y", TouchCall::Y)];

/// How many fingers are down, as a value rather than a call.
pub(crate) const TOUCH_COUNT: &str = "count";

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
    /// Every active entity carrying an authored tag, in world order.
    ///
    /// The answer to "a game cannot hold a reference to each of three hundred
    /// enemies". By tag rather than by name, because `find` matches the name a
    /// scene gave *one* entity and a game whose enemies are "Scout 41" through
    /// "Scout 300" has three hundred authored names and no way to say "the
    /// enemies". By tag rather than by component type, because spelling
    /// `sindri.sprite` in a script puts engine internals in gameplay code and
    /// makes every enemy that happens to have a sprite an enemy.
    ///
    /// Bounded, ordered, and a snapshot. See `docs/scripting.md` for what each
    /// of those costs and buys.
    WithTag,
}

pub(crate) const WORLD_CALLS: &[(&str, WorldCall)] = &[
    ("find", WorldCall::Find),
    ("despawn", WorldCall::Despawn),
    ("exists", WorldCall::Exists),
    ("spawn", WorldCall::Spawn),
    ("set_parent", WorldCall::SetParent),
    ("set_property", WorldCall::SetProperty),
    ("with_tag", WorldCall::WithTag),
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

/// What a script can do to a body, and ask about what it touched.
///
/// The velocity pair is split for the reason `Grid.position_x` and
/// `position_y` are: Decay has no vector value yet. Setting takes both at once,
/// because a velocity with one axis half-applied is a frame of motion nobody
/// asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicsCall {
    VelocityX,
    VelocityY,
    SetVelocity,
    ApplyImpulse,
    /// The entities this one started touching during the last step.
    ///
    /// A query rather than a callback, because Decay now has a value that can
    /// hold several entities and a lifecycle function would be a second way for
    /// the host to enter a script. Answered for the entity the script is on:
    /// an event is about a pair, and the pair a script cares about is the one it
    /// is half of.
    CollisionStarted,
    CollisionStopped,
    /// The same, for colliders authored as sensors, which register a touch and
    /// do not push back.
    SensorEntered,
    SensorExited,
}

pub(crate) const PHYSICS_CALLS: &[(&str, PhysicsCall)] = &[
    ("velocity_x", PhysicsCall::VelocityX),
    ("velocity_y", PhysicsCall::VelocityY),
    ("set_velocity", PhysicsCall::SetVelocity),
    ("apply_impulse", PhysicsCall::ApplyImpulse),
    ("collision_started", PhysicsCall::CollisionStarted),
    ("collision_stopped", PhysicsCall::CollisionStopped),
    ("sensor_entered", PhysicsCall::SensorEntered),
    ("sensor_exited", PhysicsCall::SensorExited),
];

impl PhysicsCall {
    /// Whether this asks about what happened rather than acting on a body.
    pub(crate) const fn is_event(self) -> bool {
        matches!(
            self,
            Self::CollisionStarted
                | Self::CollisionStopped
                | Self::SensorEntered
                | Self::SensorExited
        )
    }
}

/// What a script can change about a screen element.
///
/// Every one writes into the payload the entity already carries, so a HUD's
/// state lives in the world rather than in a table beside it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiCall {
    // Named for what each one is about rather than for the `set_` its call
    // wears, so the queries this namespace grows sit beside them evenly.
    /// Replaces the words, template and all.
    ///
    /// For swapping one authored string for another — a warning appearing, a
    /// label changing with the mode. A script can only pass a literal, because
    /// Decay has no way to build a string, which is exactly why the numbers go
    /// through the calls below instead.
    Text,
    /// Fills the template's one slot.
    Number,
    /// Fills its first two, which is what `45/100` needs.
    Numbers,
    /// How much of a bar is drawn, in `[0, 1]`.
    Fill,
}

pub(crate) const UI_CALLS: &[(&str, UiCall)] = &[
    ("set_text", UiCall::Text),
    ("set_number", UiCall::Number),
    ("set_numbers", UiCall::Numbers),
    ("set_fill", UiCall::Fill),
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
