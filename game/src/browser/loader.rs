//! Fetching a project over HTTP, one kind of asset at a time.
//!
//! Nothing is owned until the fetch source has returned it through
//! `AssetLoader` and the manifest has accepted the bytes, which is the
//! whole point: this proves the static-hosting path rather than proving
//! that `include_bytes!` works in WebAssembly.

use std::collections::BTreeMap;

use sindri_assets::{
    AssetDecoder, AssetLoadOutcome, AssetLoadQueueConfig, AssetLoader, AssetManifest, AudioAsset,
    AudioAssetDecoder, FetchAssetSource, FontAsset, FontAssetDecoder, MANIFEST_FILE_NAME,
    SceneAssetDecoder, SpriteSheetAssetDecoder, TextAssetDecoder, TextureAsset,
    TextureAssetDecoder,
};
use sindri_core::{AssetId, SceneDocument, SpriteSheetDocument};
use sindri_decay::ScriptSources;

use crate::assets::{AUDIO_IDS, FONT_IDS, SCENE_ID, SCRIPT_IDS, SHEET_IDS, TEXTURE_IDS};
use crate::error::GatherError;

pub(super) struct BrowserProjectAssets {
    pub(super) scene: SceneDocument,
    pub(super) scripts: ScriptSources,
    pub(super) textures: Vec<(AssetId, TextureAsset)>,
    pub(super) fonts: Vec<(AssetId, FontAsset)>,
    pub(super) audio: Vec<(AssetId, AudioAsset)>,
    pub(super) sheets: BTreeMap<String, SpriteSheetDocument>,
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
}

impl ProjectLoaders {
    pub(super) fn new(manifest: AssetManifest) -> Result<Self, GatherError> {
        let source = FetchAssetSource::new("assets")?;
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
        let mut sheets =
            AssetLoader::new(source, config, SpriteSheetAssetDecoder)?.with_manifest(manifest);

        request(&mut scene, &[SCENE_ID])?;
        request(&mut scripts, SCRIPT_IDS)?;
        request(&mut textures, TEXTURE_IDS)?;
        request(&mut fonts, FONT_IDS)?;
        request(&mut audio, AUDIO_IDS)?;
        request(&mut sheets, SHEET_IDS)?;

        Ok(Self {
            scene,
            scripts,
            textures,
            fonts,
            audio,
            sheets,
        })
    }

    pub(super) fn poll(&mut self) -> Result<Option<BrowserProjectAssets>, GatherError> {
        poll_loader(&mut self.scene)?;
        poll_loader(&mut self.scripts)?;
        poll_loader(&mut self.textures)?;
        poll_loader(&mut self.fonts)?;
        poll_loader(&mut self.audio)?;
        poll_loader(&mut self.sheets)?;

        if self.scene.outstanding()
            + self.scripts.outstanding()
            + self.textures.outstanding()
            + self.fonts.outstanding()
            + self.audio.outstanding()
            + self.sheets.outstanding()
            != 0
        {
            return Ok(None);
        }

        let scene = loaded(&self.scene, SCENE_ID)?;
        let mut scripts = ScriptSources::new();
        for id in SCRIPT_IDS {
            scripts.insert(*id, loaded(&self.scripts, id)?);
        }
        let textures = loaded_many(&self.textures, TEXTURE_IDS)?;
        let fonts = loaded_many(&self.fonts, FONT_IDS)?;
        let audio = loaded_many(&self.audio, AUDIO_IDS)?;
        let sheets = loaded_many(&self.sheets, SHEET_IDS)?
            .into_iter()
            .map(|(id, sheet)| (id.as_str().to_owned(), sheet))
            .collect();
        let asset_count = 1
            + SCRIPT_IDS.len()
            + TEXTURE_IDS.len()
            + FONT_IDS.len()
            + AUDIO_IDS.len()
            + SHEET_IDS.len();
        Ok(Some(BrowserProjectAssets {
            scene,
            scripts,
            textures,
            fonts,
            audio,
            sheets,
            asset_count,
        }))
    }
}

pub(super) fn request<D: AssetDecoder>(
    loader: &mut AssetLoader<D>,
    ids: &[&str],
) -> Result<(), GatherError> {
    for id in ids {
        loader.request(AssetId::new(*id)?)?;
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
    ids: &[&str],
) -> Result<Vec<(AssetId, D::Asset)>, GatherError>
where
    D: AssetDecoder,
    D::Asset: Clone,
{
    ids.iter()
        .map(|id| {
            let asset_id = AssetId::new(*id)?;
            let asset = loader.get(&asset_id).cloned().ok_or_else(|| {
                GatherError::BrowserAsset(format!("'{asset_id}' completed without a value"))
            })?;
            Ok((asset_id, asset))
        })
        .collect()
}
