//! What can stop Gather, on either host.

use sindri_platform::{AudioError, HostError};
use sindri_render::{FrameEncodeError, TextureError};
use sindri_scene::SheetBindError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatherError {
    #[error(transparent)]
    Scene(#[from] sindri_scene::SceneExtractError),
    #[error(transparent)]
    Physics(#[from] sindri_scene::PhysicsSyncError),
    #[error(transparent)]
    Document(#[from] sindri_core::SceneError),
    #[error(transparent)]
    World(#[from] sindri_core::WorldError),
    #[error(transparent)]
    Component(#[from] sindri_core::ComponentRegistryError),
    #[error(transparent)]
    Asset(#[from] sindri_core::AssetIdError),
    #[error(transparent)]
    Sheet(#[from] sindri_core::SheetError),
    #[error(transparent)]
    SheetBind(#[from] SheetBindError),
    #[error(transparent)]
    Decode(#[from] sindri_assets::AssetDecodeError),
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error(transparent)]
    Texture(#[from] TextureError),
    #[error(transparent)]
    Animation(#[from] sindri_scene::AnimationError),
    #[error(transparent)]
    Json(#[from] sindri_core::SceneJsonError),
    #[error(transparent)]
    Frame(#[from] FrameEncodeError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetQueue(#[from] sindri_assets::AssetLoadQueueCreateError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetLoader(#[from] sindri_assets::AssetLoaderError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    AssetLoad(#[from] sindri_core::AssetLoadError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    Manifest(#[from] sindri_assets::ManifestError),
    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    UrlRoot(#[from] sindri_assets::UrlRootError),
    #[cfg(target_arch = "wasm32")]
    #[error("browser project asset error: {0}")]
    BrowserAsset(String),
    #[error(transparent)]
    Host(#[from] Box<HostError<GatherError>>),
}

impl From<HostError<GatherError>> for GatherError {
    fn from(error: HostError<GatherError>) -> Self {
        Self::Host(Box::new(error))
    }
}
