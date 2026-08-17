//! The scene the editor has open, and where it came from.
//!
//! The editor used to author a copy of the demo scene compiled into the binary,
//! which made saving meaningless: there was nowhere for a save to go, so editing
//! a transform and reopening could not be the same thing twice.
//!
//! A scene is a file. This owns the path, the document as it was last agreed
//! with disk, and the two operations that keep them in step.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use sindri_core::{SceneDocument, SceneJsonError, SceneMigrator, World, WorldError};
use thiserror::Error;

/// Where the editor looks when it is started with no argument.
///
/// The demo scene is the only one in the repository, and starting in it is what
/// makes the editor useful from a clean clone.
pub const DEFAULT_SCENE_PATH: &str = "examples/cube/assets/demo.scene.json";

/// A scene document together with the file it belongs to.
#[derive(Clone, Debug)]
pub struct SceneFile {
    path: Option<PathBuf>,
    document: SceneDocument,
}

impl SceneFile {
    /// Opens a scene from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SceneFileError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| SceneFileError::Read {
            path: path.display().to_string(),
            source,
        })?;
        // Migrated rather than parsed strictly: an editor that cannot open a
        // scene written by an older Sindri is an editor that loses work.
        Ok(Self {
            path: Some(path.to_path_buf()),
            document: SceneDocument::from_json_migrated(&text, &SceneMigrator::builtin())?,
        })
    }

    /// A scene with no file behind it.
    ///
    /// Used when the editor is started somewhere the default scene is not, so
    /// it still opens and says why rather than refusing to start. Saving is
    /// unavailable until a path exists, which the interface reflects.
    pub const fn detached(document: SceneDocument) -> Self {
        Self {
            path: None,
            document,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The document as last read from or written to disk.
    ///
    /// This is what "reset to authored" resets to, so it follows a save: after
    /// saving, the file is what the scene was authored as.
    pub const fn document(&self) -> &SceneDocument {
        &self.document
    }

    /// The file's name, for an interface that has one line to spend on it.
    pub fn label(&self) -> String {
        self.path.as_ref().map_or_else(
            || "untitled scene".to_owned(),
            |path| {
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
        )
    }

    /// Writes a world back to the file it was opened from.
    ///
    /// The bytes are canonical, so saving a scene nobody edited reproduces the
    /// file exactly and a review sees only what changed.
    pub fn save(&mut self, world: &World) -> Result<(), SceneFileError> {
        let path = self.path.clone().ok_or(SceneFileError::NoPath)?;
        let document = world.to_scene()?;
        let text = document.to_canonical_json()?;
        std::fs::write(&path, text).map_err(|source| SceneFileError::Write {
            path: path.display().to_string(),
            source,
        })?;
        self.document = document;
        Ok(())
    }

    /// Re-reads the file, discarding whatever the editor had in memory.
    pub fn reload(&mut self) -> Result<(), SceneFileError> {
        let path = self.path.clone().ok_or(SceneFileError::NoPath)?;
        *self = Self::open(path)?;
        Ok(())
    }
}

impl fmt::Display for SceneFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{}", path.display()),
            None => formatter.write_str("untitled scene"),
        }
    }
}

#[derive(Debug, Error)]
pub enum SceneFileError {
    #[error("could not read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("this scene has no file to save to")]
    NoPath,
    #[error(transparent)]
    Scene(#[from] SceneJsonError),
    #[error(transparent)]
    World(#[from] WorldError),
}

#[cfg(test)]
mod tests {
    use sindri_core::{EntityData, SceneEntity, SceneEntityId, Transform3D};

    use super::*;

    fn authored_json() -> String {
        let mut entity = SceneEntity::new(SceneEntityId::new("cube").unwrap());
        entity.transform_3d = Some(Transform3D::default());
        let mut document = SceneDocument::default();
        document.entities.push(entity);
        document.to_canonical_json().unwrap()
    }

    fn written(directory: &Path, text: &str) -> PathBuf {
        let path = directory.join("scene.json");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_scene_edited_and_saved_reopens_as_it_was_left() {
        let directory = tempfile::tempdir().unwrap();
        let path = written(directory.path(), &authored_json());

        let mut file = SceneFile::open(&path).unwrap();
        let mut world = World::from_scene(file.document()).unwrap().world;
        let entity = world.entities().next().map(|(entity, _)| entity).unwrap();
        world.get_mut(entity).unwrap().transform_3d = Some(Transform3D {
            position: [1.5, -2.0, 3.25],
            ..Transform3D::default()
        });
        let edited = world.to_scene().unwrap();
        assert_ne!(
            &edited,
            file.document(),
            "the edit should have changed the document"
        );
        file.save(&world).unwrap();

        let reopened = SceneFile::open(&path).unwrap();
        assert_eq!(
            reopened.document(),
            &edited,
            "the edit did not survive the round trip through the file"
        );
    }

    /// Saving is only safe to offer if an untouched scene comes back unchanged.
    /// Canonical output makes that true; this is what proves it stays true.
    #[test]
    fn saving_an_unedited_scene_leaves_the_file_byte_for_byte_identical() {
        let directory = tempfile::tempdir().unwrap();
        let original = authored_json();
        let path = written(directory.path(), &original);

        let mut file = SceneFile::open(&path).unwrap();
        let world = World::from_scene(file.document()).unwrap().world;
        file.save(&world).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn reloading_discards_what_was_never_saved() {
        let directory = tempfile::tempdir().unwrap();
        let path = written(directory.path(), &authored_json());
        let mut file = SceneFile::open(&path).unwrap();

        let mut world = World::from_scene(file.document()).unwrap().world;
        world.spawn(EntityData::default());
        assert_eq!(world.len(), 2);

        file.reload().unwrap();
        let reloaded = World::from_scene(file.document()).unwrap().world;
        assert_eq!(
            reloaded.len(),
            1,
            "reload should have dropped the new entity"
        );
    }

    #[test]
    fn a_detached_scene_reports_that_it_has_nowhere_to_save() {
        let document = SceneDocument::from_json(&authored_json()).unwrap();
        let mut file = SceneFile::detached(document);
        let world = World::from_scene(file.document()).unwrap().world;

        assert!(file.path().is_none());
        assert!(matches!(file.save(&world), Err(SceneFileError::NoPath)));
    }

    #[test]
    fn a_missing_file_names_itself_in_the_error() {
        let error = SceneFile::open("definitely/not/here.scene.json")
            .expect_err("opening a missing scene fails");
        assert!(
            error.to_string().contains("definitely/not/here.scene.json"),
            "{error}"
        );
    }
}
