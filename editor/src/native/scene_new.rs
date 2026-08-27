//! Making a scene, and putting one somewhere it has not been.
//!
//! Apart from `scene_io` because they answer a different question. That module
//! is about the scene the editor has open: reading it, writing it back,
//! re-reading it. This is about the two moments a scene file does not exist
//! yet — New Scene and Save As — which is what the editor could not do at all.
//! A scene had to exist before the editor could touch it, so the tool could
//! only continue a project someone else had started.

use std::path::{Path, PathBuf};

use sindri_core::{SceneDocument, SceneEntity, SceneEntityId, SceneMetadata, Transform3D};
use sindri_scene::SceneExtractor;

use crate::scene_file::{SceneFile, scene_path};

use super::CAMERA_COMPONENT;
use super::EditorApp;
use super::hierarchy::row::humanize;
use super::runtime::PLAYING_TIP;

/// What a brand-new scene contains.
///
/// One world camera, and a name taken from the file. A scene with no camera is
/// a legal scene and a black Game view — the extract draws the player's view
/// through exactly one authored world camera — and "why is the game view empty"
/// is not the first question a new project should raise. Its transform is the
/// one every shipped scene uses: back along +Z far enough to see the origin.
///
/// The camera's payload comes from the registry rather than from a literal
/// here, for the same reason every other component the editor creates does: a
/// second copy of a default is a copy that drifts.
pub(super) fn blank_scene(scene: &SceneExtractor, path: &Path) -> SceneDocument {
    let mut camera =
        SceneEntity::new(SceneEntityId::new("world-camera").expect("a literal ID is not empty"));
    camera.name = Some("World Camera".to_owned());
    camera.transform_3d = Some(Transform3D {
        position: [0.0, 0.0, 9.0],
        ..Transform3D::default()
    });
    if let Some(payload) = scene.components().default_payload(CAMERA_COMPONENT) {
        camera
            .components
            .insert(CAMERA_COMPONENT.to_owned(), payload.clone());
    }
    SceneDocument {
        metadata: SceneMetadata {
            name: Some(scene_name(path)),
            ..SceneMetadata::default()
        },
        entities: vec![camera],
        ..SceneDocument::default()
    }
}

/// What a scene made at this path is called.
///
/// `SceneMetadata.name` is a real field that round-trips through a save — the
/// shipped Gather scene is called "Gather" — and nothing in the editor shows or
/// edits it yet. Deriving it from the file name is what stops a new scene being
/// the one document that has none.
fn scene_name(path: &Path) -> String {
    let file = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let stem = file
        .strip_suffix(".scene.json")
        .or_else(|| file.strip_suffix(".json"))
        .unwrap_or(&file);
    humanize(stem)
}

impl EditorApp {
    /// Writes the world somewhere else, and works there from now on.
    ///
    /// The one way a detached scene becomes a file: started somewhere the
    /// default scene is not, the editor opened on nothing with Save disabled
    /// and no way to make it possible.
    pub(super) fn save_as(&mut self) {
        if !self.authoring_enabled() {
            self.report(format!("Not saved. {PLAYING_TIP}"));
            return;
        }
        let Some(path) = self.ask_for_scene_path(&self.file.label()) else {
            return;
        };
        if let Err(error) = self.file.save_as(&path, &self.world) {
            self.report(error.to_string());
            return;
        }
        self.saved_revision = self.history.revision();
        self.notice = None;
        self.console.info(format!("Saved {}", self.file.label()));
        // The scene lives in a new directory, so what is beside it, what the
        // editor reopens on, and what its assets resolve against all move with
        // it. A fork saved into another project that then read the old one's
        // textures would be a scene that looks right only in the editor.
        self.refresh_project();
        self.remember_open_scene();
        self.reload_textures();
        self.reload_scripts();
    }

    /// Makes a scene file and opens it.
    ///
    /// A scene has to exist before the editor can do anything with it, so
    /// until this the editor could only continue a project someone else had
    /// started. It is written to disk and then opened through the ordinary
    /// path rather than adopted in memory: a new scene proves it loads before
    /// anyone starts working in it, and everything a scene brings with it —
    /// the project beside it, the textures, the scripts — is arranged once, by
    /// the code that already knows how.
    pub(super) fn new_scene(&mut self) {
        let Some(path) = self.ask_for_scene_path("untitled.scene.json") else {
            return;
        };
        let document = blank_scene(&self.scene, &path);
        if let Err(error) = SceneFile::create(&path, &document) {
            self.report(error.to_string());
            return;
        }
        self.open_path(&path);
    }

    /// Asks where a scene should go, starting where the open one is.
    fn ask_for_scene_path(&self, suggested: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Sindri scene", &["json"])
            .set_directory(self.scene_directory())
            .set_file_name(suggested)
            .save_file()
            // A save box takes a name rather than an extension, and a scene the
            // browser can list is `*.scene.json`.
            .map(|chosen| scene_path(&chosen))
    }
}
