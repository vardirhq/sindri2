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

    /// Unbinds `reference`, returning the handle it held.
    ///
    /// What a host calls when a texture is released: the reference goes back to
    /// resolving as missing, which is visibly wrong, rather than continuing to
    /// resolve to a handle whose texture is gone.
    pub fn unbind(&mut self, reference: &str) -> Option<TextureId> {
        self.bound.remove(reference)
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

/// A texture a scene names that the engine generates rather than loads.
///
/// The `procedural:` prefix is not decoration: a colon is a reserved delimiter
/// in an `AssetId`, so a procedural reference cannot be parsed as one. That is
/// what keeps the two kinds of reference apart without a rule anybody has to
/// remember — a reference a loader can parse is a file to fetch, and one it
/// cannot is the engine's to produce or nobody's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProceduralTexture {
    /// What a scene writes to ask for it.
    pub reference: &'static str,
    /// Pixels along each edge.
    pub size: u32,
    /// Cells along each edge.
    pub cells: u32,
    /// The two colours it alternates, as non-premultiplied sRGB with alpha.
    pub colors: [[u8; 4]; 2],
}

/// Every texture the engine generates, in the form a renderer needs to make one.
///
/// One table rather than a copy per host: the demo capture verifies these exact
/// colours in a rendered image, and the editor draws the same scene, so two
/// hosts choosing their own navy would be a difference nothing would catch until
/// a screenshot looked wrong.
pub const PROCEDURAL_TEXTURES: [ProceduralTexture; 1] = [ProceduralTexture {
    reference: "procedural:checkerboard",
    size: 64,
    cells: 8,
    colors: [[18, 34, 55, 255], [240, 114, 43, 255]],
}];

/// Every texture a world draws with.
///
/// This is the list a host loads: a scene's texture references are the only
/// statement anywhere of what a scene needs, and until something asked for them
/// the editor bound a fixed pair and drew the missing checker for everything
/// else. Deduplicated, because twenty entities naming one texture is one load.
pub fn referenced_textures(world: &World) -> BTreeSet<String> {
    let mut referenced = BTreeSet::new();
    for (_, data) in world.entities() {
        for (type_name, payload) in &data.components {
            // Both drawable components name their texture the same way.
            let draws = matches!(
                type_name.as_str(),
                MeshComponent::TYPE_NAME | SpriteComponent::TYPE_NAME
            );
            let reference = draws.then(|| payload.get("texture")).flatten();
            if let Some(reference) = reference.and_then(serde_json::Value::as_str) {
                referenced.insert(reference.to_owned());
            }
        }
    }
    referenced
}

/// Every texture a world draws with that nothing has bound.
///
/// Hosts call this after loading to report what is missing by name, rather than
/// leaving a magenta surface as the only clue.
pub fn unresolved_textures(world: &World, bindings: &TextureBindings) -> BTreeSet<String> {
    referenced_textures(world)
        .into_iter()
        .filter(|reference| bindings.get(reference).is_none())
        .collect()
}
