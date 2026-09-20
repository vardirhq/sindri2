//! What Causeway is made of, by logical asset ID.
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
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument, TileSetDocument, sheet_id_for};
use sindri_decay::ScriptComponent;
#[cfg(not(target_arch = "wasm32"))]
use sindri_decay::ScriptSources;
#[cfg(not(target_arch = "wasm32"))]
use sindri_platform::{AudioBackend, AudioClip};
#[cfg(not(target_arch = "wasm32"))]
use sindri_render::{TextRenderer, Texture2D, TextureRegistry};
use sindri_scene::SceneExtractor;
#[cfg(not(target_arch = "wasm32"))]
use sindri_scene::{TextureBindings, TileSetBindings};

use crate::error::CausewayError;

/// The composed presentation sources embedded by the native game.
#[cfg(not(target_arch = "wasm32"))]
pub const WEAVE_SOURCES: &[(&str, &str)] = &[
    (
        "ui/causeway.weave",
        include_str!("../assets/ui/causeway.weave"),
    ),
    ("ui/hud.weave", include_str!("../assets/ui/hud.weave")),
];

/// Every texture Causeway draws, by ID.
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
/// got their sprite sheets, and every project that was not the companion game got
/// none.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURE_IDS: &[&str] = &[
    "textures/blocks-top.png",
    "textures/blocks-side.png",
    "textures/slabs-side.png",
    "textures/log-bark.png",
    "textures/log-end.png",
    "textures/leaves.png",
    "textures/wanderer.png",
    "textures/beacon.png",
];

/// The scenes and scripts are embedded only in native builds.
///
/// Every scene the project declares, the one it opens on first, which is the
/// order `sindri.toml` lists them in and the order the session enters them.
/// A game with interiors reaches them by name, so all of them have to be here
/// — a browser build fetches the same IDs through the real asset pipeline.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SCENES: &[(&str, &str)] = &[(
    "causeway.scene.json",
    include_str!("../assets/causeway.scene.json"),
)];

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const SCRIPTS: &[(&str, &str)] = &[
    (
        "scripts/builder.decay",
        include_str!("../assets/scripts/builder.decay"),
    ),
    (
        "scripts/wanderer.decay",
        include_str!("../assets/scripts/wanderer.decay"),
    ),
    (
        "scripts/beacon.decay",
        include_str!("../assets/scripts/beacon.decay"),
    ),
    (
        "scripts/hud.decay",
        include_str!("../assets/scripts/hud.decay"),
    ),
    (
        "scripts/camera-follow.decay",
        include_str!("../assets/scripts/camera-follow.decay"),
    ),
    (
        "scripts/modes.decay",
        include_str!("../assets/scripts/modes.decay"),
    ),
    (
        "scripts/mode-button.decay",
        include_str!("../assets/scripts/mode-button.decay"),
    ),
];

/// Native art bytes used by the standalone game and capture tests.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXTURES: &[(&str, &[u8])] = &[
    // Baked by `tools/isometric-baker` from the recipes in `recipes/`. The
    // recipe is the source; the PNG and its sheet are derived, and re-baking
    // rewrites them in place.
    (
        "textures/blocks-top.png",
        include_bytes!("../assets/textures/blocks-top.png"),
    ),
    (
        "textures/blocks-side.png",
        include_bytes!("../assets/textures/blocks-side.png"),
    ),
    (
        "textures/slabs-side.png",
        include_bytes!("../assets/textures/slabs-side.png"),
    ),
    (
        "textures/log-bark.png",
        include_bytes!("../assets/textures/log-bark.png"),
    ),
    (
        "textures/log-end.png",
        include_bytes!("../assets/textures/log-end.png"),
    ),
    (
        "textures/leaves.png",
        include_bytes!("../assets/textures/leaves.png"),
    ),
    (
        "textures/wanderer.png",
        include_bytes!("../assets/textures/wanderer.png"),
    ),
    (
        "textures/beacon.png",
        include_bytes!("../assets/textures/beacon.png"),
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
        "audio/victory.wav",
        include_bytes!("../assets/audio/victory.wav"),
    ),
];

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_fonts(renderer: &mut TextRenderer) -> Result<(), CausewayError> {
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
pub fn bind_audio(audio: &mut dyn AudioBackend) -> Result<(), CausewayError> {
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
        "textures/blocks-top.sheet.json",
        include_str!("../assets/textures/blocks-top.sheet.json"),
    ),
    (
        "textures/blocks-side.sheet.json",
        include_str!("../assets/textures/blocks-side.sheet.json"),
    ),
    (
        "textures/slabs-side.sheet.json",
        include_str!("../assets/textures/slabs-side.sheet.json"),
    ),
    (
        "textures/wanderer.sheet.json",
        include_str!("../assets/textures/wanderer.sheet.json"),
    ),
    (
        "textures/beacon.sheet.json",
        include_str!("../assets/textures/beacon.sheet.json"),
    ),
];

/// Semantic block sets embedded by the native game.
#[cfg(not(target_arch = "wasm32"))]
pub const TILE_SETS: &[(&str, &str)] = &[(
    "causeway.tileset.json",
    include_str!("../assets/causeway.tileset.json"),
)];

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_tile_sets() -> Result<TileSetBindings, CausewayError> {
    let mut bindings = TileSetBindings::new();
    for (id, json) in TILE_SETS {
        bindings.bind(*id, TileSetDocument::from_json(json)?)?;
    }
    Ok(bindings)
}

/// Every native texture on the GPU, and every sheet bound to what it cuts.
#[cfg(not(target_arch = "wasm32"))]
pub fn bind_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(TextureRegistry, TextureBindings), CausewayError> {
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
pub fn extractor() -> Result<SceneExtractor, CausewayError> {
    let mut extractor = SceneExtractor::new()?;
    extractor.register::<ScriptComponent>("Script")?;
    Ok(extractor)
}

/// Every embedded scene, parsed, in the order a session should enter them.
#[cfg(not(target_arch = "wasm32"))]
pub fn scenes() -> Result<Vec<(String, SceneDocument)>, CausewayError> {
    SCENES
        .iter()
        .map(|(id, json)| Ok(((*id).to_owned(), SceneDocument::from_json(json)?)))
        .collect()
}

/// A world with the opening scene in it, ready to run.
///
/// Loaded through [`sindri_core::LoadedScenes`] rather than straight into the
/// world, so the opening scene is a scene like any other: it sits under a root
/// that can be switched off when the player walks somewhere else. A world whose
/// first scene had been poured in flat would be the one place a game could
/// never leave.
#[cfg(not(target_arch = "wasm32"))]
pub fn world() -> Result<(World, sindri_core::LoadedScenes), CausewayError> {
    let mut world = World::default();
    let mut loaded = sindri_core::LoadedScenes::new();
    let scenes = scenes()?;
    let (name, document) = scenes.first().ok_or(CausewayError::MissingScene)?;
    // The opening scene keeps the identities its file spells: everything that
    // names an entity by stable ID -- a test, an editor showing a running
    // world, an authoring proposal -- was written against those.
    loaded.enter_keeping_identities(&mut world, name, document)?;
    fill_the_world(&mut world)?;
    Ok((world, loaded))
}

/// Builds the ground the scene left empty.
///
/// The scene authors the *grid* -- how big a cell is, how many of them, where
/// the floor stands -- and leaves the cells to this. Only the opening window
/// is materialized; the session adds deterministic chunks as its camera moves.
///
/// Done here rather than in a script because it has to be true before anything
/// else looks: placement asks how high the ground is on the first pass, and a
/// script filling the volume afterwards would have put every prop at the
/// height of a world that did not exist yet.
///
/// Both hosts call it, and they have to. The browser does not load the scene
/// through `world` -- it fetches the project through the real asset pipeline,
/// which is the point of that path -- so a generator reachable only from the
/// native loader is a browser build that opens onto an empty grid, with
/// nothing anywhere reporting it.
pub(crate) fn fill_the_world(world: &mut World) -> Result<(), CausewayError> {
    let Some(entity) = world
        .entities()
        .find(|(_, data)| data.components.contains_key(TILE_GRID))
        .map(|(entity, _)| entity)
    else {
        return Ok(());
    };
    let focus = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Wanderer"))
        .and_then(|(_, data)| data.transform_3d)
        .map_or([crate::streaming::WORLD_CENTRE; 2], |transform| {
            #[allow(clippy::cast_possible_truncation)]
            [
                transform.position[0].round() as i32,
                transform.position[2].round() as i32,
            ]
        });
    let volume = crate::streaming::initial_volume(focus);
    let payload = serde_json::to_value(&volume)
        .map_err(|error| CausewayError::Generated(error.to_string()))?;
    if let Some(data) = world.get_mut(entity) {
        data.components.insert(TILE_VOLUME.to_owned(), payload);
    }
    Ok(())
}

const TILE_GRID: &str = "sindri.tile_grid";
const TILE_VOLUME: &str = "sindri.tile_volume";

/// The native equivalent of the stylesheet graph the browser fetches.
#[cfg(not(target_arch = "wasm32"))]
pub fn stylesheets() -> Result<Vec<weave::Stylesheet>, CausewayError> {
    let sources = WEAVE_SOURCES
        .iter()
        .map(|(id, source)| ((*id).to_owned(), (*source).to_owned()))
        .collect();
    let sheet = weave::compose("ui/causeway.weave", &sources)
        .map_err(|error| CausewayError::Weave(error.to_string()))?;
    Ok(vec![sheet])
}

/// Resolve presentation without changing the authored or gameplay world.
pub fn presented_world(
    authored: &World,
    stylesheets: &[weave::Stylesheet],
    viewport: weave::Viewport,
) -> Result<World, CausewayError> {
    let mut world = authored.clone();
    for stylesheet in stylesheets {
        world = sindri_weave::PresentationWorld::resolve(&world, stylesheet, viewport)
            .map_err(|error| CausewayError::Weave(error.to_string()))?
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
