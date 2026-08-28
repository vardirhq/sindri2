//! The project browser: the folder tree and the assets in a folder.

mod files;
pub(super) mod row;
pub(super) mod state;
mod tools;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout};
use egui_material_icons::MaterialIcon;
use sindri_core::EntityId;

use self::row::{folder_row, listing_row, row_menu};
use self::state::BrowserState;
use self::tools::{back_out, browser_tools, empty_listing};
use crate::{
    preferences::{AssetScope, AssetView, BottomTab, Layout as WorkspaceLayout},
    project::{AssetKind, ProjectTree},
    ui::icons,
    ui::theme::{color, metric},
    ui::widgets::{
        panel,
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
    /// Start renaming a row in place.
    Rename(PathBuf),
    /// Make a folder beside this row, or inside it when it is a folder.
    NewFolder(PathBuf),
    /// Make a `.decay` script there.
    NewScript(PathBuf),
    /// Copy a file or folder beside itself.
    Duplicate(PathBuf),
    /// Ask before removing a file from disk. There is no undo for a disk
    /// write, so this is the one browser action that stops to ask.
    ConfirmDelete(PathBuf),
    /// Copy files from elsewhere into the project.
    Import(PathBuf),
    /// A rename typed into a row was finished.
    CommitRename(PathBuf),
    /// A rename typed into a row was abandoned.
    CancelRename,
    /// Nominate a scene as the one its project opens on.
    SetMainScene(PathBuf),
}

/// The two scenes a row cannot recognise on its own.
///
/// A row knows what file it is. Whether that file is the scene being edited,
/// and whether it is the one its project opens on, are facts about the editor
/// rather than about the directory — and both change what the row says and what
/// its menu offers. Together rather than as two more arguments, because they
/// are the same kind of thing and every caller has both or neither.
#[derive(Clone, Copy, Default)]
pub(crate) struct SceneRoles<'a> {
    /// The scene the editor has open.
    pub(crate) open: Option<&'a Path>,
    /// The scene the open project opens on.
    pub(crate) main: Option<&'a Path>,
    /// Whether a project is open at all.
    ///
    /// Separate from `main`, because a project that nominates nothing yet is
    /// exactly the case where nominating one matters most: gating on `main`
    /// alone would offer it everywhere except where it is needed.
    pub(crate) in_project: bool,
}

impl SceneRoles<'_> {
    /// Whether a path is the scene being edited.
    pub(crate) fn is_open(&self, path: &Path) -> bool {
        self.open == Some(path)
    }

    /// Whether a path is the scene its project opens on.
    pub(crate) fn is_main(&self, path: &Path) -> bool {
        self.main == Some(path)
    }
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

/// What the browser is listing, and how much of the project that is.
///
/// A project is not only its assets: Gather keeps a Cargo manifest, a `src/`,
/// a `tests/`, and a web page beside the `assets/` directory that holds its
/// scene, its art, and its scripts. Only the second of those contains anything
/// a component can name — a texture field resolves `textures/orb.png` against
/// the open scene's own directory, and there is no spelling of `src/main.rs`
/// that a scene can use at all.
///
/// So the browser starts at the assets and says that the rest is there. What
/// the listing is *rooted* at is this; what the user has since pointed it into
/// is `BrowserState`, and that wins.
#[derive(Clone, Copy)]
struct Listing<'a> {
    project: &'a ProjectTree,
    /// Where the listing starts, or `None` for the whole project.
    base: Option<&'a Path>,
}

impl<'a> Listing<'a> {
    fn of(project: &'a ProjectTree, scope: AssetScope) -> Self {
        Self {
            project,
            base: match scope {
                AssetScope::Project => None,
                // The root is not a narrowing, so it is not a base: it would
                // hide the project's own row from the folder tree and answer
                // "back to project" with the listing already showing.
                AssetScope::Assets => project
                    .keeps_more_than_assets()
                    .then(|| project.assets_root())
                    .flatten(),
            },
        }
    }

    /// What going back from a folder goes back to.
    fn label(&self) -> String {
        self.base.map_or_else(
            || self.project.label(),
            |base| {
                base.file_name().map_or_else(
                    || self.project.label(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
        )
    }
}

/// The project browser, in one column or two.
///
/// Two panes need width the bottom dock has and a side column does not: at
/// column width the folder tree and the asset list were drawing over each
/// other. So the narrow arrangement drops the tree rather than shrinking it,
/// which is also why a list reads better there than a grid of identical icons.
#[allow(clippy::too_many_arguments)]
fn project_browser(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    scope: &mut AssetScope,
    browser: &mut BrowserState,
    folders: bool,
    listing: Listing<'_>,
    scenes: SceneRoles<'_>,
    renaming: &mut Option<(PathBuf, String)>,
) -> BrowserAction {
    if !folders {
        return asset_column(
            ui, search, view, scope, browser, false, listing, scenes, renaming,
        );
    }
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(176.0);
            ui.add_space(4.0);
            // The pane navigates: choosing a folder lists that folder. It used
            // to be labels with no sense, so it named the project's folders and
            // did nothing with any of them.
            //
            // Named for the project even when the listing starts below it: the
            // root row is where "back" goes, and "assets" is what the folder is
            // called rather than what the person is working on.
            if folder_row(ui, &listing.project.label(), !browser.is_scoped(), 0).clicked() {
                action = BrowserAction::LookInProject;
            }
            for (folder, depth) in listing.project.folders_in(listing.base) {
                let chosen = browser.folder.as_deref() == Some(folder.path.as_path());
                if folder_row(ui, &folder.name, chosen, depth + 1).clicked() {
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
            let listed = asset_column(
                ui, search, view, scope, browser, true, listing, scenes, renaming,
            );
            if listed != BrowserAction::None {
                action = listed;
            }
        });
    });
    action
}

/// The asset side of the browser: what it is showing, and how.
#[allow(clippy::too_many_arguments)]
fn asset_column(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    scope: &mut AssetScope,
    browser: &mut BrowserState,
    folders: bool,
    listing: Listing<'_>,
    scenes: SceneRoles<'_>,
    renaming: &mut Option<(PathBuf, String)>,
) -> BrowserAction {
    let project = listing.project;
    ui.add_space(4.0);
    // The folder tree already names the directory; without it the list has to,
    // and when the browser is looking inside a folder it says which.
    let label = (!folders).then(|| browser.label_within(project));
    // Offered only where the two listings differ, which is a fact about the
    // project rather than a preference: a project whose scene sits beside its
    // textures has one listing either way.
    let switchable = project.keeps_more_than_assets().then_some(scope);
    let mut action = browser_tools(ui, search, view, switchable, label.as_deref());
    if !folders && browser.is_scoped() && back_out(ui, &listing.label()) {
        action = BrowserAction::LookInProject;
    }
    ui.add_space(5.0);
    if let Some(error) = project.error() {
        panel::problem(ui, error);
        return action;
    }
    let searching = !search.trim().is_empty();
    let matching = project.matching(search);
    let rows = browser.rows(&matching, searching, listing.base);
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
                            if let Some(chosen) = asset_tile(ui, entry, browser, scenes) {
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
                        // The row being renamed gets the draft; every other
                        // row gets its name.
                        let editing = renaming
                            .as_mut()
                            .filter(|(path, _)| path == &entry.path)
                            .map(|(_, name)| name);
                        if let Some(chosen) =
                            listing_row(ui, entry, *depth, searching, scenes, browser, editing)
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
    scenes: SceneRoles<'_>,
) -> Option<BrowserAction> {
    let tile = crate::ui::widgets::asset::tile(
        ui,
        asset_icon(entry.kind),
        &entry.name,
        browser.selected.as_deref() == Some(entry.path.as_path()) || scenes.is_open(&entry.path),
        None,
    );
    let tile = tile.on_hover_text(match entry.kind {
        AssetKind::Scene => "Double-click to open this scene",
        AssetKind::Folder => "Double-click to look inside this folder",
        kind => kind.label(),
    });
    // The same menu the list view offers: which presentation the browser is in
    // is a matter of how files are drawn, not of what can be done with one.
    let asked = row_menu(&tile, entry, scenes);
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

impl EditorApp {
    /// The dock's tabs, and the count that says an error is waiting.
    fn dock_tabs(&mut self, ui: &mut egui::Ui) {
        let counts = self.console.counts();
        let mut chosen = self.preferences.bottom_tab;
        tabs::strip(ui, |ui| {
            for (tab, icon, label) in [
                (BottomTab::Project, icons::PROJECT, "Project"),
                (BottomTab::Console, icons::CONSOLE, "Console"),
                (BottomTab::History, icons::UNDO, "History"),
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
                    crate::ui::widgets::toolbar::chip(ui, &counts.summary(), color::DANGER_TEXT);
                });
            }
        });
        self.preferences.bottom_tab = chosen;
    }

    pub(super) fn asset_panel(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let mut action = BrowserAction::None;
        let mut clear_console = false;
        let mut go_to = None;
        let mut travel = None;
        let (panel_side, default, min, max) = match self.preferences.layout {
            // A tall column, which is what makes the list view worth having.
            WorkspaceLayout::TwoByThree => {
                (egui::Panel::right("project-column"), 280.0, 200.0, 420.0)
            }
            WorkspaceLayout::Wide => (egui::Panel::bottom("project-dock"), 226.0, 140.0, 330.0),
        };
        // The folder tree only fits when the browser is a wide dock.
        let folders = self.preferences.layout == WorkspaceLayout::Wide;
        // Read before the panel borrows the preferences mutably: how much of
        // the project is listed is decided from the tree, and the switch that
        // changes it lives inside.
        let scope = self.preferences.asset_scope;
        panel_side
            .default_size(default)
            .min_size(min)
            .max_size(max)
            .resizable(true)
            .frame(panel::frame())
            .show(ui, |ui| {
                self.dock_tabs(ui);
                match self.preferences.bottom_tab {
                    BottomTab::Project => {
                        action = project_browser(
                            ui,
                            &mut self.asset_search,
                            &mut self.preferences.asset_view,
                            &mut self.preferences.asset_scope,
                            &mut self.browser,
                            folders,
                            Listing::of(&self.project, scope),
                            SceneRoles {
                                open: self.file.path(),
                                main: self.project_main_scene.as_deref(),
                                in_project: self.open_project_root.is_some(),
                            },
                            &mut self.asset_rename,
                        );
                    }
                    BottomTab::Console => {
                        // The world is read for a name rather than handed over:
                        // the console knows which entity a line is about, and
                        // what it is called is the panel's question.
                        let world = &self.world;
                        let named = |entity: EntityId| {
                            world.get(entity).map(super::hierarchy::row::entity_name)
                        };
                        let answered = console_view(
                            ui,
                            &self.console,
                            self.lifecycle.state(),
                            &mut self.preferences.console_filter,
                            &named,
                        );
                        clear_console = answered.cleared;
                        go_to = answered.go_to;
                    }
                    BottomTab::History => {
                        travel = super::history_view::history_panel(ui, &self.history);
                    }
                }
            });
        if clear_console {
            self.console.clear();
        }
        if let Some(entity) = go_to {
            self.select(Some(entity));
        }
        if let Some(travel) = travel {
            self.travel_history(travel);
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
            BrowserAction::Rename(path) => self.begin_asset_rename(&path),
            BrowserAction::CommitRename(path) => {
                if let Some((_, name)) = self.asset_rename.take() {
                    self.rename_asset(&path, &name);
                }
            }
            BrowserAction::CancelRename => self.asset_rename = None,
            BrowserAction::NewFolder(beside) => self.new_folder(&beside),
            BrowserAction::NewScript(beside) => self.new_script(&beside),
            BrowserAction::Duplicate(path) => self.duplicate_asset(&path),
            BrowserAction::ConfirmDelete(path) => self.deleting = Some(path),
            BrowserAction::Import(into) => self.import_assets(&into),
            BrowserAction::SetMainScene(path) => self.set_main_scene(&path),
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
