//! What the editor should open when it starts.
//!
//! Three answers, in order of how deliberately they were asked for. That
//! ordering is the same one `scene_io` already applies to scenes, and it is
//! extended rather than replaced: something named on the command line is the
//! most deliberate thing anyone can say, so it wins and no window asks about it.
//! Otherwise the project someone chose to reopen every launch. Otherwise the
//! welcome window, which asks.
//!
//! Deciding this here rather than inside the window means the rule can be tested
//! without opening one, which matters because getting it wrong is how an editor
//! either stops honouring its own command line or grows a window in front of
//! every launch that nobody asked for.

use std::path::{Path, PathBuf};

use super::manifest;
use super::recent::RecentProject;

/// What the editor opens on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Launch {
    /// A scene file, opened as its own project if it turns out to be in one.
    Scene(PathBuf),
    /// A project root.
    Project(PathBuf),
    /// Nothing yet: ask.
    Welcome,
}

/// Decides what a launch means.
///
/// `remembered` is the most recently opened project, and `open_last` is whether
/// the user asked not to be shown the welcome window. A remembered project that
/// has moved or been deleted falls back to asking rather than failing: that
/// choice was made last week, its absence is not the user's doing now, and an
/// editor that opens on an error message where a project used to be has stranded
/// them somewhere they cannot work.
pub fn decide(
    argument: Option<&str>,
    remembered: Option<&RecentProject>,
    open_last: bool,
) -> Launch {
    if let Some(argument) = argument {
        let path = Path::new(argument);
        // A directory on the command line is a project when it holds a
        // manifest. Anything else is taken at its word and opened as a scene,
        // including a path to nothing — a named file that is missing is a
        // failure the editor reports, not a reason to open something else.
        if manifest::is_project(path) {
            return Launch::Project(path.to_path_buf());
        }
        return Launch::Scene(path.to_path_buf());
    }
    if open_last
        && let Some(remembered) = remembered
        && remembered.is_present()
    {
        return Launch::Project(PathBuf::from(&remembered.path));
    }
    Launch::Welcome
}

#[cfg(test)]
mod tests {
    use sindri_core::SceneDocument;

    use super::super::manifest::Project;
    use super::*;

    fn remembered(root: &Path) -> RecentProject {
        RecentProject {
            path: root.display().to_string(),
            name: "Remembered".to_owned(),
        }
    }

    #[test]
    fn nothing_asked_for_opens_the_welcome_window() {
        assert_eq!(decide(None, None, false), Launch::Welcome);
        assert_eq!(
            decide(None, None, true),
            Launch::Welcome,
            "there is nothing to open last"
        );
    }

    #[test]
    fn a_scene_on_the_command_line_opens_without_asking() {
        assert_eq!(
            decide(Some("levels/one.scene.json"), None, false),
            Launch::Scene(PathBuf::from("levels/one.scene.json")),
            "the command line is the most deliberate thing anyone can say"
        );
    }

    #[test]
    fn a_project_directory_on_the_command_line_opens_as_a_project() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join("game");
        Project::create(&root, "Game", &SceneDocument::default()).expect("a project");
        assert_eq!(
            decide(Some(&root.display().to_string()), None, false),
            Launch::Project(root)
        );
    }

    #[test]
    fn a_directory_that_is_not_a_project_is_still_taken_at_its_word() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().display().to_string();
        assert_eq!(
            decide(Some(&path), None, false),
            Launch::Scene(PathBuf::from(&path)),
            "opening it will fail and say so, which is the honest answer to a \
             path someone named"
        );
    }

    #[test]
    fn the_command_line_wins_over_the_remembered_project() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join("game");
        Project::create(&root, "Game", &SceneDocument::default()).expect("a project");
        assert_eq!(
            decide(
                Some("levels/one.scene.json"),
                Some(&remembered(&root)),
                true
            ),
            Launch::Scene(PathBuf::from("levels/one.scene.json"))
        );
    }

    #[test]
    fn the_remembered_project_opens_only_when_that_was_asked_for() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join("game");
        Project::create(&root, "Game", &SceneDocument::default()).expect("a project");
        let remembered = remembered(&root);

        assert_eq!(
            decide(None, Some(&remembered), false),
            Launch::Welcome,
            "the welcome window is the front door until someone shuts it"
        );
        assert_eq!(decide(None, Some(&remembered), true), Launch::Project(root));
    }

    #[test]
    fn a_remembered_project_that_is_gone_asks_rather_than_failing() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join("gone");
        assert_eq!(
            decide(None, Some(&remembered(&root)), true),
            Launch::Welcome
        );
    }
}
