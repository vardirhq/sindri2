//! Turning a project into a directory a static host can serve.
//!
//! The browser host proved the runtime path; this is what a project needs to
//! get there without anyone hand-writing a list of its assets. Everything the
//! export produces is derived from the scene: what it references is what ships,
//! and what it does not reference is not carried.
//!
//! The layout is what makes caching safe:
//!
//! ```text
//! index.html                 the page, rewritten for the host's base path
//! pkg/                       the WebAssembly host, as wasm-pack built it
//! assets/manifest.json       small, and never cached
//! assets/<content hash>/     every asset, and cacheable for ever
//! ```
//!
//! The manifest is the one file a browser must re-fetch, and it names the
//! directory the rest live in. Change any asset and that directory's name
//! changes, so a returning player gets the new build without a stale byte and
//! without anyone remembering to bump a version.

mod gather;
mod page;
mod write;

pub use gather::{GatheredAsset, ProjectExport};
pub use page::{PAGE_TEMPLATE, page_for, page_for_host};
pub use write::{ExportError, WrittenExport};

/// The file a browser fetches first, and the only one it must not cache.
pub const MANIFEST_PATH: &str = "assets/manifest.json";

/// Where the page and the host go, relative to the export root.
pub const HOST_DIRECTORY: &str = "pkg";
