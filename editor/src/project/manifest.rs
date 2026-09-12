//! What makes a directory a project.
//!
//! The editor's unit of work used to be a scene file. Everything followed from
//! it: the browser showed whatever directory the scene happened to sit in, the
//! textures resolved against that directory, and "the project" was a word for
//! the parent folder rather than for anything on disk. That is enough to edit
//! one scene and not enough to *have* a project — there was nowhere to put a
//! name, nothing to list on a welcome screen, and no way to tell a project
//! apart from any other folder with a `.scene.json` in it.
//!
//! So a project is a directory containing `sindri.toml`. The manifest grows only
//! when a feature has a host-level setting to read. Its explicit asset include
//! list is one such setting: export already uses it for files a scene cannot
//! name, and the editor now uses the same roots to distinguish a Weave entry
//! stylesheet from one imported beneath it.
//!
//! Kept apart from the drawing so that "what is a project, and what does making
//! one do to a directory" is a question a test can ask with a temporary folder
//! and no window.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sindri_core::SceneDocument;
use thiserror::Error;

use crate::scene_file::SceneFile;

/// The file whose presence makes a directory a project.
pub const MANIFEST_NAME: &str = "sindri.toml";

/// The manifest schema this build writes.
///
/// Numbered from the first one that ever existed, so a later editor reading an
/// earlier project can tell what it is looking at. A project from the future is
/// refused rather than guessed at: opening it would mean ignoring whatever the
/// newer field said, and a project that quietly loses a setting is worse than
/// one that says it needs a newer editor.
pub const FORMAT_VERSION: u32 = 1;

/// The directories a new project starts with.
///
/// Beside the scene rather than under an `assets/` root, because that is where
/// asset references actually resolve today: `SceneTextures::for_scene` roots the
/// loader at the scene's own directory. A layout the editor cannot resolve would
/// be a layout that looks tidy and loads nothing.
const NEW_PROJECT_DIRECTORIES: [&str; 3] = ["textures", "scripts", "fonts"];

/// The scene a new project is created with.
const NEW_PROJECT_SCENE: &str = "main.scene.json";

/// Why a project could not be opened or created.
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("{path} is not a Sindri project: it has no sindri.toml")]
    NotAProject { path: String },
    #[error("{path} is already a Sindri project")]
    AlreadyAProject { path: String },
    #[error("a project needs a name")]
    Unnamed,
    #[error(
        "{path} was made by a newer Sindri: it uses project format {found}, and this editor \
         understands {FORMAT_VERSION}"
    )]
    FromTheFuture { path: String, found: u32 },
    #[error("{path} could not be read: {source}")]
    Unreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} could not be written: {source}")]
    Unwritable {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a readable project file: {source}")]
    Malformed {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("the project file could not be written: {source}")]
    Unserializable {
        #[source]
        source: toml::ser::Error,
    },
    #[error(transparent)]
    Scene(#[from] crate::scene_file::SceneFileError),
}

/// `sindri.toml`, as it is written.
///
/// `format_version` sits above the table rather than inside it because it
/// describes the file and not the project, and because TOML puts every bare key
/// before the first table anyway — a field order that produces invalid TOML is
/// a struct that serializes into a file it cannot read back.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectManifest {
    pub format_version: u32,
    pub project: ProjectSection,
    /// Assets that cannot be discovered by walking the authored scene.
    ///
    /// Weave entry stylesheets are the first editor feature that needs this:
    /// imported `.weave` files must not be applied as independent roots just
    /// because the project browser can see them. The manifest is the source of
    /// truth the exporter and browser host already use for that distinction.
    #[serde(default, skip_serializing_if = "AssetsSection::is_empty")]
    pub assets: AssetsSection,
}

/// What the project itself says about itself.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectSection {
    /// What the project is called, which is what the welcome window lists it as.
    ///
    /// Stored rather than taken from the directory name, so a project can be
    /// called "Gather" while living in a folder called `assets`, and so renaming
    /// a checkout does not rename the game.
    pub name: String,
    /// The scene opening this project should open, relative to the project root.
    ///
    /// Optional because a project can legitimately have no obvious first scene —
    /// a library of prefabs, a project mid-restructure — and inventing one would
    /// mean the editor opening a file the author never nominated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_scene: Option<String>,
    /// Scenes the project can reach besides the one it opens on.
    ///
    /// The editor does not use these yet; it carries them so that saving a
    /// project does not delete them. This struct serializes by named field, so
    /// a field it did not know about would be quietly dropped the first time
    /// anything here rewrote the manifest — and a person would find their
    /// interiors gone from a build with nothing to point at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<String>,
}

/// What a project asks hosts to carry even when its scene cannot name it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AssetsSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
}

impl AssetsSection {
    fn is_empty(&self) -> bool {
        self.include.is_empty()
    }
}

/// A project, as it was last read from disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    root: PathBuf,
    manifest: ProjectManifest,
}

impl Project {
    /// Reads the project in a directory.
    pub fn open(root: &Path) -> Result<Self, ProjectError> {
        let path = manifest_path(root);
        if !path.exists() {
            return Err(ProjectError::NotAProject {
                path: root.display().to_string(),
            });
        }
        let text = std::fs::read_to_string(&path).map_err(|source| ProjectError::Unreadable {
            path: path.display().to_string(),
            source,
        })?;
        let manifest: ProjectManifest =
            toml::from_str(&text).map_err(|source| ProjectError::Malformed {
                path: path.display().to_string(),
                source,
            })?;
        if manifest.format_version > FORMAT_VERSION {
            return Err(ProjectError::FromTheFuture {
                path: path.display().to_string(),
                found: manifest.format_version,
            });
        }
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Makes a project in `root`, with a scene in it.
    ///
    /// The blank scene arrives from the caller rather than being built here.
    /// What a new scene contains is the editor's answer — one world camera, from
    /// the component registry's own default payload — and a second copy of that
    /// default living in this module is a copy that drifts from the first.
    ///
    /// The directory is created if it does not exist and used if it does, but a
    /// directory that already holds a `sindri.toml` is refused: creating a
    /// project over a project would overwrite a name and a nominated scene that
    /// somebody chose.
    pub fn create(root: &Path, name: &str, scene: &SceneDocument) -> Result<Self, ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::Unnamed);
        }
        if manifest_path(root).exists() {
            return Err(ProjectError::AlreadyAProject {
                path: root.display().to_string(),
            });
        }
        std::fs::create_dir_all(root).map_err(|source| ProjectError::Unwritable {
            path: root.display().to_string(),
            source,
        })?;
        for directory in NEW_PROJECT_DIRECTORIES {
            let path = root.join(directory);
            std::fs::create_dir_all(&path).map_err(|source| ProjectError::Unwritable {
                path: path.display().to_string(),
                source,
            })?;
        }
        SceneFile::create(&root.join(NEW_PROJECT_SCENE), scene)?;
        let project = Self {
            root: root.to_path_buf(),
            manifest: ProjectManifest {
                format_version: FORMAT_VERSION,
                project: ProjectSection {
                    name: name.to_owned(),
                    main_scene: Some(NEW_PROJECT_SCENE.to_owned()),
                    scenes: Vec::new(),
                },
                assets: AssetsSection::default(),
            },
        };
        project.write()?;
        Ok(project)
    }

    /// Writes the manifest back.
    pub fn write(&self) -> Result<(), ProjectError> {
        let path = manifest_path(&self.root);
        let text = toml::to_string_pretty(&self.manifest)
            .map_err(|source| ProjectError::Unserializable { source })?;
        std::fs::write(&path, text).map_err(|source| ProjectError::Unwritable {
            path: path.display().to_string(),
            source,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> &str {
        &self.manifest.project.name
    }

    /// Explicit assets the project asks hosts to carry even when a scene does
    /// not name them directly.
    pub fn included_assets(&self) -> &[String] {
        &self.manifest.assets.include
    }

    /// The scene opening this project should open.
    ///
    /// The nominated scene when it is there, and otherwise nothing. A project
    /// whose `main_scene` names a file that has been deleted or renamed does not
    /// silently open a different one: the editor opens the project with no scene
    /// and says so, because standing another scene in for the named one reads as
    /// though the named one loaded.
    pub fn main_scene(&self) -> Option<PathBuf> {
        let named = self.manifest.project.main_scene.as_ref()?;
        let path = self.root.join(named);
        path.is_file().then_some(path)
    }

    /// Nominates the scene this project opens on, and writes it back.
    ///
    /// What "Set as main scene" means, and what creating a project's first scene
    /// does. A path outside the project is refused rather than stored: the field
    /// is relative to the root, and one that escaped it would name a scene the
    /// project does not contain.
    pub fn set_main_scene(&mut self, scene: &Path) -> Result<(), ProjectError> {
        let relative = scene
            .strip_prefix(&self.root)
            .map_err(|_| ProjectError::NotAProject {
                path: scene.display().to_string(),
            })?;
        self.manifest.project.main_scene = Some(relative.to_string_lossy().replace('\\', "/"));
        self.write()
    }
}

/// Where a directory's manifest lives.
pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_NAME)
}

/// Whether a directory is a project.
pub fn is_project(root: &Path) -> bool {
    manifest_path(root).is_file()
}

/// The project a path belongs to, if any.
///
/// Walks up from the file rather than requiring the project root to be named,
/// so opening a scene deep inside a project — from the command line, or from a
/// file dialog — still opens it *as* that project rather than as a loose scene
/// in whatever folder it happened to be in. Bounded by the file system's own
/// root, which every ancestor walk reaches.
pub fn root_for(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|ancestor| is_project(ancestor))
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests;
