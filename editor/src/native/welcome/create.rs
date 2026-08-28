//! Making a project, from the two things only a person can supply.
//!
//! A name and somewhere to put it. Everything else a new project needs — the
//! manifest, the folders assets resolve from, a scene with a camera in it — is
//! the editor's answer rather than a question to ask, which is the whole
//! difference between this and the save dialog that used to be the only way to
//! start anything.
//!
//! The folder is shown before it is made. A name is not a path: "My First Game"
//! is a perfectly good project name and a poor directory name, and a form that
//! silently created `My First Game/` — or silently created `my-first-game/` —
//! would be deciding something the person typing can see and correct in advance.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout, RichText};

use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{
    button::{self, Intent},
    dialog,
};

use super::{Request, Welcome};

/// A project being described, before it exists.
#[derive(Clone, Debug)]
pub(super) struct NewProject {
    pub(super) name: String,
    pub(super) location: PathBuf,
    /// Whether the name field has been given the keyboard yet.
    ///
    /// Once, when the form opens. Asking for focus every frame is asking for it
    /// back from whatever the user just clicked on, which makes the Choose…
    /// button and the location beside it unreachable.
    focused: bool,
}

impl Default for NewProject {
    /// Somewhere that exists, which is where the editor was started.
    ///
    /// Not a guess at a documents folder: the editor would have to ask the
    /// operating system for one, and being wrong about it means offering to
    /// create a project in a directory the user has never seen.
    fn default() -> Self {
        Self {
            name: String::new(),
            location: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            focused: false,
        }
    }
}

impl NewProject {
    /// The folder this project would be created in.
    ///
    /// `None` while the name has nothing in it that can be a directory name,
    /// which is what leaves the Create button disabled rather than letting it
    /// fail on the press.
    pub(super) fn target(&self) -> Option<PathBuf> {
        let folder = folder_name(&self.name);
        (!folder.is_empty()).then(|| self.location.join(folder))
    }
}

/// The directory name a project name asks for.
///
/// Lowercase, with runs of anything that is not a letter or a digit turned into
/// a single dash. A project called "My First Game!" becomes `my-first-game`,
/// which is a name every file system, archive, and version control system
/// handles the same way — and the manifest keeps "My First Game" as what the
/// project is actually called.
fn folder_name(name: &str) -> String {
    let mut folder = String::new();
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() {
            folder.extend(character.to_lowercase());
        } else if !folder.ends_with('-') {
            folder.push('-');
        }
    }
    folder.trim_matches('-').to_owned()
}

impl Welcome {
    /// The new-project form, while it is open.
    pub(super) fn creating_form(&mut self, ui: &mut egui::Ui) {
        let Some(mut form) = self.creating.clone() else {
            return;
        };
        let mut answer = None;
        dialog::form(ui.ctx(), "sindri-new-project", "New project", |ui| {
            name_field(ui, &mut form);
            ui.add_space(metric::GROUP_GAP);
            location_field(ui, &mut form);
            ui.add_space(metric::GAP);
            let target = form.target();
            target_line(ui, target.as_deref());
            ui.add_space(14.0);
            answer = answers(ui, target);
        });

        match answer {
            Some(Answer::Create(root)) => {
                self.request = Some(Request::Create {
                    root,
                    name: form.name.trim().to_owned(),
                });
                self.problem = None;
                self.creating = None;
            }
            Some(Answer::Cancel) => self.creating = None,
            // Still being filled in, so the typing so far is kept: a form
            // rebuilt from its default every frame is a form nothing can be
            // typed into.
            None => self.creating = Some(form),
        }
    }
}

/// What finishing the form asked for.
enum Answer {
    Create(PathBuf),
    Cancel,
}

fn name_field(ui: &mut egui::Ui, form: &mut NewProject) {
    ui.label(
        RichText::new("Name")
            .size(text::LABEL)
            .color(color::TEXT_MUTED),
    );
    ui.add_space(3.0);
    let name = ui.add(
        egui::TextEdit::singleline(&mut form.name)
            .desired_width(f32::INFINITY)
            .hint_text("My First Game"),
    );
    if !form.focused {
        name.request_focus();
        form.focused = true;
    }
}

fn location_field(ui: &mut egui::Ui, form: &mut NewProject) {
    ui.label(
        RichText::new("Location")
            .size(text::LABEL)
            .color(color::TEXT_MUTED),
    );
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        // Truncated, with the whole path on hover: a location can be longer
        // than the modal, and a form that widened to fit one would be a form
        // whose width depended on where somebody keeps their projects.
        ui.add(
            egui::Label::new(
                RichText::new(form.location.display().to_string())
                    .size(text::BODY)
                    .color(color::TEXT),
            )
            .truncate(),
        )
        .on_hover_text(form.location.display().to_string());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if button::labelled(ui, "Choose…", Intent::Normal, "Pick another folder").clicked()
                && let Some(chosen) = rfd::FileDialog::new()
                    .set_directory(&form.location)
                    .pick_folder()
            {
                form.location = chosen;
            }
        });
    });
}

/// Where the project would be created, said before it is.
fn target_line(ui: &mut egui::Ui, target: Option<&Path>) {
    // A folder that already has files in it is not refused — the creation
    // itself refuses one that is already a project, and starting a project
    // beside existing work is a legitimate thing to do. It is said out loud,
    // because doing it by accident is not.
    let crowded = target.is_some_and(occupied);
    ui.label(
        RichText::new(match target {
            Some(target) if crowded => {
                format!(
                    "Creates {} — which already has files in it",
                    target.display()
                )
            }
            Some(target) => format!("Creates {}", target.display()),
            None => "Type a name to see where it goes".to_owned(),
        })
        .size(text::NOTE)
        .color(if crowded {
            color::WARNING
        } else {
            color::TEXT_FAINT
        }),
    );
}

/// The two ways out of the form.
fn answers(ui: &mut egui::Ui, target: Option<PathBuf>) -> Option<Answer> {
    let mut answer = None;
    ui.horizontal(|ui| {
        if button::labelled(ui, "Cancel", Intent::Quiet, "Do not make a project").clicked() {
            answer = Some(Answer::Cancel);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled_ui(target.is_some(), |ui| {
                    button::labelled(
                        ui,
                        "Create",
                        Intent::Primary,
                        "Make the project and open it",
                    )
                })
                .inner
                .clicked()
                && let Some(target) = target
            {
                answer = Some(Answer::Create(target));
            }
        });
    });
    answer
}

/// Whether a path already holds something, for a form that would rather say so
/// than overwrite it.
pub(super) fn occupied(target: &Path) -> bool {
    target
        .read_dir()
        .is_ok_and(|mut listing| listing.next().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A form as somebody would have typed it.
    fn form(name: &str, location: &str) -> NewProject {
        NewProject {
            name: name.to_owned(),
            location: PathBuf::from(location),
            ..NewProject::default()
        }
    }

    #[test]
    fn a_project_name_becomes_a_directory_name() {
        assert_eq!(folder_name("My First Game"), "my-first-game");
        assert_eq!(folder_name("Gather"), "gather");
        assert_eq!(folder_name("  Spaced  Out  "), "spaced-out");
    }

    #[test]
    fn punctuation_does_not_become_a_run_of_dashes() {
        assert_eq!(folder_name("My First Game!!!"), "my-first-game");
        assert_eq!(folder_name("a -- b"), "a-b");
        assert_eq!(folder_name("../etc/hosts"), "etc-hosts");
    }

    #[test]
    fn a_name_that_is_no_name_creates_nothing() {
        assert_eq!(folder_name("   "), "");
        assert_eq!(folder_name("!!!"), "");
        let form = form("!!!", "/projects");
        assert_eq!(
            form.target(),
            None,
            "the Create button is disabled rather than failing on the press"
        );
    }

    #[test]
    fn the_folder_is_shown_before_it_is_made() {
        let form = form("My First Game", "/projects");
        assert_eq!(
            form.target(),
            Some(PathBuf::from("/projects/my-first-game"))
        );
    }

    #[test]
    fn a_name_cannot_climb_out_of_the_location_it_was_given() {
        let form = form("../../etc", "/projects");
        assert_eq!(
            form.target(),
            Some(PathBuf::from("/projects/etc")),
            "a name is turned into one directory name, never into a path"
        );
    }
}
