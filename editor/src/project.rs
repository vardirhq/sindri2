//! What the project browser shows.
//!
//! It used to show eight hardcoded entries — four folders, a scene, a mesh, a
//! texture, and a script — whatever was open. That is worse than showing
//! nothing: it names files the project does not contain, and the search box
//! above it accepted typing and filtered a list that could not change. Two
//! controls that looked like they worked, on top of a list that was decoration.
//!
//! So it reads the directory the open scene lives in. The tree is bounded in
//! both depth and count, because a browser that walks a repository checkout on
//! the frame someone opens it is a browser that hangs.

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

/// What kind of thing an entry is, as far as the browser can tell.
///
/// From the extension, because that is all a file offers before something opens
/// it. `Other` is deliberate: an unrecognised file is still listed, since a
/// browser that hides what it does not understand is a browser you cannot trust
/// to be showing you the directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    Folder,
    Scene,
    Texture,
    Mesh,
    Script,
    Font,
    Other,
}

impl AssetKind {
    /// What the browser calls this kind in its right-hand column.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Scene => "Scene",
            Self::Texture => "Texture",
            Self::Mesh => "Mesh",
            Self::Script => "Script",
            Self::Font => "Font",
            Self::Other => "File",
        }
    }

    /// What a file of this name is, judged by its extension.
    ///
    /// A scene is `*.scene.json` rather than any JSON, because the editor can
    /// open one and not the other, and a row that offers to open a settings
    /// file as a scene is the same class of lie this module exists to remove.
    fn of_file(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.ends_with(".scene.json") {
            return Self::Scene;
        }
        match lower.rsplit_once('.').map(|(_, extension)| extension) {
            Some("png" | "jpg" | "jpeg" | "webp" | "bmp" | "ktx2" | "dds") => Self::Texture,
            Some("gltf" | "glb" | "obj" | "fbx") => Self::Mesh,
            // `decay` first because it is the engine's own: a `.decay` file is
            // a script the editor can actually run, and the rest are scripts
            // only in the sense that they are code sitting in a project.
            Some("decay" | "rs" | "ts" | "js" | "wgsl") => Self::Script,
            Some("ttf" | "otf" | "woff" | "woff2") => Self::Font,
            _ => Self::Other,
        }
    }
}

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
            self.entries.push(ProjectEntry {
                name,
                relative,
                kind: if directory_entry {
                    AssetKind::Folder
                } else {
                    AssetKind::of_file(&path.to_string_lossy())
                },
                depth,
                path: path.clone(),
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn project() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("demo.scene.json"), "{}").unwrap();
        fs::write(root.join("settings.json"), "{}").unwrap();
        fs::write(root.join(".hidden"), "").unwrap();
        fs::create_dir(root.join("textures")).unwrap();
        fs::write(root.join("textures/badge.png"), "").unwrap();
        fs::write(root.join("textures/tiles.png"), "").unwrap();
        fs::create_dir(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/scene.rs"), "").unwrap();
        fs::write(root.join("scripts/spin.decay"), "").unwrap();
        directory
    }

    fn names(entries: &[&ProjectEntry]) -> Vec<String> {
        entries.iter().map(|entry| entry.name.clone()).collect()
    }

    /// The whole point: the browser shows the project, not eight fixed rows.
    #[test]
    fn the_browser_reads_the_directory_the_scene_lives_in() {
        let directory = project();
        let tree = ProjectTree::beside(Some(&directory.path().join("demo.scene.json")));

        assert_eq!(tree.error(), None);
        assert_eq!(
            names(&tree.matching("")),
            [
                "demo.scene.json",
                "scripts",
                "scene.rs",
                "spin.decay",
                "settings.json",
                "textures",
                "badge.png",
                "tiles.png",
            ],
            "children follow their parent, and each level is sorted by name"
        );
        assert!(
            !names(&tree.matching(""))
                .iter()
                .any(|name| name == ".hidden"),
            "a dot file belongs to the tooling, not the project"
        );
    }

    /// The search box accepted typing and filtered nothing, which is worse than
    /// a button that visibly does nothing.
    #[test]
    fn the_search_box_filters_what_is_shown() {
        let directory = project();
        let tree = ProjectTree::rooted(directory.path());

        assert_eq!(names(&tree.matching("png")), ["badge.png", "tiles.png"]);
        assert_eq!(names(&tree.matching("BADGE")), ["badge.png"]);
        assert!(
            tree.matching("nothing here").is_empty(),
            "a search that matches nothing shows nothing rather than everything"
        );
        assert!(
            names(&tree.matching("text")).is_empty(),
            "a search lists files, not the folders they are in"
        );
    }

    /// A row says what it is, and a scene is the one thing the editor can open.
    #[test]
    fn a_row_knows_what_kind_of_file_it_is() {
        let directory = project();
        let tree = ProjectTree::rooted(directory.path());
        let kind = |name: &str| {
            tree.entries()
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| entry.kind)
        };

        assert_eq!(kind("demo.scene.json"), Some(AssetKind::Scene));
        assert_eq!(
            kind("settings.json"),
            Some(AssetKind::Other),
            "only a scene file is a scene, or a row offers to open something the editor cannot"
        );
        assert_eq!(kind("badge.png"), Some(AssetKind::Texture));
        assert_eq!(kind("scene.rs"), Some(AssetKind::Script));
        // The engine's own language, which the browser listed as a plain file
        // until it was named here.
        assert_eq!(kind("spin.decay"), Some(AssetKind::Script));
        assert_eq!(kind("textures"), Some(AssetKind::Folder));
    }

    /// A detached scene has no directory to show, and says so rather than
    /// showing the last project or a made-up one.
    #[test]
    fn a_scene_with_no_file_has_no_project_to_browse() {
        let tree = ProjectTree::beside(None);
        assert_eq!(tree.root(), None);
        assert_eq!(tree.label(), "No project");
        assert!(tree.entries().is_empty());
        assert_eq!(tree.error(), None);
    }

    /// A directory that cannot be read is reported, not drawn as empty.
    #[test]
    fn an_unreadable_directory_names_itself() {
        let directory = tempfile::tempdir().unwrap();
        let tree = ProjectTree::rooted(&directory.path().join("not-here"));
        let error = tree.error().expect("a missing directory is an error");
        assert!(error.contains("not-here"), "{error}");
        assert!(tree.entries().is_empty());
    }

    /// The walk stops rather than reading a source tree to draw thirty rows.
    #[test]
    fn a_deep_tree_stops_and_says_it_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let mut deep = directory.path().to_path_buf();
        for level in 0..(MAX_DEPTH + 2) {
            deep = deep.join(format!("level{level}"));
            fs::create_dir(&deep).unwrap();
        }
        let tree = ProjectTree::rooted(directory.path());
        assert!(tree.truncated(), "the walk has to admit it stopped");
        assert_eq!(tree.entries().len(), MAX_DEPTH);
    }
}
