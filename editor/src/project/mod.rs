//! What the project browser shows.
//!
//! It used to show eight hardcoded entries — four folders, a scene, a mesh, a
//! texture, and a script — whatever was open. That is worse than showing
//! nothing: it names files the project does not contain, and the search box
//! above it accepted typing and filtered a list that could not change. Two
//! controls that looked like they worked, on top of a list that was
//! decoration.
//!
//! So it reads the directory the open scene lives in. The tree is bounded in
//! both depth and count, because a browser that walks a repository checkout on
//! the frame someone opens it is a browser that hangs.

mod kind;
mod sheet;

#[cfg(test)]
mod tests;

pub use kind::AssetKind;
pub use sheet::{sliced_texture_beside, sprites_beside};

use std::path::{Path, PathBuf};

/// How deep the walk goes before it stops descending.
///
/// An asset directory is shallow; a source tree is not, and the browser sits
/// beside a viewport that has to keep drawing.
const MAX_DEPTH: usize = 4;

/// How many entries the walk collects before it stops.
///
/// A cap rather than paging: the browser is a dock, and the honest failure is
/// saying "and more" rather than reading a hundred thousand paths to draw
/// thirty of them.
const MAX_ENTRIES: usize = 400;

/// One file or directory the browser is showing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntry {
    pub path: PathBuf,
    /// The file's own name, which is what a row is labelled with.
    pub name: String,
    /// Where it sits below the root, which is what a filtered row shows instead
    /// of an indentation that would point at a parent the filter removed.
    pub relative: String,
    pub kind: AssetKind,
    pub depth: usize,
    /// For a texture, the sprites its sheet names. Empty when it has no sheet,
    /// which is every unsliced image.
    ///
    /// Read during the walk rather than on demand, because the browser redraws
    /// at the viewport's frame rate and a sidecar does not change that often.
    pub sprites: Vec<String>,
}

/// The directory the browser is showing, as it was last read.
///
/// Read once and kept, rather than walked every frame: the browser redraws at
/// the viewport's frame rate, and a directory does not.
#[derive(Clone, Debug, Default)]
pub struct ProjectTree {
    root: Option<PathBuf>,
    entries: Vec<ProjectEntry>,
    truncated: bool,
    error: Option<String>,
}

impl ProjectTree {
    /// Reads the directory a scene file sits in.
    ///
    /// Given the scene's own path rather than a directory, because that is what
    /// the editor has: the browser follows whatever is open, so opening a scene
    /// somewhere else shows that project instead of the last one.
    pub fn beside(scene: Option<&Path>) -> Self {
        let Some(root) = scene.and_then(Path::parent) else {
            return Self::default();
        };
        Self::rooted(root)
    }

    /// Reads a directory as the project root.
    pub fn rooted(root: &Path) -> Self {
        let mut tree = Self {
            root: Some(root.to_path_buf()),
            ..Self::default()
        };
        if let Err(error) = tree.walk(root, root, 0) {
            tree.error = Some(error);
        }
        tree.entries.sort_by(|left, right| {
            left.relative
                .to_lowercase()
                .cmp(&right.relative.to_lowercase())
        });
        tree
    }

    fn walk(&mut self, root: &Path, directory: &Path, depth: usize) -> Result<(), String> {
        if depth >= MAX_DEPTH {
            self.truncated = true;
            return Ok(());
        }
        let listing = std::fs::read_dir(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in listing {
            let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
            let path = entry.path();
            let Some(name) = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
            else {
                continue;
            };
            // A dot file is the tooling's, not the project's.
            if name.starts_with('.') {
                continue;
            }
            if self.entries.len() >= MAX_ENTRIES {
                self.truncated = true;
                return Ok(());
            }
            let directory_entry = path.is_dir();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let kind = if directory_entry {
                AssetKind::Folder
            } else {
                AssetKind::of_file(&path.to_string_lossy())
            };
            // A sheet whose texture is right there is shown as that texture's
            // sprites, so listing the file as well says the same thing twice.
            // An *orphaned* sheet is still listed, because a sidecar cutting up
            // an image nobody can find is exactly the sort of thing a browser
            // that hides files would let you never notice.
            if kind == AssetKind::Sheet && sliced_texture_beside(&path).is_some() {
                continue;
            }
            let sprites = if kind == AssetKind::Texture {
                sprites_beside(&path)
            } else {
                Vec::new()
            };
            self.entries.push(ProjectEntry {
                name,
                relative,
                kind,
                depth,
                path: path.clone(),
                sprites,
            });
            if directory_entry {
                children.push(path);
            }
        }
        for child in children {
            self.walk(root, &child, depth + 1)?;
        }
        Ok(())
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// What the root is called, for a header that has one line to spend.
    pub fn label(&self) -> String {
        self.root.as_ref().map_or_else(
            || "No project".to_owned(),
            |root| {
                root.file_name().map_or_else(
                    || root.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
        )
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    /// Project-relative references to every font the browser can see.
    ///
    /// A scene stores logical asset IDs rather than absolute paths, so the
    /// inspector must offer the same spelling the asset loader resolves. The
    /// browser has already done the bounded directory walk; reusing it keeps a
    /// font picker from walking the project again every frame.
    pub fn fonts(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Font)
            .map(|entry| entry.relative.replace('\\', "/"))
            .collect()
    }

    /// Project-relative references to every texture the browser can see, and
    /// the named sprites inside each.
    ///
    /// A sliced sheet contributes one reference per sprite — `tiles.png#floor`
    /// — beside the whole image, because those are the references a component
    /// can actually name. Offering the sheet alone would be offering to draw
    /// every frame at once.
    pub fn textures(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Texture)
            .flat_map(|entry| {
                let texture = entry.relative.replace('\\', "/");
                let named = entry
                    .sprites
                    .iter()
                    .map(|sprite| format!("{texture}#{sprite}"))
                    .collect::<Vec<String>>();
                std::iter::once(texture).chain(named)
            })
            .collect()
    }

    /// Project-relative references to every audio clip the browser can see.
    pub fn audio(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Audio)
            .map(|entry| entry.relative.replace('\\', "/"))
            .collect()
    }

    /// Project-relative references to every Decay script the browser can see.
    ///
    /// Decay only: `.rs` and `.wgsl` are listed as scripts by the browser
    /// because they are source, but a `sindri.script` component naming one
    /// would name something nothing can run.
    pub fn scripts(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Script)
            .map(|entry| entry.relative.replace('\\', "/"))
            .filter(|reference| {
                Path::new(reference)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("decay"))
            })
            .collect()
    }

    /// Named sprites belonging to one project-relative texture reference.
    pub fn sprites_for_texture(&self, texture: &str) -> Vec<String> {
        self.entries
            .iter()
            .find(|entry| {
                entry.kind == AssetKind::Texture && entry.relative.replace('\\', "/") == texture
            })
            .map(|entry| entry.sprites.clone())
            .unwrap_or_default()
    }

    /// Whether the walk stopped before it ran out of directory.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Why the directory could not be read, if it could not.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The entries a search shows.
    ///
    /// An empty search is the tree itself, indented. A search is a flat list of
    /// matching files with their path below the root, because a matching file
    /// indented under a parent the search removed sits indented under nothing —
    /// which is the complaint the audit makes about the hierarchy's filter, and
    /// there is no reason to build it twice.
    pub fn matching(&self, needle: &str) -> Vec<&ProjectEntry> {
        let needle = needle.trim().to_lowercase();
        if needle.is_empty() {
            return self.entries.iter().collect();
        }
        self.entries
            .iter()
            .filter(|entry| {
                entry.kind != AssetKind::Folder && entry.name.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The directories in the tree, which is what the folder pane lists.
    pub fn folders(&self) -> Vec<&ProjectEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Folder)
            .collect()
    }
}
