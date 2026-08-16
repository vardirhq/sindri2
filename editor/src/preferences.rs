//! What the editor remembers between launches.
//!
//! The point is not the settings themselves, it is that choosing one is a
//! decision made once rather than every time the editor opens. That also lowers
//! the stakes of every default in here: a default only has to be a reasonable
//! first guess, because disagreeing with it costs one click ever.

use serde::{Deserialize, Serialize};

/// The storage key. Changing it silently discards everyone's settings, so it
/// stays put; a field that changes meaning gets a new name instead.
const KEY: &str = "sindri.editor.preferences";

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
}

/// Editor settings that outlive a session.
///
/// Deliberately small. Anything derived from the scene, the selection, or the
/// current camera is state rather than preference, and restoring it would be
/// restoring a moment rather than a choice.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub layout: Layout,
    pub asset_view: AssetView,
    pub projection: CameraProjection,
    pub bottom_tab: BottomTab,
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

    pub fn save(self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(&self) {
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
            projection: CameraProjection::Orthographic,
            bottom_tab: BottomTab::Console,
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
            projection: CameraProjection::Orthographic,
            bottom_tab: BottomTab::Console,
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
