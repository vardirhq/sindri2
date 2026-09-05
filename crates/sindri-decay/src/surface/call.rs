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
    ("atan2", HostFunction::Binary(f64::atan2)),
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
    /// Whether a screen element is taking the pointer this frame.
    ///
    /// The engine does not silently withhold input from gameplay when a menu is
    /// up: which scripts are gameplay is not something a host can know, and a
    /// rule that guesses is one that will guess wrong. So a gameplay script
    /// asks, in one line, and the answer is why a click on a pause button does
    /// not also fire the gun behind it.
    OverUi,
    /// Where the pointer is in the overlay's own units, across and up.
    ///
    /// `x` and `y` are viewport pixels, and how many pixels tall a window is
    /// is not something a scene knows — so a script could tell where the
    /// pointer was on the screen and not what it was pointing at. The overlay
    /// is two tall and centred on the origin, which is a space the scene
    /// authored against, so a game that knows how much world its camera frames
    /// can turn these into world coordinates and the engine does not have to
    /// guess at a camera on a script's behalf.
    OverlayX,
    OverlayY,
}

pub(crate) const POINTER_VALUES: &[(&str, PointerValue)] = &[
    ("x", PointerValue::X),
    ("y", PointerValue::Y),
    ("overlay_x", PointerValue::OverlayX),
    ("overlay_y", PointerValue::OverlayY),
    ("inside", PointerValue::Inside),
    ("over_ui", PointerValue::OverUi),
];

/// A value a script reads about the viewport it is running in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ViewportValue {
    Aspect,
}

pub(crate) const VIEWPORT_VALUES: &[(&str, ViewportValue)] = &[("aspect", ViewportValue::Aspect)];

/// What a script reads from the steering stick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StickValue {
    /// How far it is pushed, -1 to 1, in screen axes.
    X,
    Y,
    /// Whether a finger is on it at all.
    ///
    /// Not the same as a zero reading, which is also what a thumb resting
    /// inside the dead zone gives: a game drawing the control wants to show it
    /// while it is held even when it is centred.
    Held,
    /// Where the thumb landed, for a game that draws the ring.
    AnchorX,
    AnchorY,
}

pub(crate) const STICK_VALUES: &[(&str, StickValue)] = &[
    ("x", StickValue::X),
    ("y", StickValue::Y),
    ("held", StickValue::Held),
    ("anchor_x", StickValue::AnchorX),
    ("anchor_y", StickValue::AnchorY),
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
    /// Reads a numeric authored property from another entity's script.
    ///
    /// This is the other half of `set_property`: a projectile can carry the
    /// damage it was spawned with, and the target it reaches can read that
    /// value without routing a per-instance fact through the global board.
    /// The fallback is required so an optional property and a misspelling do
    /// not both silently become zero.
    PropertyNumber,
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
    /// Whether one active entity carries an authored tag.
    HasTag,
    /// Switches an entity — and everything under it — on or off.
    ///
    /// `docs/scripting.md` says a screen is an entity with children and that
    /// showing one is switching it on. That was true of the engine and not of
    /// Decay: a script could make and destroy entities but not hide one, so a
    /// title screen could be authored and never dismissed, and the only way to
    /// remove a menu was to despawn it and lose the ability to show it again.
    SetActive,
    /// Whether an entity takes part in the scene.
    ///
    /// The question `set_active` answers, which a script needs to toggle a
    /// pause overlay rather than track a truth that the world already holds
    /// and that something else may have changed.
    IsActive,
}

pub(crate) const WORLD_CALLS: &[(&str, WorldCall)] = &[
    ("find", WorldCall::Find),
    ("despawn", WorldCall::Despawn),
    ("exists", WorldCall::Exists),
    ("spawn", WorldCall::Spawn),
    ("set_parent", WorldCall::SetParent),
    ("set_property", WorldCall::SetProperty),
    ("property_number", WorldCall::PropertyNumber),
    ("with_tag", WorldCall::WithTag),
    ("has_tag", WorldCall::HasTag),
    ("set_active", WorldCall::SetActive),
    ("is_active", WorldCall::IsActive),
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
    /// Whether the pointer is over this element.
    Hovered,
    /// Whether this element was clicked during this frame.
    ///
    /// A click is a press and a release on the same element, so sliding off
    /// before letting go changes a person's mind — the behaviour every
    /// platform's buttons already have.
    Pressed,
    /// Whether the pointer is being held down on this element.
    Held,
}

impl UiCall {
    /// Whether this asks about the pointer rather than changing the element.
    pub(crate) const fn is_query(self) -> bool {
        matches!(self, Self::Hovered | Self::Pressed | Self::Held)
    }
}

pub(crate) const UI_CALLS: &[(&str, UiCall)] = &[
    ("set_text", UiCall::Text),
    ("set_number", UiCall::Number),
    ("set_numbers", UiCall::Numbers),
    ("set_fill", UiCall::Fill),
    ("is_hovered", UiCall::Hovered),
    ("is_pressed", UiCall::Pressed),
    ("is_held", UiCall::Held),
];

/// What a script can draw from the run's stream.
///
/// One stream, shared by every script, owned by the host. Same seed and same
/// sequence of calls means the same numbers on every host — which is what makes
/// a run replayable, and also means a number taken early shifts every number
/// after it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RandomCall {
    /// A fraction in `[0, 1)`.
    Value,
    /// A number in `[min, max)`.
    Range,
    /// A whole number from `min` to `max`, both included.
    ///
    /// Inclusive because "a number from 1 to 6" means six outcomes to everyone
    /// who is not writing the loop themselves.
    Int,
    /// One of the entities in a collection.
    ///
    /// Decay has no indexing, so without this a script cannot choose from a
    /// group at all — and choosing from a group is most of what a game wants
    /// randomness for.
    Pick,
    /// Puts the run's stream back to the start of a seed.
    Seed,
}

pub(crate) const RANDOM_CALLS: &[(&str, RandomCall)] = &[
    ("value", RandomCall::Value),
    ("range", RandomCall::Range),
    ("int", RandomCall::Int),
    ("pick", RandomCall::Pick),
    ("seed", RandomCall::Seed),
];

/// What a game remembers between runs.
///
/// Reads and writes go to an in-memory store; getting it onto a disk or into a
/// browser is the host's business and happens on its own schedule. A script
/// that had to ask for a write would be a script deciding how often someone's
/// disk is touched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SaveCall {
    /// A stored number, or the fallback when there is none.
    ///
    /// A fallback rather than an optional because every caller has one — a
    /// starting score, a default volume — and a save is mostly read on the run
    /// where nothing has been stored yet.
    Number,
    SetNumber,
    Flag,
    SetFlag,
    /// Whether anything is stored under a key.
    Has,
    /// Forgets everything, which is what "reset my progress" means.
    Clear,
    /// Whether this is a first run, with nothing stored.
    IsNew,
    /// Whether something was stored and could not be read.
    ///
    /// Separate from `IsNew` because they call for different things: a first
    /// run starts cheerfully, and a damaged save is worth telling someone about
    /// before their progress is written over.
    IsDamaged,
    /// Whether what is stored was written by a newer build than this one.
    IsFromNewer,
}

pub(crate) const SAVE_CALLS: &[(&str, SaveCall)] = &[
    ("number", SaveCall::Number),
    ("set_number", SaveCall::SetNumber),
    ("flag", SaveCall::Flag),
    ("set_flag", SaveCall::SetFlag),
    ("has", SaveCall::Has),
    ("clear", SaveCall::Clear),
    ("is_new", SaveCall::IsNew),
    ("is_damaged", SaveCall::IsDamaged),
    ("is_from_newer", SaveCall::IsFromNewer),
];

/// Short-lived visual flecks a script can throw.
///
/// What a burst looks like is authored on the entity as `sindri.effect.burst`,
/// because how many, how fast, how big and what colour are a designer's numbers
/// and a call that named all of them would be one nobody could read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EffectsCall {
    /// Throws the entity's authored burst at the entity's own position.
    Burst,
    /// Throws it somewhere else, which is what an explosion where something
    /// used to be needs.
    BurstAt,
    /// How many flecks are alive.
    Live,
}

pub(crate) const EFFECTS_CALLS: &[(&str, EffectsCall)] = &[
    ("burst", EffectsCall::Burst),
    ("burst_at", EffectsCall::BurstAt),
    ("live", EffectsCall::Live),
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
