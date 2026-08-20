//! Cross-platform byte sources for Sindri's logical asset IDs.
//!
//! This crate owns byte acquisition, bounded scheduling, and CPU-side decoding.
//! GPU upload and caching remain separate stages so native and browser hosts can
//! use the same pipeline without pretending browser fetches are synchronous.

mod decode;
mod loader;
mod memory;
mod queue;
mod source;
mod url;

#[cfg(target_arch = "wasm32")]
mod fetch;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;

pub use decode::{
    AssetCompletionApplyError, AssetDecodeError, AssetDecoder, DecodedAssetCompletion,
    SceneAssetDecoder, TextureAsset, TextureAssetDecoder, decode_completion,
};
#[cfg(target_arch = "wasm32")]
pub use fetch::FetchAssetSource;
#[cfg(not(target_arch = "wasm32"))]
pub use filesystem::FileSystemAssetSource;
pub use loader::{AssetLoadOutcome, AssetLoader, AssetLoaderError};
pub use memory::MemoryAssetSource;
pub use queue::{
    AssetLoadCompletion, AssetLoadQueue, AssetLoadQueueConfig, AssetLoadQueueCreateError,
    AssetLoadQueueError, AssetLoadRequest,
};
pub use source::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};
pub use url::{UrlRoot, UrlRootError};
