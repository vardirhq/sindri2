//! Carrying out what the browser's menu asked for.
//!
//! Every one of these writes to disk rather than to the world, so none of them
//! goes through the command history. What keeps them honest instead: each is
//! checked by `project::ops` before it runs and refuses rather than
//! overwrites, the directory is re-read afterwards so the browser shows what
//! actually happened, and the one that cannot be taken back asks first.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout};

use crate::project::ops;
use crate::ui::widgets::button::{self, Intent};
use crate::ui::widgets::dialog;

use super::super::EditorApp;

impl EditorApp {
    /// Starts renaming an asset, with its current name as the draft.
    pub(in crate::native) fn begin_asset_rename(&mut self, path: &Path) {
        self.browser.selected = Some(path.to_path_buf());
        self.asset_rename = Some((path.to_path_buf(), named(path)));
    }

    /// Asks before removing a file, and removes it when the answer is yes.
    ///
    /// The only question the browser stops to ask. Every other file operation
    /// either refuses or can be undone by doing the opposite; this one cannot
    /// be undone at all, and on a folder it takes everything underneath.
    ///
    /// Returns `true` while the question is on screen, so the frame's
    /// remaining input handling stands down.
    pub(in crate::native) fn confirm_delete(&mut self, context: &egui::Context) -> bool {
        let Some(path) = self.deleting.clone() else {
            return false;
        };
        let folder = path.is_dir();
        let question = if folder {
            format!(
                "Delete the folder '{}' and everything in it? This cannot be undone.",
                named(&path)
            )
        } else {
            format!(
                "Delete '{}' from the project? This cannot be undone.",
                named(&path)
            )
        };
        let mut answered = None;
        dialog::ask(context, "sindri-delete-asset", "Delete", &question, |ui| {
            if button::labelled(ui, "Cancel", Intent::Quiet, "Leave the file where it is").clicked()
            {
                answered = Some(false);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button::labelled(ui, "Delete", Intent::Danger, "Remove it from disk").clicked() {
                    answered = Some(true);
                }
            });
        });
        match answered {
            None => {}
            Some(false) => self.deleting = None,
            Some(true) => {
                self.deleting = None;
                self.delete_asset(&path);
            }
        }
        self.deleting.is_some()
    }

    /// Makes a folder and starts renaming it, so it is named once rather than
    /// made as `New folder` and renamed afterwards.
    pub(super) fn new_folder(&mut self, beside: &Path) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        let Some(parent) = directory_for(beside) else {
            return;
        };
        match ops::create_folder(&root, &parent, &unused_folder_name(&parent)) {
            Ok(path) => {
                self.refresh_project();
                self.browser.selected = Some(path.clone());
                self.begin_asset_rename(&path);
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Makes a `.decay` script and starts renaming it.
    ///
    /// The browser listed the language's own source files and could not make
    /// one, in an engine whose headline capability is scripting. What it
    /// writes is a script that compiles and does nothing, because a file that
    /// reports an error the moment it is created is a worse start than an
    /// empty one.
    pub(super) fn new_script(&mut self, beside: &Path) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        let Some(parent) = directory_for(beside) else {
            return;
        };
        let name = unused_name(&parent, "script", ".decay");
        match ops::create_file(&root, &parent, &name, STARTER_SCRIPT) {
            Ok(path) => {
                self.refresh_project();
                self.select_asset(&path);
                self.begin_asset_rename(&path);
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Copies a file or folder beside itself.
    pub(in crate::native) fn duplicate_asset(&mut self, path: &Path) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        match ops::duplicate(&root, path) {
            Ok(copy) => {
                self.refresh_project();
                self.browser.selected = Some(copy.clone());
                self.console.info(format!("Copied to {}", named(&copy)));
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Renames a file or folder, and follows it if it is the open scene.
    ///
    /// Following matters: the editor holds the path it saves to, so renaming
    /// the scene you are working in without telling the editor would have the
    /// next save write the file back under its old name.
    pub(super) fn rename_asset(&mut self, path: &Path, name: &str) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        let was_open = self.file.path() == Some(path);
        match ops::rename(&root, path, name) {
            Ok(target) => {
                if was_open {
                    self.file.adopt(&target);
                    self.remember_open_scene();
                }
                self.refresh_project();
                // Whatever the inspector was showing of this file is showing
                // the old path; re-opening it is how it follows the rename.
                self.browser.selected = Some(target.clone());
                if self
                    .preview
                    .as_ref()
                    .is_some_and(|open| open.path() == path)
                {
                    self.preview = Some(crate::preview::TextPreview::open(&target));
                }
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Removes a file or folder from disk.
    ///
    /// Called only after the question has been answered: there is no undo for
    /// this, and a folder takes everything under it.
    pub(super) fn delete_asset(&mut self, path: &Path) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        match ops::delete(&root, path) {
            Ok(()) => {
                if self.browser.selected.as_deref() == Some(path) {
                    self.browser.selected = None;
                }
                // The file the inspector is showing may be the one just
                // removed, in either of the two ways it can be showing one.
                if self.slicer.as_ref().is_some_and(|open| open.path() == path) {
                    self.slicer = None;
                }
                if self
                    .preview
                    .as_ref()
                    .is_some_and(|open| open.path() == path)
                {
                    self.preview = None;
                }
                self.refresh_project();
                self.console.info(format!("Deleted {}", named(path)));
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Copies files chosen from anywhere into the project.
    pub(super) fn import_assets(&mut self, into: &Path) {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return;
        };
        let Some(into) = directory_for(into) else {
            return;
        };
        let Some(chosen) = rfd::FileDialog::new().set_directory(&root).pick_files() else {
            return;
        };
        let (arrived, refused) = ops::import(&root, &into, &chosen);
        for error in refused {
            self.report(error.to_string());
        }
        if arrived.is_empty() {
            return;
        }
        self.console
            .info(format!("Imported {} file(s)", arrived.len()));
        self.refresh_project();
        self.browser.selected = arrived.last().cloned();
        // An imported texture is one the open scene may already name, so the
        // registry that resolves those is asked to look again.
        self.reload_textures();
    }
}

/// The directory a new file belongs in, given the row it was asked from.
///
/// A folder means inside it; anything else means beside it, which is the folder
/// it is already in. "New folder here" on `textures/orb.png` makes one in
/// `textures/`, which is where "here" points.
fn directory_for(row: &Path) -> Option<PathBuf> {
    if row.is_dir() {
        return Some(row.to_path_buf());
    }
    row.parent().map(Path::to_path_buf)
}

/// What a new script starts as.
///
/// A container the scene can name and an `update` that runs, so the file
/// compiles the moment it exists: a script that reports an error before anyone
/// has written a line of it is a worse start than an empty one.
const STARTER_SCRIPT: &str = "script Script {\n    fn update(dt: f32) {\n    }\n}\n";

/// A name nothing in `parent` is using yet.
fn unused_name(parent: &Path, stem: &str, suffix: &str) -> String {
    let mut candidate = format!("{stem}{suffix}");
    let mut nth = 2_u32;
    while parent.join(&candidate).exists() {
        candidate = format!("{stem} {nth}{suffix}");
        nth += 1;
    }
    candidate
}

/// A folder name nothing in `parent` is using yet.
fn unused_folder_name(parent: &Path) -> String {
    unused_name(parent, "New folder", "")
}

/// What to call a path in a one-line message.
fn named(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
