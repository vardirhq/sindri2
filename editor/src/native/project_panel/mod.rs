//! The project browser: the folder tree and the assets in a folder.

pub(super) mod row;
pub(super) mod state;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout, RichText};
use egui_material_icons::MaterialIcon;

use self::row::{folder_row, listing_row, row_menu};
use self::state::BrowserState;
use crate::{
    preferences::{AssetView, BottomTab, Layout as WorkspaceLayout},
    project::{AssetKind, ProjectTree},
    ui::icons,
    ui::theme::{color, metric, text},
    ui::widgets::{
        button, panel,
        tabs::{self, Weight},
    },
};

use super::EditorApp;
use super::console_view::console_view;
use super::unsaved::Discarding;

/// What a frame of the project browser asked for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum BrowserAction {
    #[default]
    None,
    /// Re-read the directory, because the editor caches it.
    Refresh,
    /// Open a scene the browser is showing.
    Open(PathBuf),
    /// Mark an asset as the browser's selection. A texture also opens the
    /// slicer, which is the one asset the editor can do something with.
    Select(PathBuf),
    /// List one folder rather than the whole project.
    LookIn(PathBuf),
    /// List the whole project again.
    LookInProject,
    /// Put something on the clipboard — a path a component field wants, which
    /// otherwise had to be read off a row and typed back in by hand.
    Copy(String),
}

/// The icon a kind of file is drawn with.
pub(super) const fn asset_icon(kind: AssetKind) -> MaterialIcon {
    match kind {
        AssetKind::Folder => icons::FOLDER,
        AssetKind::Scene => icons::SCENE,
        AssetKind::Texture => icons::SPRITE,
        AssetKind::Font => icons::FONT,
        // A sprite and the sheet that cuts it are both about a grid over an
        // image, and neither is the image.
        AssetKind::Sprite | AssetKind::Sheet => icons::TILEMAP,
        AssetKind::Mesh => icons::MESH,
        AssetKind::Script => icons::SCRIPT,
        AssetKind::Audio => icons::AUDIO,
        AssetKind::Other => icons::FILE,
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
    browser: &mut BrowserState,
    folders: bool,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    if !folders {
        return asset_column(ui, search, view, browser, false, project, open);
    }
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(176.0);
            ui.add_space(4.0);
            // The pane navigates: choosing a folder lists that folder. It used
            // to be labels with no sense, so it named the project's folders and
            // did nothing with any of them.
            if folder_row(ui, &project.label(), !browser.is_scoped(), 0).clicked() {
                action = BrowserAction::LookInProject;
            }
            for folder in project.folders() {
                let chosen = browser.folder.as_deref() == Some(folder.path.as_path());
                if folder_row(ui, &folder.name, chosen, folder.depth + 1).clicked() {
                    action = BrowserAction::LookIn(folder.path.clone());
                }
            }
        });
        // A hairline rather than egui's separator: the dock already has enough
        // horizontal rules without one of them being three shades lighter.
        let (rule, _) =
            ui.allocate_exact_size(egui::vec2(1.0, ui.available_height()), egui::Sense::hover());
        ui.painter().vline(
            rule.center().x,
            rule.y_range(),
            crate::ui::theme::hairline(),
        );
        ui.vertical(|ui| {
            let listed = asset_column(ui, search, view, browser, true, project, open);
            if listed != BrowserAction::None {
                action = listed;
            }
        });
    });
    action
}

/// The browser's own controls: how it presents, and what it is filtered to.
///
/// `label` names the directory being listed, and is `None` when the folder tree
/// beside it already says so — the dock showed "assets" twice, once in the tree
/// and once over the list of the same directory.
///
/// Two rows when there is a label, one when there is not. As a bottom dock
/// there is width for everything side by side; as a tall column there is not,
/// and a right-aligned row that wants more width than it has grows leftwards,
/// which put the asset rows underneath it half outside the panel.
fn browser_tools(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    label: Option<&str>,
) -> BrowserAction {
    let mut action = BrowserAction::None;
    let stacked = label.is_some();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
        if let Some(label) = label {
            ui.label(
                icons::FOLDER
                    .outlined()
                    .rich_text()
                    .size(14.0)
                    .color(color::FORGE_DIM),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(label)
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                )
                .selectable(false)
                .truncate(),
            );
        } else {
            // Room measured from the controls that follow rather than taken
            // from whatever is left, so the search never asks the row for more
            // width than the row has.
            let room = (ui.available_width() - CONTROLS_WIDTH - metric::GUTTER).max(80.0);
            ui.allocate_ui(egui::vec2(room, metric::CONTROL_HEIGHT + 4.0), |ui| {
                panel::search(ui, search, "Search assets");
            });
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(metric::GUTTER);
            // The directory is read when a scene is opened, so a file added
            // outside the editor needs asking for. This slot used to hold a
            // filter icon that did nothing.
            if button::icon(ui, icons::REFRESH, false, "Re-read the project directory").clicked() {
                action = BrowserAction::Refresh;
            }
            let mut presentation = *view;
            if button::Segmented::new(&mut presentation)
                .option(AssetView::List, "List", "One row per file, with its kind")
                .option(AssetView::Grid, "Tiles", "A plate per file, for looking")
                .show(ui)
            {
                *view = presentation;
            }
        });
    });
    if stacked {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER);
            let room = (ui.available_width() - metric::GUTTER).max(60.0);
            ui.allocate_ui(egui::vec2(room, metric::CONTROL_HEIGHT + 4.0), |ui| {
                panel::search(ui, search, "Search assets");
            });
        });
    }
    action
}

/// How much room the view switch and the refresh button take together.
///
/// A measured constant rather than a guess: the search box beside them is
/// allocated the rest, and getting it wrong is how a toolbar overflows its
/// panel.
const CONTROLS_WIDTH: f32 = 152.0;

/// The asset side of the browser: what it is showing, and how.
fn asset_column(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    browser: &mut BrowserState,
    folders: bool,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    ui.add_space(4.0);
    // The folder tree already names the directory; without it the list has to,
    // and when the browser is looking inside a folder it says which.
    let label = (!folders).then(|| browser.label_within(project));
    let mut action = browser_tools(ui, search, view, label.as_deref());
    if !folders && browser.is_scoped() && back_out(ui) {
        action = BrowserAction::LookInProject;
    }
    ui.add_space(5.0);
    if let Some(error) = project.error() {
        panel::problem(ui, error);
        return action;
    }
    let searching = !search.trim().is_empty();
    let matching = project.matching(search);
    let rows = browser.rows(&matching, searching);
    if rows.is_empty() {
        empty_listing(ui, searching, browser.is_scoped());
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
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        for (entry, _) in &rows {
                            if let Some(chosen) = asset_tile(ui, entry, browser, open) {
                                action = chosen;
                            }
                        }
                    });
                }
                AssetView::List => {
                    // Rows are denser than egui's default spacing, so the dock
                    // shows a useful number of them without taking height from
                    // the viewport it sits under.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (entry, depth) in &rows {
                        if let Some(chosen) =
                            listing_row(ui, entry, *depth, searching, open, browser)
                        {
                            action = chosen;
                        }
                    }
                }
            }
            if project.truncated() {
                panel::note(ui, "More files than the browser reads");
            }
        });
    action
}

/// One asset as a tile, reporting what selecting it means.
fn asset_tile(
    ui: &mut egui::Ui,
    entry: &crate::project::ProjectEntry,
    browser: &BrowserState,
    open: Option<&Path>,
) -> Option<BrowserAction> {
    let tile = crate::ui::widgets::asset::tile(
        ui,
        asset_icon(entry.kind),
        &entry.name,
        browser.selected.as_deref() == Some(entry.path.as_path())
            || open.is_some_and(|path| path == entry.path),
        None,
    );
    let tile = tile.on_hover_text(match entry.kind {
        AssetKind::Scene => "Double-click to open this scene",
        AssetKind::Folder => "Double-click to look inside this folder",
        kind => kind.label(),
    });
    // The same menu the list view offers: which presentation the browser is in
    // is a matter of how files are drawn, not of what can be done with one.
    let asked = row_menu(&tile, entry);
    if tile.double_clicked() {
        return match entry.kind {
            AssetKind::Scene => Some(BrowserAction::Open(entry.path.clone())),
            AssetKind::Folder => Some(BrowserAction::LookIn(entry.path.clone())),
            _ => asked,
        };
    }
    if tile.clicked() {
        return Some(BrowserAction::Select(entry.path.clone()));
    }
    asked
}

/// The way back out of a folder, for the arrangement that has no folder tree.
fn back_out(ui: &mut egui::Ui) -> bool {
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        button::labelled(
            ui,
            "Back to project",
            button::Intent::Quiet,
            "List every asset again",
        )
        .clicked()
    })
    .inner
}

/// What the browser says when the listing is empty, which depends on why.
fn empty_listing(ui: &mut egui::Ui, searching: bool, scoped: bool) {
    if searching {
        panel::empty_state(
            ui,
            icons::SEARCH,
            "Nothing matches",
            "No file beside this scene has that in its name.",
        );
    } else if scoped {
        panel::empty_state(
            ui,
            icons::FOLDER,
            "This folder is empty",
            "Nothing is in it yet. Go back to the project to see the rest.",
        );
    } else {
        panel::empty_state(
            ui,
            icons::PROJECT,
            "This directory is empty",
            "Assets placed beside the scene file show up here.",
        );
    }
}

impl EditorApp {
    pub(super) fn asset_panel(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let mut action = BrowserAction::None;
        let mut clear_console = false;
        let (panel_side, default, min, max) = match self.preferences.layout {
            // A tall column, which is what makes the list view worth having.
            WorkspaceLayout::TwoByThree => {
                (egui::Panel::right("project-column"), 280.0, 200.0, 420.0)
            }
            WorkspaceLayout::Wide => (egui::Panel::bottom("project-dock"), 226.0, 140.0, 330.0),
        };
        // The folder tree only fits when the browser is a wide dock.
        let folders = self.preferences.layout == WorkspaceLayout::Wide;
        panel_side
            .default_size(default)
            .min_size(min)
            .max_size(max)
            .resizable(true)
            .frame(panel::frame())
            .show(ui, |ui| {
                let counts = self.console.counts();
                let mut chosen = self.preferences.bottom_tab;
                tabs::strip(ui, |ui| {
                    for (tab, icon, label) in [
                        (BottomTab::Project, icons::PROJECT, "Project"),
                        (BottomTab::Console, icons::CONSOLE, "Console"),
                    ] {
                        if tabs::tab(
                            ui,
                            Weight::Secondary,
                            self.preferences.bottom_tab == tab,
                            Some(icon),
                            label,
                        )
                        .clicked()
                        {
                            chosen = tab;
                        }
                    }
                    // An error nobody is looking at is the reason the console
                    // exists, so the tab strip says there is one whichever tab
                    // is showing.
                    if counts.errors > 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(metric::GUTTER);
                            crate::ui::widgets::toolbar::chip(
                                ui,
                                &counts.summary(),
                                color::DANGER_TEXT,
                            );
                        });
                    }
                });
                self.preferences.bottom_tab = chosen;
                match self.preferences.bottom_tab {
                    BottomTab::Project => {
                        action = project_browser(
                            ui,
                            &mut self.asset_search,
                            &mut self.preferences.asset_view,
                            &mut self.browser,
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
        // Acted on outside the panel, because every answer writes to state the
        // browser was reading from.
        match action {
            BrowserAction::None => {}
            BrowserAction::Refresh => self.refresh_project(),
            BrowserAction::Open(path) => {
                self.discard_or_confirm(Discarding::OpenPath(path), &context);
            }
            BrowserAction::Select(path) => self.select_asset(&path),
            BrowserAction::LookIn(folder) => self.browser.look_in(Some(&folder)),
            BrowserAction::LookInProject => self.browser.look_in(None),
            BrowserAction::Copy(text) => {
                // Said in the console rather than through `report`, which is
                // for things that did not happen: a copy that worked is not a
                // failure, and marking it one puts a red count on the tab.
                context.copy_text(text.clone());
                self.console.info(format!("Copied {text}"));
            }
        }
    }
}
