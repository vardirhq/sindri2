//! Opening a project, making one, and following a scene to the one it is in.
//!
//! Apart from `welcome/` because these are not the window. The window is one
//! way to ask for a project; the File menu is another, a path on the command
//! line is a third, and opening a scene that happens to live inside a project
//! is a fourth that nobody asks for at all. What they have in common is what is
//! here: the editor arranging itself around a project, once, so that four ways
//! in cannot drift into four arrangements.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::project::{Project, ProjectTree, manifest};

use super::EditorApp;
use super::unsaved::Discarding;

impl EditorApp {
    /// Makes a project and opens it.
    ///
    /// The blank scene comes from the same place New Scene's does — the
    /// component registry's own default payload — rather than from a second
    /// copy of that answer living beside the project format.
    pub(super) fn create_project(&mut self, root: &Path, name: &str) {
        let document = super::scene_new::blank_scene(&self.scene, &root.join("main.scene.json"));
        match Project::create(root, name, &document) {
            Ok(project) => {
                self.console.info(format!(
                    "Created {} in {}",
                    project.name(),
                    project.root().display()
                ));
                self.open_project(&project);
            }
            Err(error) => self.tell_welcome(error.to_string()),
        }
    }

    /// Opens a project by its root.
    pub(super) fn open_project_at(&mut self, root: &Path) {
        match Project::open(root) {
            Ok(project) => self.open_project(&project),
            Err(error) => self.tell_welcome(error.to_string()),
        }
    }

    /// Opens a project the editor has already read.
    ///
    /// The scene comes second and is allowed to be absent: a project whose
    /// manifest nominates no scene, or nominates one that has been deleted, is
    /// still a project worth opening — the browser shows its files, and the
    /// editor says there is nothing open rather than refusing the project.
    fn open_project(&mut self, project: &Project) {
        self.preferences.recent_projects.remember(project);
        self.adopt(project);
        if let Some(scene) = self.scene_for(project) {
            self.open_path(&scene);
        } else {
            self.project = self.project_tree();
            self.console.info(format!(
                "Opened {} - no scene is nominated in {}",
                project.name(),
                manifest::MANIFEST_NAME
            ));
        }
        // Only once the project is actually open: a failed scene load leaves
        // the editor where it was, and hiding the window someone is working in
        // behind a welcome screen would be the worse half of that failure.
        self.welcome = None;
    }

    /// Which of a project's scenes opening it opens.
    ///
    /// The scene the editor was last left in when that scene is inside this
    /// project, and the project's nominated scene otherwise. Reopening a
    /// project should put someone back where they were working rather than at
    /// its front door, and `main_scene` is what a project opens on the first
    /// time and after working somewhere else.
    fn scene_for(&self, project: &Project) -> Option<PathBuf> {
        self.preferences
            .last_scene
            .as_ref()
            .map(PathBuf::from)
            .filter(|scene| scene.starts_with(project.root()) && scene.is_file())
            .or_else(|| project.main_scene())
    }

    /// Asks for a folder and opens the project in it.
    ///
    /// Through the same confirmation every other way of replacing the open
    /// scene goes through: a project is a scene and everything around it, so
    /// opening one costs at least as much unsaved work as opening a scene.
    pub(super) fn browse_for_project(&mut self, context: &egui::Context) {
        let Some(root) = rfd::FileDialog::new()
            .set_directory(self.scene_directory())
            .pick_folder()
        else {
            return;
        };
        self.discard_or_confirm(Discarding::OpenProject(root), context);
    }

    /// The browser's view of whatever is open.
    ///
    /// Rooted at the project when there is one, so the browser shows the whole
    /// project rather than the folder the open scene happens to sit in, and
    /// named by the manifest so a game called Gather is not listed as `assets`.
    pub(super) fn project_tree(&self) -> ProjectTree {
        match (&self.open_project_root, &self.project_name) {
            (Some(root), Some(name)) => ProjectTree::rooted_as(root, name),
            (Some(root), None) => ProjectTree::rooted(root),
            _ => ProjectTree::beside(self.file.path()),
        }
    }

    /// Follows a scene to whichever project it belongs to.
    ///
    /// Called whenever a scene is opened, so a scene opened from the command
    /// line or from a file dialog opens *as* its project — walking up from the
    /// file to the nearest `sindri.toml`. A scene in no project leaves the
    /// editor with none, which is the state it was always in before projects
    /// existed and is still a perfectly good way to edit one file.
    pub(super) fn adopt_project_for(&mut self, scene: &Path) {
        let project = manifest::root_for(scene).and_then(|root| Project::open(&root).ok());
        let Some(project) = project else {
            self.project_name = None;
            self.open_project_root = None;
            self.project_main_scene = None;
            return;
        };
        self.preferences.recent_projects.remember(&project);
        self.adopt(&project);
    }

    /// Records what the open project is.
    ///
    /// Three fields read off one manifest, so they are written in one place:
    /// they have to agree, and the browser reads all three every frame it
    /// draws. Held rather than re-read because a manifest does not change at
    /// the frame rate a viewport does — the editor is the only thing that
    /// changes it, and it does so through `set_main_scene`.
    fn adopt(&mut self, project: &Project) {
        self.project_name = Some(project.name().to_owned());
        self.open_project_root = Some(project.root().to_path_buf());
        self.project_main_scene = project.main_scene();
    }

    /// Nominates a scene as the one its project opens on.
    ///
    /// The manifest field had no way to change: it was written when a project
    /// was created and then only editable by hand, so a project whose first
    /// scene turned out to be a sketch opened on the sketch for ever.
    ///
    /// Re-read from disk rather than kept in memory, because the editor holds a
    /// project root and a name rather than the manifest itself: this is a
    /// deliberate act on a file somebody may have edited since, and a write
    /// from a stale copy would put back whatever else they had changed.
    pub(super) fn set_main_scene(&mut self, scene: &Path) {
        let Some(root) = self.open_project_root.clone() else {
            self.report("There is no project open to set a main scene for".to_owned());
            return;
        };
        let mut project = match Project::open(&root) {
            Ok(project) => project,
            Err(error) => {
                self.report(error.to_string());
                return;
            }
        };
        match project.set_main_scene(scene) {
            Ok(()) => {
                self.adopt(&project);
                self.console.info(format!(
                    "{} opens {} now",
                    project.name(),
                    scene.strip_prefix(&root).unwrap_or(scene).display()
                ));
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Nominates a new scene when its project has nobody to open.
    ///
    /// Making the first scene in a project made by New Project is the ordinary
    /// case — a project is created with one, and the second thing anyone does
    /// is replace it — but a project whose nominated scene was deleted, or one
    /// made by hand with none, would otherwise keep opening on nothing however
    /// many scenes were added to it.
    ///
    /// Only when there is nothing to overwrite. Silently re-pointing a project
    /// at whichever scene was made last would be deciding something the author
    /// already decided, and the browser's own entry is how they change it.
    pub(super) fn nominate_if_unclaimed(&mut self, scene: &Path) {
        if nominates(
            self.open_project_root.as_deref(),
            self.project_main_scene.as_deref(),
            scene,
        ) {
            self.set_main_scene(scene);
        }
    }
}

/// Whether a scene just made should become the one its project opens on.
///
/// A rule rather than a condition written inline, because it is three questions
/// that each have a wrong answer: a scene outside the project is not the
/// project's to open, a project that already nominates one has been decided
/// about, and with no project open there is nothing to nominate it in.
fn nominates(root: Option<&Path>, main: Option<&Path>, scene: &Path) -> bool {
    root.is_some_and(|root| scene.starts_with(root)) && main.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_with_no_scene_takes_the_one_just_made() {
        assert!(nominates(
            Some(Path::new("/games/mine")),
            None,
            Path::new("/games/mine/main.scene.json")
        ));
    }

    #[test]
    fn a_project_that_already_opens_on_a_scene_is_left_alone() {
        assert!(
            !nominates(
                Some(Path::new("/games/mine")),
                Some(Path::new("/games/mine/main.scene.json")),
                Path::new("/games/mine/sketch.scene.json")
            ),
            "the author chose one, and making another is not changing their mind"
        );
    }

    #[test]
    fn a_scene_made_outside_the_project_is_not_the_projects_to_open() {
        assert!(!nominates(
            Some(Path::new("/games/mine")),
            None,
            Path::new("/elsewhere/loose.scene.json")
        ));
    }

    #[test]
    fn with_no_project_open_there_is_nothing_to_nominate_in() {
        assert!(!nominates(None, None, Path::new("/loose.scene.json")));
    }
}
