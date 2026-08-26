//! The project browser: the folder tree and the assets in a folder.

pub(super) mod row;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Align, Layout, RichText, Stroke};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_CODE, ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_FOLDER, ICON_GRID_VIEW, ICON_IMAGE,
        ICON_PLAY_ARROW, ICON_REFRESH, ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};

use self::row::{asset_tile, folder_row, sliceable_row};
use crate::{
    preferences::{AssetView, BottomTab, Layout as WorkspaceLayout},
    project::{AssetKind, ProjectTree},
};

use super::EditorApp;
use super::chrome::bottom_tab;
use super::unsaved::Discarding;
use super::{
    console_view::console_view,
    theme::{BORDER, PANEL_BG, TEXT, TEXT_FAINT, icon_button},
};

/// What the project browser shows, until it reads a real asset directory.
///
/// Each entry carries its kind as well as its name, because a list has room to
/// say what a thing is and a grid of generic icons does not.
/// What a frame of the project browser asked for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum BrowserAction {
    #[default]
    None,
    /// Re-read the directory, because the editor caches it.
    Refresh,
    /// Open a scene the browser is showing.
    Open(PathBuf),
    /// Show an asset in the inspector, which for a texture means its slice.
    Select(PathBuf),
}

/// The icon a kind of file is drawn with.
pub(super) const fn asset_icon(kind: AssetKind) -> MaterialIcon {
    match kind {
        AssetKind::Folder => ICON_FOLDER,
        AssetKind::Scene => ICON_DESCRIPTION,
        AssetKind::Texture | AssetKind::Font => ICON_IMAGE,
        // A sprite and the sheet that cuts it are both about a grid over an
        // image, and neither is the image.
        AssetKind::Sprite | AssetKind::Sheet => ICON_GRID_VIEW,
        AssetKind::Mesh => ICON_VIEW_IN_AR,
        AssetKind::Script => ICON_CODE,
        AssetKind::Audio => ICON_PLAY_ARROW,
        AssetKind::Other => ICON_DEPLOYED_CODE,
    }
}

/// The project browser, in one column or two.
///
/// Two panes need width the bottom dock has and a side column does not: at
/// column width the folder tree and the asset list were drawing over each
/// other. So the narrow arrangement drops the tree rather than shrinking it,
/// which is also why a list reads better there than a grid of identical icons.
fn project_browser(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    expanded: &mut BTreeSet<PathBuf>,
    folders: bool,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    if !folders {
        return asset_column(ui, search, view, expanded, project, open);
    }
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(174.0);
            folder_row(ui, &project.label(), true, 0);
            for folder in project.folders() {
                folder_row(ui, &folder.name, false, folder.depth + 1);
            }
        });
        ui.separator();
        ui.vertical(|ui| action = asset_column(ui, search, view, expanded, project, open));
    });
    action
}

/// The asset side of the browser: what it is showing, and how.
fn asset_column(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    expanded: &mut BTreeSet<PathBuf>,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(project.label()).size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_VIEW_LIST, *view == AssetView::List, "List view").clicked() {
                *view = AssetView::List;
            }
            if icon_button(ui, ICON_GRID_VIEW, *view == AssetView::Grid, "Grid view").clicked() {
                *view = AssetView::Grid;
            }
            // The directory is read when a scene is opened, so a file added
            // outside the editor needs asking for. This slot used to hold a
            // filter icon that did nothing.
            if icon_button(ui, ICON_REFRESH, false, "Re-read the project directory").clicked() {
                action = BrowserAction::Refresh;
            }
            // Whatever is left after the buttons, rather than a fixed width
            // that overflowed the moment the browser became a column.
            let room = (ui.available_width() - 6.0).clamp(60.0, 210.0);
            ui.add_sized(
                [room, 27.0],
                egui::TextEdit::singleline(search).hint_text("Search"),
            );
        });
    });
    ui.add_space(8.0);
    if let Some(error) = project.error() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(error).size(11.0).color(TEXT_FAINT));
        });
        return action;
    }
    let searching = !search.trim().is_empty();
    let entries = project.matching(search);
    if entries.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let message = if searching {
                "Nothing matches"
            } else {
                "This directory is empty"
            };
            ui.label(RichText::new(message).size(11.0).color(TEXT_FAINT));
        });
        return action;
    }
    // A project has more assets than a dock has room for, in either
    // presentation. Scrolling here is what lets the list be the default
    // without the last few assets falling off the bottom.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match view {
                AssetView::Grid => {
                    ui.horizontal_wrapped(|ui| {
                        for entry in &entries {
                            if asset_tile(ui, entry, open).double_clicked() {
                                action = BrowserAction::Open(entry.path.clone());
                            }
                        }
                    });
                }
                AssetView::List => {
                    // Rows are denser than egui's default spacing, so the dock
                    // shows a useful number of them without taking height from
                    // the viewport it sits under.
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for entry in &entries {
                        // A search shows a flat list, so an indentation would
                        // point at a parent the search has removed.
                        let depth = if searching { 0 } else { entry.depth };
                        // A sliced image's parts sit under it, because that is
                        // where a person looks for them: they belong to the
                        // image, not to the directory. Collapsed until asked
                        // for, because a sheet is as likely to hold sixty-four
                        // frames as four, and a browser that cannot be scrolled
                        // past is the failure the hierarchy already taught us.
                        if let Some(chosen) =
                            sliceable_row(ui, entry, depth, searching, open, expanded)
                        {
                            action = chosen;
                        }
                    }
                }
            }
            if project.truncated() {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("More files than the browser reads")
                            .size(10.0)
                            .color(TEXT_FAINT),
                    );
                });
            }
        });
    action
}

impl EditorApp {
    pub(super) fn asset_panel(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let mut action = BrowserAction::None;
        let mut clear_console = false;
        let (panel, default, min, max) = match self.preferences.layout {
            // A tall column, which is what makes the list view worth having.
            WorkspaceLayout::TwoByThree => {
                (egui::Panel::right("project-column"), 280.0, 200.0, 420.0)
            }
            WorkspaceLayout::Wide => (egui::Panel::bottom("project-dock"), 226.0, 140.0, 330.0),
        };
        // The folder tree only fits when the browser is a wide dock.
        let folders = self.preferences.layout == WorkspaceLayout::Wide;
        panel
            .default_size(default)
            .min_size(min)
            .max_size(max)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    bottom_tab(
                        ui,
                        &mut self.preferences.bottom_tab,
                        BottomTab::Project,
                        "Project",
                    );
                    bottom_tab(
                        ui,
                        &mut self.preferences.bottom_tab,
                        BottomTab::Console,
                        "Console",
                    );
                });
                ui.separator();
                match self.preferences.bottom_tab {
                    BottomTab::Project => {
                        action = project_browser(
                            ui,
                            &mut self.asset_search,
                            &mut self.preferences.asset_view,
                            &mut self.expanded_sheets,
                            folders,
                            &self.project,
                            self.file.path(),
                        );
                    }
                    BottomTab::Console => {
                        if console_view(ui, &self.console, self.lifecycle.state()) {
                            clear_console = true;
                        }
                    }
                }
            });
        if clear_console {
            self.console.clear();
        }
        // Acted on outside the panel, because both answers write to the field
        // the browser was reading from.
        match action {
            BrowserAction::None => {}
            BrowserAction::Refresh => self.refresh_project(),
            BrowserAction::Open(path) => {
                self.discard_or_confirm(Discarding::OpenPath(path), &context);
            }
            BrowserAction::Select(path) => self.select_asset(&path),
        }
    }
}
