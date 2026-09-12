//! Which scenes a world is holding, and which one is being played.
//!
//! A game with more than one place in it needs somewhere to keep that fact. The
//! world can hold several scenes at once — see [`World::add_scene`] — but it has
//! no opinion about which of them the player is in, and nor should it: a world
//! is entities, and "where we are" is a game's own idea.
//!
//! So this is the bookkeeping between the two. It remembers the root each scene
//! was loaded under, switches one on and the rest off, and loads a scene the
//! first time it is asked for. Everything a scene contains stays exactly as the
//! player left it, because leaving a scene disables its root rather than
//! unloading it.

use std::collections::BTreeMap;

use crate::{EntityData, EntityId, SceneDocument, World, WorldError};

/// The scenes a world holds, and which one is live.
///
/// Scenes are named by the caller — an asset ID, usually — and that name is
/// both the key here and the namespace their stable IDs are loaded under, so
/// an entity's identity says which scene it came from.
#[derive(Clone, Debug, Default)]
pub struct LoadedScenes {
    roots: BTreeMap<String, EntityId>,
    active: Option<String>,
}

impl LoadedScenes {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The scene being played, if any.
    #[must_use]
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// The root a scene was loaded under.
    ///
    /// This is the switch: disabling it takes the whole scene out of play,
    /// because [`World::is_active`] walks ancestors.
    #[must_use]
    pub fn root(&self, name: &str) -> Option<EntityId> {
        self.roots.get(name).copied()
    }

    /// Whether this scene is already in the world.
    #[must_use]
    pub fn holds(&self, name: &str) -> bool {
        self.roots.contains_key(name)
    }

    /// Every scene held, in name order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.roots.keys().map(String::as_str)
    }

    /// Loads a scene into the world if it is not already there, and returns the
    /// root it lives under.
    ///
    /// The root is an entity of its own rather than the scene's first entity,
    /// so a scene with several roots still has exactly one switch, and so the
    /// switch exists before anything is under it.
    /// Loads a scene into the world if it is not already there, and returns the
    /// root it lives under.
    ///
    /// The scene's stable IDs are namespaced by `name`. Use
    /// [`Self::load_keeping_identities`] for a project's own opening scene,
    /// where the identities in the file are the ones everything else was
    /// written against.
    pub fn load(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
    ) -> Result<EntityId, WorldError> {
        self.load_under(world, name, scene, name)
    }

    /// The same, keeping the identities the scene file spells.
    ///
    /// For the scene a project opens on. Everything that resolves an entity by
    /// stable ID was written against the file, so renaming them at load would
    /// make the runtime and the file disagree about what anything is called.
    /// See `World::add_scene` on the cost.
    pub fn load_keeping_identities(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
    ) -> Result<EntityId, WorldError> {
        self.load_under(world, name, scene, "")
    }

    fn load_under(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
        namespace: &str,
    ) -> Result<EntityId, WorldError> {
        if let Some(root) = self.roots.get(name) {
            return Ok(*root);
        }
        let root = world.spawn(EntityData {
            name: Some(name.to_owned()),
            // Off until something asks for it. A scene that arrived live would
            // draw itself over whatever is already being played for the frame
            // between loading and switching.
            disabled: true,
            ..EntityData::default()
        });
        // If the scene will not load, the root it would have lived under goes
        // away with it: a name that half-exists is worse than one that does
        // not, because `holds` would answer yes for an empty scene.
        if let Err(error) = world.add_scene(scene, namespace, Some(root)) {
            let _ = world.despawn_recursive(root);
            return Err(error);
        }
        self.roots.insert(name.to_owned(), root);
        Ok(root)
    }

    /// Switches to a scene the world already holds.
    ///
    /// Every other scene is switched off, including one that was somehow left
    /// on: the invariant is that exactly one scene is in play, and restoring it
    /// is cheaper than trusting it.
    pub fn go_to(&mut self, world: &mut World, name: &str) -> Result<(), SceneSwitchError> {
        if !self.roots.contains_key(name) {
            return Err(SceneSwitchError::NotLoaded(name.to_owned()));
        }
        for (held, root) in &self.roots {
            let Some(data) = world.get_mut(*root) else {
                return Err(SceneSwitchError::RootGone(held.clone()));
            };
            data.disabled = held != name;
        }
        self.active = Some(name.to_owned());
        Ok(())
    }

    /// Loads a scene if needed, then switches to it.
    ///
    /// The call a door makes, and the reason the two halves above are separate:
    /// a game that wants to load a scene early without entering it can.
    pub fn enter(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
    ) -> Result<EntityId, SceneSwitchError> {
        self.enter_under(world, name, scene, name)
    }

    /// The same, keeping the identities the scene file spells.
    pub fn enter_keeping_identities(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
    ) -> Result<EntityId, SceneSwitchError> {
        self.enter_under(world, name, scene, "")
    }

    fn enter_under(
        &mut self,
        world: &mut World,
        name: &str,
        scene: &SceneDocument,
        namespace: &str,
    ) -> Result<EntityId, SceneSwitchError> {
        let root = self
            .load_under(world, name, scene, namespace)
            .map_err(|source| SceneSwitchError::Load {
                scene: name.to_owned(),
                source,
            })?;
        self.go_to(world, name)?;
        Ok(root)
    }

    /// Forgets a scene, and takes it out of the world.
    ///
    /// The one thing that does discard played state, so it is a separate verb
    /// from leaving a scene rather than what leaving one happens to do.
    pub fn unload(&mut self, world: &mut World, name: &str) -> Result<bool, WorldError> {
        let Some(root) = self.roots.remove(name) else {
            return Ok(false);
        };
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
        world.despawn_recursive(root)?;
        Ok(true)
    }
}

/// Why a scene change did not happen.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum SceneSwitchError {
    #[error("no scene named '{0}' is loaded")]
    NotLoaded(String),
    #[error("the root holding scene '{0}' is gone from the world")]
    RootGone(String),
    #[error("scene '{scene}' could not be loaded: {source}")]
    Load {
        scene: String,
        #[source]
        source: WorldError,
    },
}
