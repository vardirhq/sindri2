//! The names a script writes: host types, and the components they reach.

pub(crate) const TRANSFORM: &str = "Transform";

pub(crate) const VEC3: &str = "Vec3";

pub(crate) const SPRITE: &str = "Sprite";

pub(crate) const UI_IMAGE: &str = "UiImage";
pub(crate) const SHAPE: &str = "Shape";

pub(crate) const RGBA: &str = "Rgba";

pub(crate) const INPUT: &str = "Input";

/// Where the person is pointing, whatever they are pointing with.
pub(crate) const POINTER: &str = "Pointer";

/// The fingers, for a game that wants more than one.
pub(crate) const TOUCH: &str = "Touch";

/// A joystick made out of whichever finger is steering.
///
/// Its own namespace rather than more of `Pointer`, because it answers a
/// different question. `Pointer` says where the person is pointing, which is an
/// absolute place on the screen; a stick says which way and how hard they are
/// pushing, which is relative to wherever they put their thumb down. A game
/// that steers wants the second and gets the first only by doing the
/// subtraction itself -- which is the arithmetic every mobile game rewrites.
pub(crate) const STICK: &str = "Stick";

pub(crate) const TIME: &str = "Time";

pub(crate) const GAME: &str = "Game";

pub(crate) const WORLD: &str = "World";

pub(crate) const GRID: &str = "Grid";

/// What a body is doing, and what it touched.
///
/// Sindri physics, never Rapier: `docs/physics.md` makes the backend a private
/// implementation detail, and a namespace that leaked its vocabulary would make
/// the backend unreplaceable one script at a time.
pub(crate) const PHYSICS: &str = "Physics";
pub(crate) const UI: &str = "Ui";
pub(crate) const RANDOM: &str = "Random";
pub(crate) const SAVE: &str = "Save";
pub(crate) const EFFECTS: &str = "Effects";

/// The type of a value that names another entity.
pub(crate) const ENTITY: &str = "Entity";

/// The type of a value that names an authored prefab.
///
/// Opaque, like [`ENTITY`], and for a sharper reason: a prefab reference is a
/// project asset, and the only way a script can hold one is for the scene to
/// have authored it into an `@export` field. That is what lets the editor draw
/// an asset picker for it, resolve it against the project, load the document
/// before the first frame, and refuse a reference that names nothing. A string
/// literal in a script's source would be none of those things — invisible to
/// the asset pipeline and discovered wrong on the frame it is spawned — which
/// is why `World.spawn` takes this and not text.
pub(crate) const PREFAB: &str = "Prefab";

/// The component a sprite's fields live in.
pub(crate) const SPRITE_COMPONENT: &str = "sindri.sprite";

/// The component a UI image's fields live in.
///
/// A separate path rather than a second meaning for `sprite`: a HUD element and
/// a thing in the world are different components, and a script that writes a
/// tint should say which of the two it means.
pub(crate) const UI_IMAGE_COMPONENT: &str = "sindri.ui.image";
pub(crate) const SHAPE_COMPONENT: &str = "sindri.shape";

/// The component whose layout and entity transform define a gameplay grid.
pub(crate) const TILEMAP_COMPONENT: &str = "sindri.tilemap";
