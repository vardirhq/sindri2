use std::collections::{BTreeMap, BTreeSet};

use sindri_core::{SceneComponent, World};
use sindri_render::{TextureId, TextureRegistry};

use crate::{MeshComponent, SpriteComponent};

/// Maps the texture references a scene names to the textures a renderer holds.
///
/// Scenes refer to textures by a stable string; the renderer knows only
/// handles. This is where the two meet. A reference nothing has bound resolves
/// to the missing texture, so an absent or failed asset draws as obviously
/// wrong rather than failing the frame or, worse, silently reusing whatever
/// texture happened to be bound last.
#[derive(Clone, Debug, Default)]
pub struct TextureBindings {
    bound: BTreeMap<String, TextureId>,
}

impl TextureBindings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `reference` to `texture`, returning the handle it replaced.
    pub fn bind(&mut self, reference: impl Into<String>, texture: TextureId) -> Option<TextureId> {
        self.bound.insert(reference.into(), texture)
    }

    /// The texture bound to `reference`, if any.
    pub fn get(&self, reference: &str) -> Option<TextureId> {
        self.bound.get(reference).copied()
    }

    /// The texture to draw `reference` with, or the missing texture.
    pub fn resolve(&self, reference: &str) -> TextureId {
        self.get(reference).unwrap_or(TextureRegistry::MISSING)
    }

    pub fn len(&self) -> usize {
        self.bound.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bound.is_empty()
    }

    pub fn references(&self) -> impl Iterator<Item = &str> {
        self.bound.keys().map(String::as_str)
    }
}

/// Every texture a world draws with that nothing has bound.
///
/// Hosts call this after loading to report what is missing by name, rather than
/// leaving a magenta surface as the only clue.
pub fn unresolved_textures(world: &World, bindings: &TextureBindings) -> BTreeSet<String> {
    let mut missing = BTreeSet::new();
    for (_, data) in world.entities() {
        for (type_name, payload) in &data.components {
            // Both drawable components name their texture the same way.
            let draws = matches!(
                type_name.as_str(),
                MeshComponent::TYPE_NAME | SpriteComponent::TYPE_NAME
            );
            let reference = draws.then(|| payload.get("texture")).flatten();
            if let Some(reference) = reference.and_then(serde_json::Value::as_str)
                && bindings.get(reference).is_none()
            {
                missing.insert(reference.to_owned());
            }
        }
    }
    missing
}
