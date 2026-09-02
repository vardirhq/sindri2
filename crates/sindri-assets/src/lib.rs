//! Cross-platform byte sources for Sindri's logical asset IDs.
//!
//! This crate owns byte acquisition, bounded scheduling, and CPU-side decoding.
//! GPU upload and caching remain separate stages so native and browser hosts can
//! use the same pipeline without pretending browser fetches are synchronous.

mod audio;
mod decode;
mod loader;
mod manifest;
mod memory;
mod queue;
mod source;
mod url;

#[cfg(target_arch = "wasm32")]
mod fetch;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
#[cfg(not(target_arch = "wasm32"))]
mod watch;

pub use audio::{AudioAsset, AudioAssetDecoder, AudioFormat};
pub use decode::{
    AssetCompletionApplyError, AssetDecodeError, AssetDecoder, DecodedAssetCompletion, FontAsset,
    FontAssetDecoder, SceneAssetDecoder, SpriteSheetAssetDecoder, TextAssetDecoder, TextureAsset,
    TextureAssetDecoder, decode_completion,
};
#[cfg(target_arch = "wasm32")]
pub use fetch::FetchAssetSource;
#[cfg(not(target_arch = "wasm32"))]
pub use filesystem::FileSystemAssetSource;
pub use loader::{AssetLoadOutcome, AssetLoader, AssetLoaderError};
pub use manifest::{
    AssetKind, AssetManifest, ContentHash, MANIFEST_FILE_NAME, MANIFEST_FORMAT_VERSION,
    ManifestEntry, ManifestError,
};
pub use memory::MemoryAssetSource;
pub use queue::{
    AssetLoadCompletion, AssetLoadQueue, AssetLoadQueueConfig, AssetLoadQueueCreateError,
    AssetLoadQueueError, AssetLoadRequest,
};
pub use source::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};
pub use url::{UrlRoot, UrlRootError};
#[cfg(not(target_arch = "wasm32"))]
pub use watch::AssetWatch;
