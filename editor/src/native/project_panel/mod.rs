//! The project browser: the folder tree and the assets in a folder.

pub(super) mod row;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Align, Layout, RichText};
use egui_material_icons::MaterialIcon;

use self::row::{folder_row, sliceable_row};
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
    /// Show an asset in the inspector, which for a texture means its slice.
    Select(PathBuf),
}

/// The icon a kind of file is drawn with.
pub(super) const fn asset_icon(kind: AssetKind) -> MaterialIcon {
    match kind {
        AssetKind::Folder => icons::FOLDER,
        AssetKind::Scene => icons::SCENE,
        AssetKind::Texture | AssetKind::Font => icons::SPRITE,
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
            ui.set_width(176.0);
            ui.add_space(4.0);
            folder_row(ui, &project.label(), true, 0);
            for folder in project.folders() {
                folder_row(ui, &folder.name, false, folder.depth + 1);
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
        ui.vertical(|ui| action = asset_column(ui, search, view, expanded, project, open));
    });
    action
}

/// The browser's own controls: where it is looking, how it presents, and what
/// it is filtered to.
///
/// Two rows rather than one. As a bottom dock there is width for everything
/// side by side; as a tall column there is not, and a right-aligned row that
/// wants more width than it has grows leftwards — which put the asset rows
/// underneath it half outside the panel.
fn browser_tools(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    label: &str,
) -> BrowserAction {
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.add_space(metric::GUTTER);
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
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        let room = (ui.available_width() - metric::GUTTER).max(60.0);
        ui.allocate_ui(egui::vec2(room, metric::CONTROL_HEIGHT + 4.0), |ui| {
            panel::search(ui, search, "Search assets");
        });
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
    ui.add_space(4.0);
    let mut action = browser_tools(ui, search, view, &project.label());
    ui.add_space(5.0);
    if let Some(error) = project.error() {
        panel::problem(ui, error);
        return action;
    }
    let searching = !search.trim().is_empty();
    let entries = project.matching(search);
    if entries.is_empty() {
        if searching {
            panel::empty_state(
                ui,
                icons::SEARCH,
                "Nothing matches",
                "No file beside this scene has that in its name.",
            );
        } else {
            panel::empty_state(
                ui,
                icons::PROJECT,
                "This directory is empty",
                "Assets placed beside the scene file show up here.",
            );
        }
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
                        for entry in &entries {
                            let tile = crate::ui::widgets::asset::tile(
                                ui,
                                asset_icon(entry.kind),
                                &entry.name,
                                open.is_some_and(|path| path == entry.path),
                                None,
                            );
                            let tile = if entry.kind == AssetKind::Scene {
                                tile.on_hover_text("Double-click to open")
                            } else {
                                tile.on_hover_text(entry.kind.label())
                            };
                            if tile.double_clicked() {
                                action = BrowserAction::Open(entry.path.clone());
                            }
                        }
                    });
                }
                AssetView::List => {
                    // Rows are denser than egui's default spacing, so the dock
                    // shows a useful number of them without taking height from
                    // the viewport it sits under.
                    ui.spacing_mut().item_spacing.y = 0.0;
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
                panel::note(ui, "More files than the browser reads");
            }
        });
    action
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
