//! Public facade for the Sindri game engine.
//!
//! Feature crates will be re-exported here as they become stable. Keeping a
//! facade prevents games from depending on Sindri's internal crate layout.

pub use sindri_assets as assets;
pub use sindri_core as core;

#[cfg(feature = "render")]
pub use sindri_gpu as gpu;
#[cfg(feature = "render")]
pub use sindri_render as render;

pub mod prelude {
    pub use sindri_assets::{
        AssetDecoder, AssetLoadCompletion, AssetLoadQueue, AssetLoadQueueConfig, AssetLoadRequest,
        AssetSource, SceneAssetDecoder, TextureAsset, TextureAssetDecoder, decode_completion,
    };
    pub use sindri_core::prelude::*;
    #[cfg(feature = "render")]
    pub use sindri_gpu::{GpuContext, GpuRequestOptions, SurfaceProfile};
}
