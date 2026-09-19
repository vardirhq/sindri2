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

/// Facts about the screen a game is being drawn into.
pub(crate) const VIEWPORT: &str = "Viewport";

/// Which block the person is pointing at, and where a block clicked there goes.
///
/// Its own namespace rather than more of `Pointer` for the reason `Stick` is:
/// `Pointer` says where on the screen the person is, which any game can use.
/// This says what that means in a world made of blocks, which needs a camera
/// and a volume to answer. Keeping them apart is what lets a game with no
/// blocks in it ignore the whole idea.
pub(crate) const AIM: &str = "Aim";

/// What the person just did with a finger or a mouse, as an intention.
///
/// `Pointer` says a button is down and `Touch` says where the fingers are.
/// Neither can tell a tap from the beginning of a drag, because the difference
/// is how the press ends and how far it wandered on the way -- which is a
/// question about a press's whole life rather than about this instant. A game
/// that tried to work it out from `Pointer` alone would be writing a gesture
/// recogniser in Decay, badly, once per game.
///
/// This is why the same build works on a phone: a mouse has a second button to
/// mean "remove" and a finger does not, so removing is a hold.
pub(crate) const GESTURE: &str = "Gesture";

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

/// Where the game is, and asking to be somewhere else.
///
/// A name and nothing more: what a scene contains, where its file is and when
/// it loads are the host's business, and a namespace that knew any of it would
/// make the language responsible for a project layout.
pub(crate) const SCENE: &str = "Scene";

/// What a body is doing, and what it touched.
///
/// Sindri physics, never Rapier: `docs/physics.md` makes the backend a private
/// implementation detail, and a namespace that leaked its vocabulary would make
/// the backend unreplaceable one script at a time.
pub(crate) const PHYSICS: &str = "Physics";
pub(crate) const UI: &str = "Ui";

/// Which authored clip an entity is playing, and where it has got to.
///
/// Clips are authored, never built: a script names one the scene already holds,
/// the same way it names an audio asset. That is what keeps the editor's clip
/// list the single record of what an entity can do — a script that could invent
/// a clip would make the animation panel a partial view of the truth.
pub(crate) const ANIMATION: &str = "Animation";
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

/// The opaque type of a reusable authored profile asset.
pub(crate) const PROFILE: &str = "Profile";

/// Typed reads from profile assets.
pub(crate) const PROFILES: &str = "Profiles";

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

/// The same geometry, for a scene that has moved on to stackable tile volumes.
///
/// A flat map and a volume disagree about what a cell *contains*, and about
/// whether levels exist at all, but not about where a cell is. A script asking
/// that question is answered by whichever of the two the entity carries.
pub(crate) const TILE_GRID_COMPONENT: &str = "sindri.tile_grid";
