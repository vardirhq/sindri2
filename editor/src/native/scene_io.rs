//! Opening, saving, and reloading a scene, and what the shell shows about it.

use std::path::{Path, PathBuf};

use eframe::egui::{self};
use sindri_core::{SceneDocument, UnknownComponentPolicy, World};
use sindri_decay::ScriptComponent;
use sindri_scene::{SceneExtractor, SpriteAnimations};

use crate::{scene_file::SceneFile, scripts::SceneScripts, textures::SceneTextures};

use super::EditorApp;
use super::runtime::{PLAYING_TIP, initialized_lifecycle};

/// One rendered view of the world, and the egui texture it is drawn through.
/// Everything a frame is derived from: the world, the schemas that read it, and
/// the runtime state beside it.
///
/// Together rather than as five arguments, because they are one thing — the
/// state of the open scene — and each view draws all of it or none of it.
#[derive(Clone, Copy)]
pub(super) struct SceneSource<'a> {
    pub(super) scene: &'a SceneExtractor,
    pub(super) world: &'a World,
    pub(super) animations: &'a SpriteAnimations,
    pub(super) effects: &'a sindri_scene::Effects2d,
    pub(super) textures: &'a SceneTextures,
}

/// Opens the scene a launch named.
///
/// No fallback. Standing a different scene in for the one someone just named on
/// the command line reads as though theirs opened, so a path that is missing or
/// unreadable is reported and the editor opens on nothing — a window that says
/// what went wrong beats one that never appears, and beats one quietly showing
/// somebody else's scene.
///
/// Which scene a launch with no argument means is not this function's question
/// any more. `project::launch` answers it, in the same order of deliberateness
/// this used to: the command line, then the project the editor was last left
/// in, then the welcome window.
pub(super) fn open_named_scene(path: &str) -> (SceneFile, Option<String>) {
    match SceneFile::open(path) {
        Ok(file) => (file, None),
        Err(error) => (
            SceneFile::detached(SceneDocument::default()),
            Some(error.to_string()),
        ),
    }
}

/// The component schemas the editor understands.
///
/// `sindri.script` is registered here rather than in `sindri-scene`, because
/// the engine's scene crate must not learn about a language —
/// `SceneExtractor::register` exists for exactly this, a component the host
/// brings of its own. It is one function rather than an inline registration so
/// that the editor and everything asserting about the editor's scenes agree by
/// construction instead of by both remembering the same list.
pub fn scene_extractor() -> SceneExtractor {
    let mut scene = SceneExtractor::new().expect("the built-in component schemas register");
    // Fields but no default: a script names a source and a container, and
    // neither is something the engine can invent. The editor completes both
    // from the project and from what the source declares, which is why Script
    // is addable at all.
    scene
        .register_with_fields::<ScriptComponent>(
            "Script",
            serde_json::json!({
                "source": "",
                "script": "",
                "properties": {},
                "enabled": true
            }),
        )
        .expect("sindri.script registers");
    scene
}

/// Builds a runtime world from a document the editor has opened.
///
/// `Preserve` rather than `Reject`: a scene may carry components this build has
/// never heard of, and the format exists to keep them through a load, an edit,
/// and a save. Rejecting them is how the editor came to refuse — and from the
/// command line, crash on — any project that defined a component of its own.
pub fn load_world(extractor: &SceneExtractor, document: &SceneDocument) -> Result<World, String> {
    extractor
        .validate(document, UnknownComponentPolicy::Preserve)
        .map_err(|error| error.to_string())?;
    Ok(World::from_scene(document)
        .map_err(|error| error.to_string())?
        .world)
}

impl EditorApp {
    /// Writes the world back to the file it came from.
    ///
    /// Refused while the scene is playing. `self.world` is the *running* world
    /// then, so this used to write wherever the scripts had pushed everything
    /// over the authored scene, mark the result saved, and then have Stop
    /// restore a world the file no longer held. The guard is here as well as on
    /// the controls because the keyboard reaches this directly.
    pub(super) fn save(&mut self) {
        if !self.authoring_enabled() {
            self.report(format!("Not saved. {PLAYING_TIP}"));
            return;
        }
        match self.file.save(&self.world) {
            Ok(()) => {
                self.saved_revision = self.history.revision();
                self.notice = None;
                self.console.info(format!("Saved {}", self.file.label()));
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// The directory a file dialog should open in.
    ///
    /// Beside the open scene, or the open project when there is no scene: a
    /// project can be opened with nothing in it yet, and a Save As that started
    /// in the working directory would offer to put the first scene somewhere
    /// outside the project it belongs to.
    pub(super) fn scene_directory(&self) -> PathBuf {
        self.file
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.open_project_root.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Asks for a scene file and opens it.
    ///
    /// Until this existed, the only way to open a scene was the command-line
    /// argument, which meant the editor could edit exactly the scene it was
    /// started on.
    pub(super) fn open_scene(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Sindri scene", &["json"])
            .set_directory(self.scene_directory())
            .pick_file()
        else {
            return;
        };
        self.open_path(&path);
    }

    /// Replaces the open scene with the one in `path`.
    ///
    /// A different scene is a different world, so every runtime handle in the
    /// old one is now meaningless: history is cleared rather than left pointing
    /// at entities that no longer exist, and so is the selection. The file is
    /// only adopted once its world loads, so a scene that fails to open leaves
    /// the editor on the one that was already working.
    pub(super) fn open_path(&mut self, path: &Path) {
        let opened = match SceneFile::open(path) {
            Ok(opened) => opened,
            Err(error) => {
                self.report(error.to_string());
                return;
            }
        };
        match load_world(&self.scene, opened.document()) {
            Ok(world) => {
                self.file = opened;
                // A scene carries its project with it: one opened from inside a
                // project opens that project, and one opened outside every
                // project leaves the editor with none.
                self.adopt_project_for(path);
                self.project = self.project_tree();
                self.world = world;
                self.history.clear();
                self.selection.clear();
                self.tilemap_tool.reset();
                self.animation_tool.reset();
                self.saved_revision = self.history.revision();
                self.lifecycle = initialized_lifecycle();
                // A cursor belongs to the world it was advanced against, and
                // a freshly loaded world reuses entity slots from the start.
                self.animations = SpriteAnimations::new();
                self.play_snapshot = None;
                self.notice = None;
                self.announce_scene();
                self.reload_textures();
                self.reload_scripts();
                self.remember_open_scene();
            }
            Err(error) => self.report(error),
        }
    }

    /// Re-reads the file, discarding unsaved edits along with their history.
    pub(super) fn reload(&mut self) {
        if let Err(error) = self.file.reload() {
            self.report(error.to_string());
            return;
        }
        self.refresh_project();
        self.reset_to_authored();
    }

    /// Re-reads the directory the browser is showing.
    ///
    /// The tree is cached, so a file added or removed outside the editor is
    /// invisible until something asks for it again. Opening or reloading a
    /// scene asks, and so does the browser's own refresh control — which is
    /// what used to be an inert filter icon.
    pub(super) fn refresh_project(&mut self) {
        self.project = self.project_tree();
        self.tilemap_tool.palette.invalidate();
        self.animation_tool.palette.invalidate();
    }

    /// Changes the selection, ending any in-progress merge run so the next
    /// edit starts its own undo step.
    /// What to show the user, preferring what they just did over what the
    /// renderer is doing, since an action's failure is the newer news.
    pub(super) fn problem(&self) -> Option<&str> {
        self.notice.as_deref().or(self.render_error.as_deref())
    }

    /// Records which scene to reopen on the next launch.
    ///
    /// A detached scene clears it rather than leaving the previous one: the
    /// editor should reopen where it was left, and it was left nowhere.
    pub(super) fn remember_open_scene(&mut self) {
        self.preferences.last_scene = self.file.path().map(|path| path.display().to_string());
    }

    /// Names the open scene in the window title.
    ///
    /// The title is where an operating system shows what a window is for — in a
    /// task switcher, a dock, a window list — and "Sindri Editor" answers that
    /// with the name of the program. The file goes first because that is the
    /// part a switcher has room for, and the unsaved marker goes with it,
    /// matching the status bar rather than inventing a second vocabulary.
    pub(super) fn update_title(&mut self, context: &egui::Context) {
        let title = format!(
            "{}{} - Sindri Editor",
            self.file.label(),
            if self.unsaved() { " (unsaved)" } else { "" }
        );
        if title == self.title {
            return;
        }
        context.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        self.title = title;
    }

    /// Says what a scene turned out to be once it was loaded.
    pub(super) fn announce_scene(&mut self) {
        self.console.info(format!(
            "Opened {} - {} entities",
            self.file.label(),
            self.world.len()
        ));
    }

    /// Starts the open scene's textures loading from its own directory.
    ///
    /// A new scene resolves its references against a new directory, so the
    /// whole set is rebuilt rather than re-rooted. That is also how the previous
    /// scene's textures are released: the registry that owned them is dropped,
    /// and a `Texture2D` frees its GPU texture when it goes.
    pub(super) fn reload_textures(&mut self) {
        let state = self.render_state.clone();
        self.renderers.text.clear_bindings();
        self.textures = SceneTextures::for_scene(&state.device, &state.queue, self.file.path());
        self.textured_revision = self.history.revision();
        let notes = self.textures.request(&self.world, &mut self.renderers.text);
        self.record_texture_notes(notes);
    }

    /// Asks again for whatever the world references, after an edit that could
    /// have changed it.
    pub(super) fn refresh_textures(&mut self) {
        // Scripts are asked every frame, and textures only after an edit,
        // because they answer to different clocks. A texture is named outright
        // by a component, so nothing but an edit can change which are wanted.
        // A *prefab* is named by the declared type of a compiled script's
        // export, so which are wanted changes when a script finishes
        // compiling -- frames after the scene opened, without touching the
        // world at all.
        //
        // Behind the edit gate, the only ask that ever happened was the one on
        // open, before any script had compiled, and no prefab was ever
        // discovered: the acceptance project opened in the editor with every
        // enemy missing and two hundred errors saying so, while the exported
        // build of the same project played.
        self.refresh_scripts();
        if self.textured_revision == self.history.revision() {
            return;
        }
        self.textured_revision = self.history.revision();
        let notes = self.textures.request(&self.world, &mut self.renderers.text);
        self.record_texture_notes(notes);
    }

    /// Asks again for whatever scripts the world names, after an edit that
    /// could have changed it.
    ///
    /// Shares `textured_revision` deliberately: both ask "has the world changed
    /// since we last looked", and two counters that must agree is one more
    /// thing to keep in step than there is any reason for.
    fn refresh_scripts(&mut self) {
        let notes = self.scripts.request(&self.world, self.scene.components());
        self.record_script_notes(notes);
    }

    /// Points the scripts at the open scene's directory and asks for its
    /// sources.
    ///
    /// Rebuilt rather than re-rooted for the same reason the textures are: a
    /// new scene resolves its references somewhere else, and the previous
    /// scene's sources and running instances go when the old one drops.
    pub(super) fn reload_scripts(&mut self) {
        self.scripts = SceneScripts::for_scene(self.file.path());
        let notes = self.scripts.request(&self.world, self.scene.components());
        self.record_script_notes(notes);
    }
}
