//! What Gather is made of, by logical asset ID.
//!
//! Native builds embed the project so the standalone binary has no
//! working-directory requirement. Browser builds deliberately do not:
//! `browser` loads these same IDs through `FetchAssetSource` and
//! `AssetLoader`, which proves the static-hosting path rather than
//! proving only that `include_bytes!` works in WebAssembly.
//!
//! Adding content to the game is an entry in the list for its kind and
//! the file beside it.

#[cfg(not(target_arch = "wasm32"))]
use sindri_assets::{
    AssetBytes, AssetDecoder, AudioAssetDecoder, FontAssetDecoder, TextureAssetDecoder,
};
use sindri_core::World;
#[cfg(not(target_arch = "wasm32"))]
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument, sheet_id_for};
use sindri_decay::ScriptComponent;
#[cfg(not(target_arch = "wasm32"))]
use sindri_decay::ScriptSources;
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{AudioBackend, AudioClip};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{TextRenderer, Texture2D, TextureRegistry};
use sindri_scene::SceneExtractor;
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::TextureBindings;

use crate::error::GatherError;

/// The composed presentation sources embedded by the native game.
#[cfg(not(target_arch = "wasm32"))]
pub const WEAVE_SOURCES: &[(&str, &str)] = &[
    ("ui/gather.weave", include_str!("../assets/ui/gather.weave")),
    ("ui/hud.weave", include_str!("../assets/ui/hud.weave")),
    (
        "ui/completion.weave",
        include_str!("../assets/ui/completion.weave"),
    ),
];

/// Every texture Gather draws, by ID.
///
/// The native build embeds the bytes in `TEXTURES`; this is the same set as
/// IDs, and `the_browser_fetches_every_texture_the_native_build_embeds` asserts
/// the two agree — a texture added to one and not the other is a game that
/// looks right on one target and wrong on the other, with nothing failing.
///
/// Native-only, and the `cfg` is the point rather than an accident. The browser
/// takes its textures from the project manifest, which is what lets one host
/// serve any project; it does not read this list and must not, because a list
/// of one game's textures compiled into a generic host is a list that is wrong
/// for every other game. It did read this list once, to decide which textures
/// got their sprite sheets, and every project that was not Gather silently got
/// none.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURE_IDS: &[&str] = &[
    "textures/tiles.png",
    "textures/orb.png",
    "textures/player.png",
    "textures/pip.png",
    "textures/banner.png",
    "textures/shrine.png",
    "textures/standing-stones.png",
    "textures/waystone.png",
    "textures/stone-wall.png",
    "textures/tree.png",
];

/// The scene and scripts are embedded only in native builds.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SCENE: &str = include_str!("../assets/gather.scene.json");

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SCRIPTS: &[(&str, &str)] = &[
    (
        "scripts/player.decay",
        include_str!("../assets/scripts/player.decay"),
    ),
    (
        "scripts/wisp.decay",
        include_str!("../assets/scripts/wisp.decay"),
    ),
    (
        "scripts/ambient.decay",
        include_str!("../assets/scripts/ambient.decay"),
    ),
    (
        "scripts/orb.decay",
        include_str!("../assets/scripts/orb.decay"),
    ),
    (
        "scripts/pip.decay",
        include_str!("../assets/scripts/pip.decay"),
    ),
    (
        "scripts/banner.decay",
        include_str!("../assets/scripts/banner.decay"),
    ),
];

/// Native art bytes used by the standalone game and capture tests.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURES: &[(&str, &[u8])] = &[
    (
        "textures/tiles.png",
        include_bytes!("../assets/textures/tiles.png"),
    ),
    (
        "textures/orb.png",
        include_bytes!("../assets/textures/orb.png"),
    ),
    (
        "textures/player.png",
        include_bytes!("../assets/textures/player.png"),
    ),
    (
        "textures/pip.png",
        include_bytes!("../assets/textures/pip.png"),
    ),
    (
        "textures/banner.png",
        include_bytes!("../assets/textures/banner.png"),
    ),
    // Baked by `tools/isometric-baker` from the recipes beside them. The
    // recipe is the source; the PNG and its sheet are derived, and re-baking
    // rewrites them in place.
    (
        "textures/shrine.png",
        include_bytes!("../assets/textures/shrine.png"),
    ),
    (
        "textures/standing-stones.png",
        include_bytes!("../assets/textures/standing-stones.png"),
    ),
    (
        "textures/waystone.png",
        include_bytes!("../assets/textures/waystone.png"),
    ),
    (
        "textures/stone-wall.png",
        include_bytes!("../assets/textures/stone-wall.png"),
    ),
    (
        "textures/tree.png",
        include_bytes!("../assets/textures/tree.png"),
    ),
];

/// Native project-owned typefaces.
#[cfg(not(target_arch = "wasm32"))]
pub const FONTS: &[(&str, &[u8])] = &[(
    "fonts/Inter.ttf",
    include_bytes!("../assets/fonts/Inter.ttf"),
)];

/// Native sounds. Browser builds fetch the same IDs instead.
#[cfg(not(target_arch = "wasm32"))]
pub const AUDIO: &[(&str, &[u8])] = &[
    (
        "audio/background.wav",
        include_bytes!("../assets/audio/background.wav"),
    ),
    (
        "audio/pickup.wav",
        include_bytes!("../assets/audio/pickup.wav"),
    ),
    (
        "audio/victory.wav",
        include_bytes!("../assets/audio/victory.wav"),
    ),
];

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_fonts(renderer: &mut TextRenderer) -> Result<(), GatherError> {
    for (id, bytes) in FONTS {
        let asset = FontAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        renderer.bind_font(*id, asset.family(), asset.bytes().to_vec());
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_audio(audio: &mut dyn AudioBackend) -> Result<(), GatherError> {
    for (id, bytes) in AUDIO {
        let asset = AudioAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        let mime = asset.format().mime_type();
        audio.register(AudioClip::new(*id, asset.into_bytes(), mime))?;
    }
    Ok(())
}

/// How each sliced native texture is cut, shipped beside it.
#[cfg(not(target_arch = "wasm32"))]
pub const SHEETS: &[(&str, &str)] = &[
    (
        "textures/tiles.sheet.json",
        include_str!("../assets/textures/tiles.sheet.json"),
    ),
    (
        "textures/orb.sheet.json",
        include_str!("../assets/textures/orb.sheet.json"),
    ),
    (
        "textures/pip.sheet.json",
        include_str!("../assets/textures/pip.sheet.json"),
    ),
    (
        "textures/player.sheet.json",
        include_str!("../assets/textures/player.sheet.json"),
    ),
    (
        "textures/shrine.sheet.json",
        include_str!("../assets/textures/shrine.sheet.json"),
    ),
    (
        "textures/standing-stones.sheet.json",
        include_str!("../assets/textures/standing-stones.sheet.json"),
    ),
    (
        "textures/waystone.sheet.json",
        include_str!("../assets/textures/waystone.sheet.json"),
    ),
    (
        "textures/stone-wall.sheet.json",
        include_str!("../assets/textures/stone-wall.sheet.json"),
    ),
    (
        "textures/tree.sheet.json",
        include_str!("../assets/textures/tree.sheet.json"),
    ),
];

/// Every native texture on the GPU, and every sheet bound to what it cuts.
#[cfg(not(target_arch = "wasm32"))]
pub fn bind_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(TextureRegistry, TextureBindings), GatherError> {
    let mut textures = TextureRegistry::new(device, queue);
    let mut bindings = TextureBindings::new();
    for (id, bytes) in TEXTURES {
        let asset = TextureAssetDecoder.decode(AssetBytes::new(
            (*id).parse::<AssetId>()?,
            (*bytes).to_vec(),
        ))?;
        let texture = Texture2D::from_rgba8(
            device,
            queue,
            id,
            asset.width(),
            asset.height(),
            asset.rgba8(),
        )?;
        bindings.bind(*id, textures.insert(texture));

        let Some(sheet) = (*id)
            .parse::<AssetId>()
            .ok()
            .and_then(|id| sheet_id_for(&id))
        else {
            continue;
        };
        let Some((_, json)) = SHEETS.iter().find(|(name, _)| *name == sheet.as_str()) else {
            continue;
        };
        bindings.bind_sheet(*id, &SpriteSheetDocument::from_json(json)?)?;
    }
    Ok((textures, bindings))
}

/// The scene's component schemas, including the one the engine does not know.
pub fn extractor() -> Result<SceneExtractor, GatherError> {
    let mut extractor = SceneExtractor::new()?;
    extractor.register::<ScriptComponent>("Script")?;
    Ok(extractor)
}

/// The embedded native scene as a world, ready to run.
#[cfg(not(target_arch = "wasm32"))]
pub fn world() -> Result<World, GatherError> {
    let document = SceneDocument::from_json(SCENE)?;
    Ok(World::from_scene(&document)?.world)
}

/// The native equivalent of the stylesheet graph the browser fetches.
#[cfg(not(target_arch = "wasm32"))]
pub fn stylesheets() -> Result<Vec<weave::Stylesheet>, GatherError> {
    let sources = WEAVE_SOURCES
        .iter()
        .map(|(id, source)| ((*id).to_owned(), (*source).to_owned()))
        .collect();
    let sheet = weave::compose("ui/gather.weave", &sources)
        .map_err(|error| GatherError::Weave(error.to_string()))?;
    Ok(vec![sheet])
}

/// Resolve presentation without changing the authored or gameplay world.
pub fn presented_world(
    authored: &World,
    stylesheets: &[weave::Stylesheet],
    viewport: weave::Viewport,
) -> Result<World, GatherError> {
    let mut world = authored.clone();
    for stylesheet in stylesheets {
        world = sindri_weave::PresentationWorld::resolve(&world, stylesheet, viewport)
            .map_err(|error| GatherError::Weave(error.to_string()))?
            .world()
            .clone();
    }
    Ok(world)
}

/// The embedded native scripts, keyed by the IDs the scene names.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn sources() -> ScriptSources {
    let mut sources = ScriptSources::new();
    for (id, text) in SCRIPTS {
        sources.insert(*id, *text);
    }
    sources
}
