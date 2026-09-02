//! Writing the directory a static host serves.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sindri_assets::{AssetManifest, ContentHash, MANIFEST_FILE_NAME};
use sindri_core::AssetId;

use crate::gather::ProjectExport;
use crate::{HOST_DIRECTORY, page_for};

/// What an export produced.
#[derive(Debug)]
pub struct WrittenExport {
    pub root: PathBuf,
    /// The directory every asset lives in, named by what they hash to.
    pub content_root: String,
    pub files: usize,
    pub bytes: u64,
}

impl ProjectExport {
    /// Writes this project into `root` as a directory a static host can serve.
    ///
    /// `base_path` is where the export will be reached from — `/` for a domain
    /// of its own, `/repo/` for a GitHub Pages project site. It is baked into
    /// the page rather than guessed at run time, because a page that guessed
    /// would be a page that works locally and 404s once it is deployed.
    pub fn write(&self, root: &Path, base_path: &str) -> Result<WrittenExport, ExportError> {
        let content_root = self.content_hash();
        // Cleared first, because a hashed directory changes name with every
        // change and exporting twice would otherwise leave the old build
        // beside the new one for ever — growing a deployment by a whole copy
        // of itself each time anyone edits a texture.
        let assets_root = root.join("assets");
        if assets_root.exists() {
            std::fs::remove_dir_all(&assets_root)
                .map_err(|error| ExportError::unwritable(&assets_root, &error))?;
        }
        let assets_dir = assets_root.join(&content_root);
        create(&assets_dir)?;

        let mut manifest = AssetManifest::new();
        let mut bytes = 0_u64;
        for asset in &self.assets {
            let id = AssetId::new(&asset.id)
                .map_err(|error| ExportError::Project(format!("{}: {error}", asset.id)))?;
            let path = assets_dir.join(&asset.id);
            if let Some(parent) = path.parent() {
                create(parent)?;
            }
            std::fs::write(&path, &asset.bytes)
                .map_err(|error| ExportError::unwritable(&path, &error))?;
            manifest.insert_as(id, asset.kind, &asset.bytes);
            bytes += asset.bytes.len() as u64;
        }

        // Beside the hashed directory rather than inside it, because this is
        // the one file a browser must always re-fetch: it is how the browser
        // learns which directory to look in.
        let manifest_path = assets_root.join(MANIFEST_FILE_NAME);
        if let Some(parent) = manifest_path.parent() {
            create(parent)?;
        }
        manifest.set_content_root(&content_root);
        let document = manifest
            .to_canonical_json()
            .map_err(|error| ExportError::Project(error.to_string()))?;
        std::fs::write(&manifest_path, &document)
            .map_err(|error| ExportError::unwritable(&manifest_path, &error))?;

        let page = page_for(&self.name, base_path);
        let page_path = root.join("index.html");
        std::fs::write(&page_path, page)
            .map_err(|error| ExportError::unwritable(&page_path, &error))?;

        create(&root.join(HOST_DIRECTORY))?;

        Ok(WrittenExport {
            root: root.to_path_buf(),
            content_root,
            files: self.assets.len(),
            bytes,
        })
    }

    /// What this project's contents hash to, as a directory name.
    ///
    /// Over every asset's id and bytes, so a build differs from another build
    /// exactly when something a player would download differs. That is what
    /// makes the directory safe to cache for ever: a changed asset cannot land
    /// in a directory anyone has already cached.
    #[must_use]
    pub fn content_hash(&self) -> String {
        let mut ordered: BTreeMap<&str, &[u8]> = BTreeMap::new();
        for asset in &self.assets {
            ordered.insert(&asset.id, &asset.bytes);
        }
        let mut combined = Vec::new();
        for (id, bytes) in ordered {
            combined.extend_from_slice(id.as_bytes());
            combined.push(0);
            combined.extend_from_slice(&ContentHash::of(bytes).to_string().into_bytes());
            combined.push(0);
        }
        // Just the digest: a hash reads `sha256:<hex>`, and a colon in a
        // directory name is legal in a URL and awkward in every other place a
        // build passes through. Shortened because this names a directory a
        // person will see in a path, and sixteen hex digits is far past what a
        // build could collide on.
        let full = ContentHash::of(&combined).to_string();
        let digest = full.rsplit(':').next().unwrap_or(&full);
        digest.chars().take(16).collect()
    }
}

fn create(path: &Path) -> Result<(), ExportError> {
    std::fs::create_dir_all(path).map_err(|error| ExportError::unwritable(path, &error))
}

/// Why a project could not be exported.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Project(String),
    #[error("{path} could not be read: {reason}")]
    Unreadable { path: String, reason: String },
    #[error("{path} could not be written: {reason}")]
    Unwritable { path: String, reason: String },
}

impl ExportError {
    pub(crate) fn unreadable(path: &Path, error: &std::io::Error) -> Self {
        Self::Unreadable {
            path: path.display().to_string(),
            reason: error.to_string(),
        }
    }

    pub(crate) fn unwritable(path: &Path, error: &std::io::Error) -> Self {
        Self::Unwritable {
            path: path.display().to_string(),
            reason: error.to_string(),
        }
    }
}
