//! Fetching a project over HTTP, one kind of asset at a time.
//!
//! Nothing is owned until the fetch source has returned it through
//! `AssetLoader` and the manifest has accepted the bytes, which is the
//! whole point: this proves the static-hosting path rather than proving
//! that `include_bytes!` works in WebAssembly.

use std::collections::BTreeMap;

use sindri_assets::{
    AssetDecoder, AssetKind, AssetLoadOutcome, AssetLoadQueueConfig, AssetLoader, AssetManifest,
    AudioAsset, AudioAssetDecoder, FetchAssetSource, FontAsset, FontAssetDecoder,
    MANIFEST_FILE_NAME, PrefabAssetDecoder, ProfileAssetDecoder, SceneAssetDecoder,
    SpriteSheetAssetDecoder, TextAssetDecoder, TextureAsset, TextureAssetDecoder,
    TileSetAssetDecoder,
};
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument, TileSetDocument};
use sindri_decay::{PrefabSources, ProfileSources, ScriptSources};
use weave::Stylesheet;

use crate::error::GatherError;

pub(super) struct BrowserProjectAssets {
    pub(super) scene: SceneDocument,
    pub(super) scripts: ScriptSources,
    pub(super) prefabs: PrefabSources,
    pub(super) profiles: ProfileSources,
    pub(super) textures: Vec<(AssetId, TextureAsset)>,
    pub(super) fonts: Vec<(AssetId, FontAsset)>,
    pub(super) audio: Vec<(AssetId, AudioAsset)>,
    pub(super) sheets: BTreeMap<String, SpriteSheetDocument>,
    pub(super) tile_sets: Vec<(AssetId, TileSetDocument)>,
    pub(super) stylesheets: Vec<Stylesheet>,
    pub(super) asset_count: usize,
}

pub(super) struct BrowserProjectLoader {
    phase: Option<LoadPhase>,
}

pub(super) enum LoadPhase {
    Manifest(AssetLoader<TextAssetDecoder>),
    Assets(Box<ProjectLoaders>),
}

impl BrowserProjectLoader {
    pub(super) fn new() -> Result<Self, GatherError> {
        let source = FetchAssetSource::new("assets")?;
        let mut manifest =
            AssetLoader::new(source, AssetLoadQueueConfig::default(), TextAssetDecoder)?;
        manifest.request(AssetId::new(MANIFEST_FILE_NAME)?)?;
        Ok(Self {
            phase: Some(LoadPhase::Manifest(manifest)),
        })
    }

    pub(super) fn poll(&mut self) -> Result<Option<BrowserProjectAssets>, GatherError> {
        let Some(phase) = self.phase.take() else {
            return Ok(None);
        };
        match phase {
            LoadPhase::Manifest(mut loader) => {
                poll_loader(&mut loader)?;
                let id = AssetId::new(MANIFEST_FILE_NAME)?;
                if let Some(text) = loader.get(&id) {
                    let manifest = AssetManifest::from_json(text)?;
                    self.phase = Some(LoadPhase::Assets(Box::new(ProjectLoaders::new(manifest)?)));
                } else {
                    self.phase = Some(LoadPhase::Manifest(loader));
                }
                Ok(None)
            }
            LoadPhase::Assets(mut loaders) => {
                if let Some(project) = loaders.poll()? {
                    Ok(Some(project))
                } else {
                    self.phase = Some(LoadPhase::Assets(loaders));
                    Ok(None)
                }
            }
        }
    }
}

pub(super) struct ProjectLoaders {
    scene: AssetLoader<SceneAssetDecoder>,
    scripts: AssetLoader<TextAssetDecoder>,
    textures: AssetLoader<TextureAssetDecoder>,
    fonts: AssetLoader<FontAssetDecoder>,
    audio: AssetLoader<AudioAssetDecoder>,
    sheets: AssetLoader<SpriteSheetAssetDecoder>,
    tile_sets: AssetLoader<TileSetAssetDecoder>,
    prefabs: AssetLoader<PrefabAssetDecoder>,
    profiles: AssetLoader<ProfileAssetDecoder>,
    styles: AssetLoader<TextAssetDecoder>,
    style_ids: Vec<String>,
    /// Kept, because what was asked for is also what has to be collected.
    manifest: AssetManifest,
}

impl ProjectLoaders {
    pub(super) fn new(manifest: AssetManifest) -> Result<Self, GatherError> {
        // Where the export put the assets, which the manifest names because it
        // is the one file that is never cached. A project served straight from
        // a source tree says nothing, and then the assets sit beside the
        // manifest as they always have.
        let root = if manifest.content_root().is_empty() {
            "assets".to_owned()
        } else {
            format!("assets/{}", manifest.content_root())
        };
        let source = FetchAssetSource::new(&root)?;
        let config = AssetLoadQueueConfig::default();
        let mut scene = AssetLoader::new(source.clone(), config, SceneAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut scripts = AssetLoader::new(source.clone(), config, TextAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut textures = AssetLoader::new(source.clone(), config, TextureAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut fonts = AssetLoader::new(source.clone(), config, FontAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut audio = AssetLoader::new(source.clone(), config, AudioAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut sheets = AssetLoader::new(source.clone(), config, SpriteSheetAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut tile_sets = AssetLoader::new(source.clone(), config, TileSetAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut prefabs = AssetLoader::new(source.clone(), config, PrefabAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut profiles = AssetLoader::new(source.clone(), config, ProfileAssetDecoder)?
            .with_manifest(manifest.clone());
        let mut styles =
            AssetLoader::new(source, config, TextAssetDecoder)?.with_manifest(manifest.clone());

        // From the manifest rather than from a list compiled into this binary.
        // Those lists were the thing that made a project's host something
        // somebody had to hand-write: adding a texture meant editing Rust, and
        // an export could not exist for a project this crate had never heard
        // of. The manifest says what a project is made of, so the host does not
        // have to be told.
        request_kind(&mut scene, &manifest, AssetKind::Scene)?;
        request_kind(&mut scripts, &manifest, AssetKind::Script)?;
        request_kind(&mut textures, &manifest, AssetKind::Texture)?;
        request_kind(&mut fonts, &manifest, AssetKind::Font)?;
        request_kind(&mut audio, &manifest, AssetKind::Audio)?;
        request_kind(&mut sheets, &manifest, AssetKind::Sheet)?;
        request_kind(&mut tile_sets, &manifest, AssetKind::TileSet)?;
        request_kind(&mut prefabs, &manifest, AssetKind::Prefab)?;
        request_kind(&mut profiles, &manifest, AssetKind::Profile)?;
        let style_ids = manifest
            .ids_of(AssetKind::Other)
            .filter(|id| id.as_str().ends_with(".weave"))
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>();
        for id in &style_ids {
            styles.request(AssetId::new(id.as_str())?)?;
        }

        Ok(Self {
            scene,
            scripts,
            prefabs,
            profiles,
            styles,
            style_ids,
            textures,
            fonts,
            audio,
            sheets,
            tile_sets,
            manifest,
        })
    }

    pub(super) fn poll(&mut self) -> Result<Option<BrowserProjectAssets>, GatherError> {
        poll_loader(&mut self.scene)?;
        poll_loader(&mut self.scripts)?;
        poll_loader(&mut self.textures)?;
        poll_loader(&mut self.fonts)?;
        poll_loader(&mut self.audio)?;
        poll_loader(&mut self.sheets)?;
        poll_loader(&mut self.tile_sets)?;
        poll_loader(&mut self.prefabs)?;
        poll_loader(&mut self.profiles)?;
        poll_loader(&mut self.styles)?;

        if self.scene.outstanding()
            + self.scripts.outstanding()
            + self.textures.outstanding()
            + self.fonts.outstanding()
            + self.audio.outstanding()
            + self.sheets.outstanding()
            + self.tile_sets.outstanding()
            + self.prefabs.outstanding()
            + self.profiles.outstanding()
            + self.styles.outstanding()
            != 0
        {
            return Ok(None);
        }

        let ids = |kind| {
            self.manifest
                .ids_of(kind)
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<String>>()
        };
        let scene_ids = ids(AssetKind::Scene);
        let Some(scene_id) = scene_ids.first() else {
            return Err(GatherError::MissingScene);
        };
        let scene = loaded(&self.scene, scene_id)?;
        let mut scripts = ScriptSources::new();
        for id in ids(AssetKind::Script) {
            let source = loaded(&self.scripts, &id)?;
            scripts.insert(id, source);
        }
        let mut prefabs = PrefabSources::new();
        for id in ids(AssetKind::Prefab) {
            let prefab = loaded(&self.prefabs, &id)?;
            prefabs.insert(id, prefab);
        }
        let mut profiles = ProfileSources::new();
        for id in ids(AssetKind::Profile) {
            let profile = loaded(&self.profiles, &id)?;
            profiles.insert(id, profile);
        }
        let textures = loaded_many(&self.textures, &ids(AssetKind::Texture))?;
        let fonts = loaded_many(&self.fonts, &ids(AssetKind::Font))?;
        let audio = loaded_many(&self.audio, &ids(AssetKind::Audio))?;
        let sheets = loaded_many(&self.sheets, &ids(AssetKind::Sheet))?
            .into_iter()
            .map(|(id, sheet)| (id.as_str().to_owned(), sheet))
            .collect();
        let tile_sets = loaded_many(&self.tile_sets, &ids(AssetKind::TileSet))?;
        let style_sources = self
            .style_ids
            .iter()
            .map(|id| loaded(&self.styles, id).map(|source| (id.clone(), source)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let stylesheets = weave::compose_all(&style_sources)
            .map_err(|error| GatherError::BrowserAsset(error.to_string()))?
            .into_iter()
            .map(|(_, sheet)| sheet)
            .collect();
        let asset_count = self.manifest.len();
        Ok(Some(BrowserProjectAssets {
            scene,
            scripts,
            prefabs,
            profiles,
            textures,
            fonts,
            audio,
            sheets,
            tile_sets,
            stylesheets,
            asset_count,
        }))
    }
}

/// Asks for every asset the manifest records of one kind.
pub(super) fn request_kind<D: AssetDecoder>(
    loader: &mut AssetLoader<D>,
    manifest: &AssetManifest,
    kind: AssetKind,
) -> Result<(), GatherError> {
    for id in manifest.ids_of(kind) {
        loader.request(id.clone())?;
    }
    Ok(())
}

pub(super) fn poll_loader<D: AssetDecoder>(loader: &mut AssetLoader<D>) -> Result<(), GatherError> {
    for outcome in loader.poll() {
        if let AssetLoadOutcome::Failed(error) = outcome {
            return Err(error.into());
        }
    }
    Ok(())
}

pub(super) fn loaded<D>(loader: &AssetLoader<D>, id: &str) -> Result<D::Asset, GatherError>
where
    D: AssetDecoder,
    D::Asset: Clone,
{
    let id = AssetId::new(id)?;
    loader
        .get(&id)
        .cloned()
        .ok_or_else(|| GatherError::BrowserAsset(format!("'{id}' completed without a value")))
}

pub(super) fn loaded_many<D>(
    loader: &AssetLoader<D>,
    ids: &[String],
) -> Result<Vec<(AssetId, D::Asset)>, GatherError>
where
    D: AssetDecoder,
    D::Asset: Clone,
{
    ids.iter()
        .map(|id| {
            let asset_id = AssetId::new(id.clone())?;
            let asset = loader.get(&asset_id).cloned().ok_or_else(|| {
                GatherError::BrowserAsset(format!("'{asset_id}' completed without a value"))
            })?;
            Ok((asset_id, asset))
        })
        .collect()
}
