//! Cross-platform byte sources for Sindri's logical asset IDs.
//!
//! This crate defines I/O boundaries only. Decoding, GPU upload, caching, and
//! scheduling remain separate stages so native and browser hosts can drive the
//! same source contract without pretending browser fetches are synchronous.

mod memory;
mod source;

#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
#[cfg(target_arch = "wasm32")]
mod fetch;

#[cfg(target_arch = "wasm32")]
pub use fetch::FetchAssetSource;
#[cfg(not(target_arch = "wasm32"))]
pub use filesystem::FileSystemAssetSource;
pub use memory::MemoryAssetSource;
pub use source::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};
