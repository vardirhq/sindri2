//! The names a script writes: host types, and the components they reach.

pub(crate) const TRANSFORM: &str = "Transform";

pub(crate) const VEC3: &str = "Vec3";

pub(crate) const SPRITE: &str = "Sprite";

pub(crate) const RGBA: &str = "Rgba";

pub(crate) const INPUT: &str = "Input";

pub(crate) const TIME: &str = "Time";

pub(crate) const GAME: &str = "Game";

pub(crate) const WORLD: &str = "World";

pub(crate) const GRID: &str = "Grid";

/// The type of a value that names another entity.
pub(crate) const ENTITY: &str = "Entity";

/// The component a sprite's fields live in.
pub(crate) const SPRITE_COMPONENT: &str = "sindri.sprite";

/// The component whose layout and entity transform define a gameplay grid.
pub(crate) const TILEMAP_COMPONENT: &str = "sindri.tilemap";
