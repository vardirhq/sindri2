use std::collections::{BTreeMap, BTreeSet};

use sindri_core::{SceneComponent, SpriteRef, SpriteSheetDocument, World};
use sindri_render::{TextureId, TextureRegistry, UvRect};
use thiserror::Error;

use crate::{
    MeshComponent, SpriteAnimationComponent, SpriteComponent, TilemapComponent, UiImageComponent,
    UiTextComponent,
};

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
    /// How each texture is sliced, by the same reference key. Held here for
    /// the reason the handles are: a scene says `tiles.png#floor`, the renderer
    /// knows a handle and a rect, and this is the only place that knows both.
    sheets: BTreeMap<String, BTreeMap<String, UvRect>>,
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

    /// Records how `texture` is cut into named sprites.
    ///
    /// Rects are checked here, once, rather than everywhere one is used: a
    /// sheet arrives as authored numbers and leaves as `UvRect`s or as an
    /// error naming the sprite that is wrong.
    pub fn bind_sheet(
        &mut self,
        texture: impl Into<String>,
        sheet: &SpriteSheetDocument,
    ) -> Result<(), SheetBindError> {
        let texture = texture.into();
        let authored = sheet.rects().map_err(|error| SheetBindError::Sheet {
            texture: texture.clone(),
            message: error.to_string(),
        })?;
        let mut rects = BTreeMap::new();
        for (name, [x, y, width, height]) in authored {
            let rect = UvRect::new(x, y, width, height).map_err(|error| SheetBindError::Rect {
                texture: texture.clone(),
                sprite: name.clone(),
                message: error.to_string(),
            })?;
            rects.insert(name, rect);
        }
        self.sheets.insert(texture, rects);
        Ok(())
    }

    /// Forgets how `texture` is cut, leaving its sprites unresolved.
    pub fn unbind_sheet(&mut self, texture: &str) {
        self.sheets.remove(texture);
    }

    /// The part of the texture `reference` names, or `None` when it names the
    /// whole image.
    ///
    /// `Some(None)` is not a case: a reference with no fragment wants the whole
    /// image and gets [`UvRect::FULL`]; one with a fragment that no loaded sheet
    /// names is *unresolved*, and is reported by [`unresolved_sprites`] rather
    /// than quietly drawing the whole sheet — a sheet drawn whole is every
    /// sprite at once, which is the picture that made this worth doing.
    pub fn sprite_rect(&self, reference: &SpriteRef) -> Option<UvRect> {
        let Some(name) = reference.sprite() else {
            return Some(UvRect::FULL);
        };
        self.sheets.get(reference.texture())?.get(name).copied()
    }

    /// The handle and the rect to draw `reference` with.
    ///
    /// Both fall back to something visibly wrong rather than failing the frame,
    /// which is the rule an unbound texture has always followed.
    pub fn resolve_sprite(&self, reference: &SpriteRef) -> (TextureId, UvRect) {
        (
            self.resolve(reference.texture()),
            self.sprite_rect(reference).unwrap_or(UvRect::FULL),
        )
    }

    /// One named sprite of a sheet, without building a reference for it.
    ///
    /// What a tilemap resolves its palette through: a map of 49 tiles names at
    /// most a handful of sprites, so the names are resolved once and the cells
    /// index the answers.
    pub fn sheet_sprite(&self, texture: &str, sprite: &str) -> Option<UvRect> {
        self.sheets.get(texture)?.get(sprite).copied()
    }

    /// Whether `texture` has a sheet bound.
    pub fn has_sheet(&self, texture: &str) -> bool {
        self.sheets.contains_key(texture)
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

/// What is wrong with a sheet a host tried to bind.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SheetBindError {
    #[error("the sheet slicing {texture} is not usable: {message}")]
    Sheet { texture: String, message: String },
    #[error("sprite `{sprite}` of {texture} is not a usable rect: {message}")]
    Rect {
        texture: String,
        sprite: String,
        message: String,
    },
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
    sprite_references(world)
        .into_iter()
        .map(|reference| reference.texture().to_owned())
        .collect()
}

/// Every project font a world uses for text.
///
/// Font references are gathered independently of textures because the two are
/// decoded and bound by different renderers, even though they share the same
/// project asset loader and manifest.
pub fn referenced_fonts(world: &World) -> BTreeSet<String> {
    world
        .entities()
        .flat_map(|(_, data)| {
            FONT_NAMING_COMPONENTS
                .iter()
                .filter_map(|type_name| data.components.get(*type_name))
        })
        .filter_map(|payload| payload.get("font"))
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Every sheet a world needs loaded to draw what it names.
///
/// Only textures used *with a fragment* appear: a reference to a whole image
/// needs no sheet, so an unsliced texture is never asked for a sidecar that does
/// not exist. That is what keeps a missing sheet an error worth reporting
/// rather than the ordinary case.
pub fn referenced_sheets(world: &World) -> BTreeSet<String> {
    let mut sheets: BTreeSet<String> = sprite_references(world)
        .into_iter()
        .filter_map(|reference| reference.sheet())
        .map(|id| id.as_str().to_owned())
        .collect();

    // An animated sprite is the exception, and it has to be: its own reference
    // carries no fragment, because which part it draws is the clip's business
    // and changes every few frames. So the *clips* are what need the sheet, and
    // nothing about the sprite's reference says so.
    //
    // Missing this drew every frame of a sheet at once — the sprite resolved
    // its whole texture, because the sheet that would have named the frame was
    // never asked for. The game caught it and the editor showed it.
    for (_, data) in world.entities() {
        if !data
            .components
            .contains_key(SpriteAnimationComponent::TYPE_NAME)
        {
            continue;
        }
        let sheet = data
            .components
            .get(SpriteComponent::TYPE_NAME)
            .and_then(|payload| payload.get("texture"))
            .and_then(serde_json::Value::as_str)
            .and_then(|texture| SpriteRef::parse(texture).ok())
            .and_then(|reference| sindri_core::sheet_id_for(&reference.asset()?));
        if let Some(sheet) = sheet {
            sheets.insert(sheet.as_str().to_owned());
        }
    }
    sheets
}

/// Every sprite reference a world draws with, parsed.
///
/// A reference that will not parse is dropped here rather than reported: it is
/// reported where it is drawn, by the extractor, with the entity attached.
fn sprite_references(world: &World) -> BTreeSet<SpriteRef> {
    let mut referenced = BTreeSet::new();
    for (_, data) in world.entities() {
        for (type_name, payload) in &data.components {
            if !TEXTURE_NAMING_COMPONENTS.contains(&type_name.as_str()) {
                continue;
            }
            let Some(texture) = payload.get("texture").and_then(serde_json::Value::as_str) else {
                continue;
            };
            // A tilemap names its sprites in a palette rather than in the
            // reference, so its texture is asked for every one of them.
            let palette: Vec<&str> = payload
                .get("palette")
                .and_then(serde_json::Value::as_array)
                .map(|names| names.iter().filter_map(serde_json::Value::as_str).collect())
                .unwrap_or_default();
            if palette.is_empty() {
                if let Ok(reference) = SpriteRef::parse(texture) {
                    referenced.insert(reference);
                }
                continue;
            }
            for sprite in palette {
                if let Ok(reference) = SpriteRef::parse(&format!("{texture}#{sprite}")) {
                    referenced.insert(reference);
                }
            }
        }
    }
    referenced
}

/// Every built-in component that names a texture, which is the list hosts
/// load from.
///
/// A drawable component missing from here is not a compile error and not a
/// failed frame: its texture is simply never requested, so it never binds, and
/// the thing draws as the magenta checker. `sindri.tilemap` did exactly that
/// for the length of one commit. The test below is what stops the next one —
/// it holds this list against the schema registry, so a component whose payload
/// carries a texture has to be named here or fail the build.
pub const TEXTURE_NAMING_COMPONENTS: &[&str] = &[
    MeshComponent::TYPE_NAME,
    SpriteComponent::TYPE_NAME,
    TilemapComponent::TYPE_NAME,
    UiImageComponent::TYPE_NAME,
];

/// Every built-in component that names a project font, which is the list hosts
/// load from.
///
/// The same trap as `TEXTURE_NAMING_COMPONENTS`, one renderer along: a
/// component missing from here never has its font requested, so nothing binds
/// it and the text simply does not draw — a blank where a label should be, with
/// no failed frame to point at it. The test beside the one above holds this
/// list against the registry too, though it has to ask differently: a
/// font-naming component has no default payload to be read — it cannot invent a
/// font — so the test probes each validator instead, and a component that
/// deserializes a `font` has to be named here or fail the build.
pub const FONT_NAMING_COMPONENTS: &[&str] = &[UiTextComponent::TYPE_NAME];

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

/// Every named sprite a world draws that no loaded sheet places.
///
/// Reported by name for the reason an unbound texture is: the frame still
/// draws, so without this the only clue would be a picture that is subtly the
/// wrong part of an image.
pub fn unresolved_sprites(world: &World, bindings: &TextureBindings) -> BTreeSet<String> {
    sprite_references(world)
        .into_iter()
        .filter(|reference| reference.sprite().is_some())
        .filter(|reference| bindings.sprite_rect(reference).is_none())
        .map(|reference| reference.to_string())
        .collect()
}
