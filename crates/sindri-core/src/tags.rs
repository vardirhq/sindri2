//! What an entity *is*, in the words the game uses for it.
//!
//! A script that spawns hundreds of enemies cannot hold a reference to each of
//! them, so it has to be able to ask the world for a group. The question needs
//! something to ask *about*, and a name is the wrong answer: `World.find`
//! matches the name a scene gave one entity, and a game whose enemies are
//! "Scout 41" through "Scout 300" is a game with three hundred authored names
//! and no way to say "the enemies".
//!
//! A tag is that: authored, stable, and about the entity's kind rather than its
//! identity. It is deliberately not a component type — asking by component
//! would mean spelling `sindri.sprite` in a script, which puts engine
//! internals in gameplay code and makes every enemy that happens to have a
//! sprite an enemy.
//!
//! It lives here, in the crate that owns entities, rather than beside the
//! components that draw: a tag is world data with no renderer, script, or
//! platform in it, and the two crates that need it — the one that registers
//! schemas and the one that answers a script's questions — share only this
//! one.

use std::collections::BTreeSet;

use crate::SceneComponent;
use serde::Deserialize;

/// The tags one entity carries.
///
/// A set rather than a list: an entity is or is not an enemy, and carrying
/// "enemy" twice means nothing. Sorted, so a scene written twice is the same
/// bytes both times.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct TagsComponent {
    #[serde(default)]
    pub tags: BTreeSet<String>,
}

impl TagsComponent {
    /// Whether this entity carries `tag`.
    #[must_use]
    pub fn has(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }
}

impl SceneComponent for TagsComponent {
    const TYPE_NAME: &'static str = "sindri.tags";
}
