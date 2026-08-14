use std::{io, path::PathBuf};

use sindri_core::{AssetId, AssetLoadErrorKind};

use crate::{AssetBytes, AssetSource, AssetSourceError, AssetSourceFuture};

#[derive(Clone, Debug)]
pub struct FileSystemAssetSource {
    root: PathBuf,
}

impl FileSystemAssetSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn read(&self, id: &AssetId) -> Result<AssetBytes, AssetSourceError> {
        let root = std::fs::canonicalize(&self.root)
            .map_err(|error| io_error(id, "filesystem", "resolve asset root", &error))?;
        let candidate = self.root.join(id.as_str());
        let resolved = std::fs::canonicalize(&candidate)
            .map_err(|error| io_error(id, "filesystem", "resolve asset path", &error))?;

        if !resolved.starts_with(&root) {
            return Err(AssetSourceError::new(
                id.clone(),
                "filesystem",
                AssetLoadErrorKind::AccessDenied,
                format!(
                    "resolved path '{}' escapes asset root '{}'",
                    resolved.display(),
                    root.display()
                ),
            ));
        }

        let bytes = std::fs::read(&resolved)
            .map_err(|error| io_error(id, "filesystem", "read asset", &error))?;
        Ok(AssetBytes::new(id.clone(), bytes))
    }
}

impl AssetSource for FileSystemAssetSource {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn load<'a>(&'a self, id: &'a AssetId) -> AssetSourceFuture<'a> {
        Box::pin(async move { self.read(id) })
    }
}

fn io_error(
    id: &AssetId,
    source: &'static str,
    operation: &str,
    error: &io::Error,
) -> AssetSourceError {
    let kind = match error.kind() {
        io::ErrorKind::NotFound => AssetLoadErrorKind::NotFound,
        io::ErrorKind::PermissionDenied => AssetLoadErrorKind::AccessDenied,
        _ => AssetLoadErrorKind::Io,
    };
    AssetSourceError::new(
        id.clone(),
        source,
        kind,
        format!("could not {operation}: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_source_reads_relative_logical_ids() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("scenes")).unwrap();
        std::fs::write(directory.path().join("scenes/room.json"), b"room").unwrap();
        let source = FileSystemAssetSource::new(directory.path());
        let id = AssetId::new("scenes/room.json").unwrap();

        let loaded = pollster::block_on(source.load(&id)).unwrap();
        assert_eq!(loaded.id(), &id);
        assert_eq!(loaded.as_slice(), b"room");
    }

    #[test]
    fn filesystem_source_classifies_missing_assets() {
        let directory = tempfile::tempdir().unwrap();
        let source = FileSystemAssetSource::new(directory.path());
        let id = AssetId::new("textures/missing.png").unwrap();

        let error = pollster::block_on(source.load(&id)).unwrap_err();
        assert_eq!(error.kind(), AssetLoadErrorKind::NotFound);
        assert!(error.to_string().contains("textures/missing.png"));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_source_rejects_symlinks_outside_the_root() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();
        let source = FileSystemAssetSource::new(root.path());
        let id = AssetId::new("linked/secret.txt").unwrap();

        let error = pollster::block_on(source.load(&id)).unwrap_err();
        assert_eq!(error.kind(), AssetLoadErrorKind::AccessDenied);
    }
}
