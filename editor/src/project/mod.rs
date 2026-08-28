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
/// What the editor opens when it starts, and why.
pub mod launch;
/// What makes a directory a project rather than a folder with a scene in it.
pub mod manifest;
pub mod ops;
/// The projects the welcome window offers.
pub mod recent;
mod sheet;

#[cfg(test)]
mod tests;

pub use kind::AssetKind;
pub use launch::Launch;
pub use manifest::{MANIFEST_NAME, Project, ProjectError};
pub use recent::{RecentProject, RecentProjects};
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
    /// How a scene names this file, or `None` when nothing can name it.
    ///
    /// Not the same string as [`Self::relative`], and the difference is the
    /// whole point: a reference is resolved against the directory the asset
    /// loader is rooted at, which is the open scene's own directory, while
    /// `relative` is measured from the project root. Those are the same folder
    /// for a project the editor created and are two folders apart for one that
    /// keeps its scene under `assets/` — where `assets/textures/orb.png` is the
    /// path from the root and `textures/orb.png` is the reference that loads.
    ///
    /// `None` for a directory, which nothing references, and for a file outside
    /// the directory references resolve against — a `src/main.rs` beside the
    /// assets is a real file that no component can name.
    pub reference: Option<String>,
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
    /// What the project calls itself, when the browser is showing one.
    ///
    /// A folder name is what the browser had, and it is not always the answer:
    /// the companion game is called Gather and lives in a directory called
    /// `assets`. A manifest is the only place that name exists, so a tree
    /// rooted at a project is told it.
    name: Option<String>,
    /// The directory asset references are resolved against.
    ///
    /// The project root until something says otherwise, and the open scene's
    /// own directory once [`ProjectTree::resolving_at`] has been told it.
    assets: Option<PathBuf>,
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
        Self::read(root, None)
    }

    /// Reads a project's directory, under the name the project gives itself.
    pub fn rooted_as(root: &Path, name: &str) -> Self {
        Self::read(root, Some(name.to_owned()))
    }

    /// Points the tree's asset references at the directory they resolve against.
    ///
    /// The one thing a directory listing cannot work out for itself. A scene
    /// names its assets relative to its own directory — that is where
    /// `SceneTextures::for_scene` roots the loader — and the browser is rooted
    /// at the project, which is a different folder whenever a project keeps its
    /// scene under `assets/`. Reading the tree from the root and offering its
    /// root-relative paths as references is how the inspector came to mark
    /// `textures/orb.png` as missing and to offer `assets/textures/orb.png`,
    /// which is the spelling that does not load.
    ///
    /// A directory outside the tree is ignored rather than honoured: it would
    /// leave every file in the project unreferenceable, which is a worse answer
    /// than the root.
    #[must_use]
    pub fn resolving_at(mut self, assets: Option<&Path>) -> Self {
        let Some(assets) = assets else {
            return self;
        };
        if !self
            .root
            .as_ref()
            .is_some_and(|root| assets.starts_with(root))
        {
            return self;
        }
        self.point_references_at(assets);
        self
    }

    fn read(root: &Path, name: Option<String>) -> Self {
        let mut tree = Self {
            root: Some(root.to_path_buf()),
            name,
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
        tree.point_references_at(root);
        tree
    }

    /// Works out how each file would be named from `assets`.
    ///
    /// A folder gets none: nothing references a directory, and giving one a
    /// reference would put it in a picker as something a component could name.
    fn point_references_at(&mut self, assets: &Path) {
        for entry in &mut self.entries {
            entry.reference = (entry.kind != AssetKind::Folder)
                .then(|| entry.path.strip_prefix(assets).ok())
                .flatten()
                .map(|below| below.to_string_lossy().replace('\\', "/"));
        }
        self.assets = Some(assets.to_path_buf());
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
                // Filled in once the walk is done, by the one place that knows
                // which directory references resolve against.
                reference: None,
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

    /// The directory asset references resolve against.
    ///
    /// The project root for a project the editor created, whose scene sits
    /// beside its `textures/` and `scripts/`. A folder below it for a project
    /// that keeps its scene under `assets/`, which is the layout the companion
    /// game uses and the one the browser has to tell apart from the root.
    pub fn assets_root(&self) -> Option<&Path> {
        self.assets.as_deref()
    }

    /// Whether the project keeps anything outside the directory it loads assets
    /// from.
    ///
    /// What makes listing only the assets worth offering: a project whose
    /// scene sits at its root has nothing to hide, and a control that switches
    /// between two identical listings is a control that does nothing.
    pub fn keeps_more_than_assets(&self) -> bool {
        self.assets.is_some() && self.assets != self.root
    }

    /// What the root is called, for a header that has one line to spend.
    ///
    /// The project's own name when it has one, and the folder's otherwise.
    pub fn label(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
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

    /// Every file of one kind, spelled the way a scene names it.
    ///
    /// A scene stores logical asset IDs rather than absolute paths, so the
    /// inspector must offer the same spelling the asset loader resolves. The
    /// browser has already done the bounded directory walk; reusing it keeps a
    /// font picker from walking the project again every frame.
    ///
    /// A file the loader cannot reach has no reference and is left out
    /// entirely, because offering a path that will not load is worse than
    /// offering nothing: the field would accept it and the scene would draw the
    /// missing checker.
    fn referenced(&self, kind: AssetKind) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(move |entry| entry.kind == kind)
            .filter_map(|entry| entry.reference.as_deref())
    }

    /// References to every font the browser can see.
    pub fn fonts(&self) -> Vec<String> {
        self.referenced(AssetKind::Font)
            .map(str::to_owned)
            .collect()
    }

    /// References to every texture the browser can see, and the named sprites
    /// inside each.
    ///
    /// A sliced sheet contributes one reference per sprite — `tiles.png#floor`
    /// — beside the whole image, because those are the references a component
    /// can actually name. Offering the sheet alone would be offering to draw
    /// every frame at once.
    pub fn textures(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Texture)
            .filter_map(|entry| Some((entry.reference.as_deref()?, entry)))
            .flat_map(|(texture, entry)| {
                let named = entry
                    .sprites
                    .iter()
                    .map(|sprite| format!("{texture}#{sprite}"))
                    .collect::<Vec<String>>();
                std::iter::once(texture.to_owned()).chain(named)
            })
            .collect()
    }

    /// References to every audio clip the browser can see.
    pub fn audio(&self) -> Vec<String> {
        self.referenced(AssetKind::Audio)
            .map(str::to_owned)
            .collect()
    }

    /// References to every Decay script the browser can see.
    ///
    /// Decay only: `.rs` and `.wgsl` are listed as scripts by the browser
    /// because they are source, but a `sindri.script` component naming one
    /// would name something nothing can run.
    pub fn scripts(&self) -> Vec<String> {
        self.referenced(AssetKind::Script)
            .filter(|reference| {
                Path::new(reference)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("decay"))
            })
            .map(str::to_owned)
            .collect()
    }

    /// Named sprites belonging to one texture reference.
    pub fn sprites_for_texture(&self, texture: &str) -> Vec<String> {
        self.entries
            .iter()
            .find(|entry| {
                entry.kind == AssetKind::Texture && entry.reference.as_deref() == Some(texture)
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

    /// The directories a listing rooted at `within` navigates, each with the
    /// depth it is drawn at.
    ///
    /// Depth is measured from that listing's own root rather than from the
    /// project's, so the folders inside an assets directory sit at the left
    /// edge instead of one indent in under a parent the listing does not show.
    /// `None` is the whole project, which is what the pane listed before there
    /// was anything narrower to list.
    pub fn folders_in(&self, within: Option<&Path>) -> Vec<(&ProjectEntry, usize)> {
        let base = within
            .and_then(|within| {
                self.entries
                    .iter()
                    .find(|entry| entry.path == within)
                    .map(|entry| entry.depth + 1)
            })
            .unwrap_or(0);
        self.entries
            .iter()
            .filter(|entry| entry.kind == AssetKind::Folder)
            .filter(|entry| {
                within.is_none_or(|within| entry.path.starts_with(within) && entry.path != within)
            })
            .map(|entry| (entry, entry.depth.saturating_sub(base)))
            .collect()
    }
}
