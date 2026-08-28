//! What the editor remembers between launches.
//!
//! The point is not the settings themselves, it is that choosing one is a
//! decision made once rather than every time the editor opens. That also lowers
//! the stakes of every default in here: a default only has to be a reasonable
//! first guess, because disagreeing with it costs one click ever.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The storage key. Changing it silently discards everyone's settings, so it
/// stays put; a field that changes meaning gets a new name instead.
const KEY: &str = "sindri.editor.preferences";

/// How much of the console is shown.
///
/// Remembered, because it is a reading preference rather than a state: someone
/// watching for a failure wants the console filtered to failures for as long as
/// they are watching, not for one frame.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleFilter {
    /// Everything the editor said.
    #[default]
    All,
    /// Only what went wrong, and what might have.
    Problems,
    /// Only what did not happen.
    Errors,
}

impl ConsoleFilter {
    /// The lowest level this shows.
    pub const fn floor(self) -> crate::console::Level {
        match self {
            Self::All => crate::console::Level::Info,
            Self::Problems => crate::console::Level::Warning,
            Self::Errors => crate::console::Level::Error,
        }
    }
}

/// How the project browser presents assets.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetView {
    /// Rows carrying a name and a kind.
    ///
    /// The default, because the grid's tiles show a generic icon per file type
    /// rather than a picture of the asset. Until a thumbnail is a thumbnail,
    /// the grid spends more space to say less.
    #[default]
    List,
    /// Tiles, for when there is something to look at.
    Grid,
}

/// How much of the project the browser lists.
///
/// A project is not only its assets. Gather keeps a Cargo manifest, a `src/`,
/// a `tests/`, and a web page beside the `assets/` directory its scene and art
/// live in, and none of those is a file a component can name — listing them
/// beside the textures makes the browser a directory listing again, which is
/// what it exists not to be.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetScope {
    /// Only the directory asset references resolve against.
    ///
    /// The default, because it is the only directory whose paths mean anything
    /// in an inspector field: everything in it can be named by a scene, and
    /// everything outside it cannot.
    #[default]
    Assets,
    /// Every file in the project, including the ones the editor never loads.
    ///
    /// Offered rather than assumed. Hiding a project's own source from the
    /// person editing it would be the browser deciding what their project
    /// contains, so the rest of it is one control away and says so.
    Project,
}

/// Which projection the scene viewport uses.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraProjection {
    #[default]
    Perspective,
    Orthographic,
}

/// How the workspace arranges its panels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// Scene above Game on the left, then Hierarchy, Project, and Inspector as
    /// columns beside them.
    ///
    /// The default because it shows the scene and what the player would see at
    /// the same time, which is the comparison an editor exists to make, and
    /// because Project as a tall column is where a list of assets reads better
    /// than a grid of identical icons.
    #[default]
    TwoByThree,
    /// One view at a time with Project docked along the bottom.
    ///
    /// Keeps the whole width for the viewport, which suits a narrow screen or
    /// working on one view without the other competing for attention.
    Wide,
}

impl Layout {
    pub const ALL: [Self; 2] = [Self::TwoByThree, Self::Wide];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TwoByThree => "2 by 3",
            Self::Wide => "Wide",
        }
    }
}

/// Which dock at the bottom of the workspace is showing.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BottomTab {
    #[default]
    Project,
    Console,
    History,
}

/// Editor settings that outlive a session.
///
/// Deliberately small. Anything derived from the selection or the current
/// camera is state rather than preference, and restoring it would be restoring
/// a moment rather than a choice.
///
/// The open scene is the one thing here that looks like state and is not. It is
/// not where the camera happened to be pointing when the editor closed; it is
/// which project someone is working on, and answering that question again by
/// hand every launch is the thing this whole module exists to stop.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub layout: Layout,
    pub asset_view: AssetView,
    /// How much of the project the browser lists.
    ///
    /// A reading preference like the console filter rather than a state:
    /// someone who wants to see their `src/` beside their textures wants it for
    /// as long as they are working that way, not for one frame.
    pub asset_scope: AssetScope,
    pub console_filter: ConsoleFilter,
    pub snapping: crate::gizmo::Snapping,
    pub projection: CameraProjection,
    pub bottom_tab: BottomTab,
    /// The scene file the editor last had open, reopened on the next launch.
    ///
    /// A path rather than anything richer, and one the editor is free to fail
    /// to open: a project can move or be deleted between launches, and an
    /// editor that refuses to start because a remembered file is gone is worse
    /// than one that opens on the default and says why.
    pub last_scene: Option<String>,
    /// Scene-qualified stable IDs of hierarchy rows the user folded closed.
    ///
    /// Open is the default, so an older preferences file and a newly created
    /// `GameObject` both reveal their children. Keeping this outside the scene
    /// means navigating the hierarchy never makes authored content unsaved.
    pub collapsed_hierarchy: BTreeSet<String>,
    /// The projects the welcome window offers, most recently opened first.
    pub recent_projects: crate::project::RecentProjects,
    /// Whether a launch reopens the last project instead of asking.
    ///
    /// False by default, which is the welcome window being the front door. That
    /// is a real cost — one extra click for someone who works in one project all
    /// day — and it is the reason this setting exists rather than the reason to
    /// default the other way: a tool that decides for you which project you
    /// meant is only right until the day it is not, and the welcome window is
    /// also the only way to reach a project you have not opened recently.
    pub open_last_project: bool,
}

impl Preferences {
    /// Reads settings back, falling back to the defaults.
    ///
    /// Unreadable or outdated stored settings are replaced rather than
    /// reported: an editor that refused to open because it could not parse a
    /// window preference would be worse than one that opens with defaults.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|storage| storage.get_string(KEY))
            .and_then(|stored| serde_json::from_str(&stored).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(self) {
            storage.set_string(KEY, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_browser_opens_as_a_list() {
        // Grid tiles show a generic icon per file type rather than the asset,
        // so until thumbnails exist the list says strictly more.
        assert_eq!(Preferences::default().asset_view, AssetView::List);
    }

    /// The layout question this settled: 2 by 3 is what the editor opens as.
    #[test]
    fn the_workspace_opens_in_the_two_by_three_layout() {
        assert_eq!(Preferences::default().layout, Layout::TwoByThree);
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let chosen = Preferences {
            layout: Layout::Wide,
            asset_view: AssetView::Grid,
            asset_scope: AssetScope::Project,
            console_filter: ConsoleFilter::Errors,
            snapping: crate::gizmo::Snapping {
                enabled: true,
                translation: 0.25,
                rotation_degrees: 45.0,
                scale: 0.5,
            },
            projection: CameraProjection::Orthographic,
            bottom_tab: BottomTab::Console,
            last_scene: Some("projects/level.scene.json".to_owned()),
            collapsed_hierarchy: BTreeSet::from(["projects/level.scene.json::player".to_owned()]),
            recent_projects: crate::project::RecentProjects::default(),
            open_last_project: true,
        };
        let text = serde_json::to_string(&chosen).unwrap();
        assert_eq!(serde_json::from_str::<Preferences>(&text).unwrap(), chosen);
    }

    /// Settings written by an older editor must still load, or upgrading would
    /// silently reset everyone.
    #[test]
    fn settings_missing_a_field_keep_their_default() {
        let partial: Preferences = serde_json::from_str(r#"{"asset_view":"grid"}"#).unwrap();
        assert_eq!(partial.asset_view, AssetView::Grid);
        assert_eq!(partial.projection, CameraProjection::Perspective);
        assert!(partial.collapsed_hierarchy.is_empty());
        assert_eq!(
            partial.last_scene, None,
            "settings from before the editor remembered a scene open the default one"
        );
        assert!(
            partial.recent_projects.is_empty(),
            "settings from before projects existed list none"
        );
        assert!(
            !partial.open_last_project,
            "and they open the welcome window rather than a project they never recorded"
        );
    }

    /// Somewhere to write to, so the round trip through storage is checked
    /// here rather than by opening the editor and waiting for an auto-save.
    #[derive(Default)]
    struct FakeStorage(std::collections::BTreeMap<String, String>);

    impl eframe::Storage for FakeStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn what_is_saved_is_what_comes_back() {
        let mut storage = FakeStorage::default();
        let chosen = Preferences {
            layout: Layout::Wide,
            asset_view: AssetView::Grid,
            asset_scope: AssetScope::Project,
            console_filter: ConsoleFilter::Errors,
            snapping: crate::gizmo::Snapping {
                enabled: true,
                translation: 0.25,
                rotation_degrees: 45.0,
                scale: 0.5,
            },
            projection: CameraProjection::Orthographic,
            bottom_tab: BottomTab::Console,
            last_scene: Some("projects/level.scene.json".to_owned()),
            collapsed_hierarchy: BTreeSet::new(),
            recent_projects: crate::project::RecentProjects::default(),
            open_last_project: true,
        };

        chosen.save(&mut storage);
        assert_eq!(Preferences::load(Some(&storage)), chosen);
    }

    #[test]
    fn storage_holding_nothing_yet_gives_the_defaults() {
        let storage = FakeStorage::default();
        assert_eq!(
            Preferences::load(Some(&storage)),
            Preferences::default(),
            "a first launch has nothing stored and must still open"
        );
    }

    #[test]
    fn unreadable_settings_fall_back_rather_than_failing() {
        assert_eq!(
            serde_json::from_str::<Preferences>("not json").ok(),
            None,
            "a parse failure is what `load` turns into the defaults"
        );
        assert_eq!(Preferences::load(None), Preferences::default());
    }
}
