//! The window the editor opens before it opens anything else.
//!
//! Until this existed the editor started *somewhere* whatever happened: the
//! scene named on the command line, the one it was last left in, or the demo
//! scene compiled into the repository. That is a reasonable answer to "which
//! file" and no answer at all to "which project" — there was nowhere to see
//! what you had been working on, no way to reach a project you had not opened
//! recently, and no way to start one that did not begin with a save dialog and
//! a folder you made yourself in a file manager.
//!
//! It is its own window rather than a screen inside the editor's, because it is
//! not part of editing anything. The editor's window is about a scene: it has a
//! title carrying that scene's name, panels holding that scene's entities, and
//! a viewport rendering it. A "no project open" state painted over all of that
//! would be an editor pretending to be a launcher.
//!
//! What it is not is Unity Hub. There are no editor versions to install, no
//! account, no news. What is left is small and worth doing well: the projects
//! you have, the two ways to get another one, and the projects this repository
//! ships so that a clean clone still has something to open.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui::{self, ViewportBuilder, ViewportId};

use crate::preferences::Preferences;
use crate::project::{Project, ProjectTree, RecentProjects, manifest};

use super::EditorApp;
use super::unsaved::Discarding;

mod create;
mod view;

use create::NewProject;

/// The welcome window's viewport, which is also how egui knows it is one window
/// rather than a new one every frame.
const VIEWPORT: &str = "sindri-welcome";

/// What the window is called.
///
/// Deliberately not "Sindri Editor": `scripts/capture-editor.sh` finds the
/// editor by matching a title ending in that, and a second window answering to
/// the same name would have CI photograph whichever one it found first.
const TITLE: &str = "Sindri";

const SIZE: [f32; 2] = [900.0, 580.0];
const MIN_SIZE: [f32; 2] = [640.0, 420.0];

/// The projects this repository ships, offered when they are actually there.
///
/// Relative to the working directory, which is the repository root when the
/// editor is run with `cargo run`. An installed editor is somewhere else and
/// finds none of them, which is why a sample is listed only when it opens: a
/// row that fails on the click is worse than no row.
///
/// Gather alone, because Gather alone is a project. The cube example is a scene
/// and a texture rather than something anyone would work in, and making it a
/// project to pad this list would be inventing a project to have two of them.
const SHIPPED: [&str; 1] = ["game"];

/// A project the window can offer, as a row.
pub(super) struct Listing {
    pub(super) name: String,
    pub(super) root: PathBuf,
    /// Whether the project is still where it was remembered from.
    pub(super) present: bool,
}

/// What the user asked the editor for, waiting to be acted on.
///
/// The window cannot do these itself. Opening a project means loading a scene
/// into a world, and creating one means asking the component registry what a
/// blank scene contains — both belong to the editor, and the window runs in a
/// callback that only reaches this state.
pub(super) enum Request {
    Open(PathBuf),
    Create { root: PathBuf, name: String },
}

/// Everything the welcome window shows and everything it has been told.
///
/// Shared with its viewport callback, which egui requires to be `Send + Sync`,
/// so this is behind a mutex rather than borrowed from the editor. That is also
/// why nothing here is a handle into the editor's state: the window holds its
/// own copy of what it lists, and the editor reads back what changed.
pub(super) struct Welcome {
    /// The remembered projects, as the preferences hold them.
    recent: RecentProjects,
    /// The shipped projects that are on disk.
    samples: Vec<Listing>,
    /// Whether the next launch should skip this window.
    open_last: bool,
    /// The new-project form, while it is open.
    creating: Option<NewProject>,
    /// What went wrong with the last thing the window was asked to do.
    problem: Option<String>,
    /// What the editor has to act on.
    request: Option<Request>,
    /// Whether the list or the launch preference changed since the editor last
    /// looked, so preferences are written when they are and not every frame.
    changed: bool,
    /// Whether the window has been closed.
    dismissed: bool,
}

impl Welcome {
    fn new(preferences: &Preferences) -> Self {
        Self {
            recent: preferences.recent_projects.clone(),
            samples: shipped_samples(),
            open_last: preferences.open_last_project,
            creating: None,
            problem: None,
            request: None,
            changed: false,
            dismissed: false,
        }
    }

    /// Whether the editor has something to come and collect.
    const fn waiting(&self) -> bool {
        self.request.is_some() || self.changed || self.dismissed
    }

    /// The remembered projects as rows, checked against the file system.
    ///
    /// Checked here rather than when they were stored, because a project can
    /// move, be deleted, or sit on a volume that is not mounted this morning.
    /// The window redraws only when something happens to it, so this is a
    /// directory check per row per interaction rather than per frame.
    fn rows(&self) -> Vec<Listing> {
        self.recent
            .entries()
            .iter()
            .map(|entry| Listing {
                name: entry.name.clone(),
                root: PathBuf::from(&entry.path),
                present: entry.is_present(),
            })
            .collect()
    }
}

/// The shipped projects that exist and open.
fn shipped_samples() -> Vec<Listing> {
    SHIPPED
        .iter()
        .filter_map(|relative| {
            let root = PathBuf::from(relative);
            let project = Project::open(&root).ok()?;
            Some(Listing {
                name: project.name().to_owned(),
                root,
                present: true,
            })
        })
        .collect()
}

/// Locks the shared state, taking it back even if a draw panicked in it.
///
/// A poisoned mutex here would mean the window's own callback failed, which is
/// not a reason for the editor behind it to stop as well.
fn state(welcome: &Mutex<Welcome>) -> std::sync::MutexGuard<'_, Welcome> {
    welcome.lock().unwrap_or_else(PoisonError::into_inner)
}

impl EditorApp {
    /// Opens the welcome window.
    pub(super) fn open_welcome(&mut self) {
        if self.welcome.is_none() {
            self.welcome = Some(Arc::new(Mutex::new(Welcome::new(&self.preferences))));
        }
    }

    /// Opens the welcome window with the new-project form already up.
    pub(super) fn open_welcome_creating(&mut self) {
        self.open_welcome();
        if let Some(welcome) = &self.welcome {
            state(welcome).creating = Some(NewProject::default());
        }
    }

    /// Whether the editor is waiting on the welcome window rather than showing
    /// anything of its own.
    ///
    /// True only while its own window is still hidden: once a project has been
    /// opened the editor is what the user is looking at, and the welcome window
    /// reopened from the File menu is a window in front of a working editor.
    pub(super) const fn awaiting_welcome(&self) -> bool {
        self.welcome.is_some() && !self.window_shown
    }

    /// Draws the welcome window and acts on whatever it was told.
    ///
    /// A deferred viewport rather than an immediate one: eframe throttles a
    /// hidden window to ten frames a second so that a `Visible` command still
    /// gets through, and an immediate child viewport is drawn by its parent —
    /// which would make the one window the user can see repaint at the rate of
    /// the one they cannot.
    pub(super) fn show_welcome(&mut self, context: &egui::Context) {
        let Some(welcome) = self.welcome.clone() else {
            return;
        };
        // Where multiple windows are unavailable, egui draws a child viewport
        // inside its parent — so the parent has to be on screen or the welcome
        // window is painted somewhere nobody can see.
        if context.embed_viewports() {
            self.show_window(context);
        }
        let drawn = welcome.clone();
        context.show_viewport_deferred(
            ViewportId::from_hash_of(VIEWPORT),
            ViewportBuilder::default()
                .with_title(TITLE)
                .with_inner_size(SIZE)
                .with_min_inner_size(MIN_SIZE),
            move |ui, _class| {
                let mut welcome = state(&drawn);
                welcome.draw(ui);
                if ui.ctx().input(|input| input.viewport().close_requested()) {
                    welcome.dismissed = true;
                }
                // The editor is hidden, and a hidden window is not repainting
                // for anything of its own: it has no scene open, nothing is
                // animating, and eframe paints it only when it has asked to be
                // painted. So the window that took the instruction wakes the
                // one that carries it out. Without this the click landed, the
                // request was recorded, and nothing ever read it.
                if welcome.waiting() {
                    ui.ctx().request_repaint_of(ViewportId::ROOT);
                }
            },
        );
        self.handle_welcome(context, &welcome);
    }

    /// Reads back what the welcome window was told, and does it.
    fn handle_welcome(&mut self, context: &egui::Context, welcome: &Mutex<Welcome>) {
        let (request, changed, dismissed, recent, open_last) = {
            let mut welcome = state(welcome);
            (
                welcome.request.take(),
                std::mem::take(&mut welcome.changed),
                welcome.dismissed,
                welcome.recent.clone(),
                welcome.open_last,
            )
        };
        if changed {
            self.preferences.recent_projects = recent;
            self.preferences.open_last_project = open_last;
        }
        match request {
            Some(Request::Open(root)) => {
                self.discard_or_confirm(Discarding::OpenProject(root), context);
            }
            Some(Request::Create { root, name }) => self.create_project(&root, &name),
            None => {}
        }
        if dismissed {
            self.dismiss_welcome(context);
        }
    }

    /// Closes the welcome window.
    ///
    /// Closing it with no project open closes the editor. The editor's own
    /// window is hidden at that point, so leaving it running would leave a
    /// process with nothing on screen and no way back to it.
    fn dismiss_welcome(&mut self, context: &egui::Context) {
        let nothing_open = !self.window_shown;
        self.welcome = None;
        if nothing_open {
            self.closing = true;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Makes a project and opens it.
    ///
    /// The blank scene comes from the same place New Scene's does — the
    /// component registry's own default payload — rather than from a second
    /// copy of that answer living beside the project format.
    fn create_project(&mut self, root: &Path, name: &str) {
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
        self.project_name = Some(project.name().to_owned());
        self.open_project_root = Some(project.root().to_path_buf());
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

    /// Says something to the welcome window if it is up, and to the editor if
    /// it is not.
    ///
    /// A creation that failed has to be reported where the person who asked for
    /// it is looking, and that is the window they typed the name into.
    fn tell_welcome(&mut self, problem: String) {
        self.console.error(problem.clone());
        if let Some(welcome) = &self.welcome {
            state(welcome).problem = Some(problem);
        } else {
            self.report(problem);
        }
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
            return;
        };
        self.preferences.recent_projects.remember(&project);
        self.project_name = Some(project.name().to_owned());
        self.open_project_root = Some(project.root().to_path_buf());
    }

    /// Reveals the editor's own window.
    ///
    /// It starts hidden, because the alternative is an empty editor flashing up
    /// behind the welcome window on every launch. Sent once rather than every
    /// frame: a viewport command per frame is sixty round trips a second to say
    /// something that was already true.
    pub(super) fn show_window(&mut self, context: &egui::Context) {
        if self.window_shown {
            return;
        }
        context.send_viewport_cmd_to(ViewportId::ROOT, egui::ViewportCommand::Visible(true));
        self.window_shown = true;
    }
}
