//! Making, moving, copying and removing the files a project is made of.
//!
//! Every asset used to have to arrive from outside the editor: there was no
//! create, no folder, no rename, no delete, no duplicate and no import, so
//! building a project meant a file manager beside the window and the Refresh
//! button afterwards.
//!
//! These are disk writes, and disk writes are not commands. Nothing here goes
//! through the undo history, because the history describes a world and these
//! describe a directory — undoing a delete would mean the editor holding the
//! bytes of every file anyone removed for as long as the session lasted. So
//! the rules are the other ones that keep a destructive act honest: every
//! operation is checked before it runs, refuses rather than overwrites, and
//! the one that cannot be taken back is asked about first by the panel that
//! calls it.
//!
//! Kept apart from the drawing so that "what does this do to the directory"
//! is a question a test can ask with a temporary folder and no window.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Why a file operation did not happen.
#[derive(Debug, Error)]
pub enum AssetOpError {
    #[error("a name cannot be empty")]
    EmptyName,
    #[error("'{0}' is not a name: it points somewhere else in the file system")]
    NotAName(String),
    #[error("'{0}' already exists here")]
    Exists(String),
    #[error("that file is not inside this project")]
    OutsideProject,
    #[error("{path} could not be read or written: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl AssetOpError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            source,
        }
    }
}

/// A file name that is a name and nothing else.
///
/// The one check every operation here starts with. A browser row hands over
/// whatever was typed into it, and `../../etc/hosts` is a perfectly good string
/// — it is just not a file name, and joining it to a directory would put the
/// operation somewhere nobody was looking.
fn checked_name(name: &str) -> Result<&str, AssetOpError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AssetOpError::EmptyName);
    }
    // Exactly one *ordinary* component. Counting components alone is not
    // enough: `..` is one component, and joining it to a directory walks out
    // of the project rather than naming something in it.
    let mut parts = Path::new(name).components();
    let one_plain_part =
        matches!(parts.next(), Some(std::path::Component::Normal(_))) && parts.next().is_none();
    if name.contains(['/', '\\']) || !one_plain_part {
        return Err(AssetOpError::NotAName(name.to_owned()));
    }
    Ok(name)
}

/// Whether a path is inside the project the browser is showing.
///
/// Asked of every target, because "inside the project" is the whole of what
/// makes these operations safe to offer: the browser lists one directory, and
/// nothing it offers should reach outside it. A root the path is not under is
/// a refusal rather than a silent success somewhere else.
fn inside(root: &Path, path: &Path) -> Result<(), AssetOpError> {
    // Compared against the canonical root so a project reached through a
    // symlink still recognises its own files.
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let anchor = path.parent().unwrap_or(path);
    let anchor = anchor
        .canonicalize()
        .unwrap_or_else(|_| anchor.to_path_buf());
    if anchor.starts_with(&root) {
        Ok(())
    } else {
        Err(AssetOpError::OutsideProject)
    }
}

/// Refuses a path something is already at.
///
/// Every operation here creates something, and none of them overwrite: a
/// duplicate that silently replaced the file it was named after would be a
/// delete wearing another verb.
fn vacant(path: &Path) -> Result<(), AssetOpError> {
    if path.exists() {
        return Err(AssetOpError::Exists(path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )));
    }
    Ok(())
}

/// Makes a folder inside `parent`.
pub fn create_folder(root: &Path, parent: &Path, name: &str) -> Result<PathBuf, AssetOpError> {
    let name = checked_name(name)?;
    let path = parent.join(name);
    inside(root, &path)?;
    vacant(&path)?;
    std::fs::create_dir(&path).map_err(|source| AssetOpError::io(&path, source))?;
    Ok(path)
}

/// Makes a file with the given contents inside `parent`.
///
/// What "New script" needs. The contents are the caller's business — this only
/// promises that nothing was overwritten and that the name is a name.
pub fn create_file(
    root: &Path,
    parent: &Path,
    name: &str,
    contents: &str,
) -> Result<PathBuf, AssetOpError> {
    let name = checked_name(name)?;
    let path = parent.join(name);
    inside(root, &path)?;
    vacant(&path)?;
    std::fs::write(&path, contents).map_err(|source| AssetOpError::io(&path, source))?;
    Ok(path)
}

/// Renames a file or folder in place, keeping it where it is.
///
/// A rename is a move, and a move that can land anywhere is not a rename: the
/// new name is joined to the same parent rather than treated as a path, so
/// typing a slash into the field is refused instead of relocating the file.
pub fn rename(root: &Path, path: &Path, name: &str) -> Result<PathBuf, AssetOpError> {
    let name = checked_name(name)?;
    inside(root, path)?;
    let parent = path.parent().ok_or(AssetOpError::OutsideProject)?;
    let target = parent.join(name);
    if target == path {
        return Ok(target);
    }
    vacant(&target)?;
    std::fs::rename(path, &target).map_err(|source| AssetOpError::io(path, source))?;
    Ok(target)
}

/// Copies a file or folder beside itself, under a name nothing is using.
pub fn duplicate(root: &Path, path: &Path) -> Result<PathBuf, AssetOpError> {
    inside(root, path)?;
    let parent = path.parent().ok_or(AssetOpError::OutsideProject)?;
    let target = unused_beside(parent, path);
    if path.is_dir() {
        copy_tree(path, &target)?;
    } else {
        std::fs::copy(path, &target).map_err(|source| AssetOpError::io(path, source))?;
    }
    Ok(target)
}

/// Removes a file, or a folder and everything in it.
///
/// The one operation with nothing behind it: there is no undo for a disk
/// write, so the panel asks before calling this and this does not ask again.
pub fn delete(root: &Path, path: &Path) -> Result<(), AssetOpError> {
    inside(root, path)?;
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|source| AssetOpError::io(path, source))
    } else {
        std::fs::remove_file(path).map_err(|source| AssetOpError::io(path, source))
    }
}

/// Copies files from anywhere into `into`, keeping their names.
///
/// What "import" means here, and all it means: an asset is a file beside the
/// scene, so bringing one in is copying it there. Nothing is converted, and
/// nothing is registered — the browser reads the directory, so a file that
/// lands in it is in the project.
///
/// Returns what arrived. A source that would overwrite something is skipped
/// and reported rather than failing the whole import: choosing eight images
/// and losing all of them because one shares a name is not a useful answer.
pub fn import(root: &Path, into: &Path, sources: &[PathBuf]) -> (Vec<PathBuf>, Vec<AssetOpError>) {
    let mut arrived = Vec::new();
    let mut refused = Vec::new();
    for source in sources {
        match import_one(root, into, source) {
            Ok(path) => arrived.push(path),
            Err(error) => refused.push(error),
        }
    }
    (arrived, refused)
}

fn import_one(root: &Path, into: &Path, source: &Path) -> Result<PathBuf, AssetOpError> {
    let name = source
        .file_name()
        .ok_or_else(|| AssetOpError::NotAName(source.display().to_string()))?;
    let target = into.join(name);
    inside(root, &target)?;
    vacant(&target)?;
    std::fs::copy(source, &target).map_err(|error| AssetOpError::io(source, error))?;
    Ok(target)
}

/// A name beside `path` that nothing is using yet.
///
/// Derived from the original — `orb copy.png`, then `orb copy 2.png` — because
/// it says what the file is, and a stem the extension still follows is what
/// keeps the copy the same kind of asset as the thing it came from.
fn unused_beside(parent: &Path, path: &Path) -> PathBuf {
    let (stem, suffix) = split_name(path);
    let mut candidate = parent.join(format!("{stem} copy{suffix}"));
    let mut nth = 2_u32;
    while candidate.exists() {
        candidate = parent.join(format!("{stem} copy {nth}{suffix}"));
        nth += 1;
    }
    candidate
}

/// A file's name split into the part to add to and the part to keep.
///
/// Not `Path::file_stem`, which stops at the last dot: a scene is
/// `level.scene.json` and a sheet is `tiles.sheet.json`, and a copy called
/// `level.scene copy.json` is a file the browser no longer reads as a scene.
fn split_name(path: &Path) -> (String, String) {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    for suffix in [".scene.json", ".sheet.json"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return (stem.to_owned(), suffix.to_owned());
        }
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem.to_owned(), format!(".{extension}")),
        _ => (name, String::new()),
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), AssetOpError> {
    std::fs::create_dir_all(to).map_err(|source| AssetOpError::io(to, source))?;
    for entry in std::fs::read_dir(from).map_err(|source| AssetOpError::io(from, source))? {
        let entry = entry.map_err(|source| AssetOpError::io(from, source))?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)
                .map_err(|source| AssetOpError::io(&entry.path(), source))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
