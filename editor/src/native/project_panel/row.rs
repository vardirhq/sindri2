//! One entry in the project browser, as a row.
//!
//! The row itself is `ui::widgets::asset`, which the hierarchy's tree row
//! shares its banding and indentation with. What is here is what a *project*
//! row is: what the editor can do with each kind of file, and what hangs
//! underneath a folder or a sliced image.

use eframe::egui::{self, Response};

use crate::project::{AssetKind, ProjectEntry};
use crate::ui::widgets::asset::AssetRow;
use crate::ui::widgets::{asset, menu};

use super::state::BrowserState;
use super::{BrowserAction, SceneRoles, asset_icon};

/// One row of the listing, plus whatever hangs under it.
///
/// A folder's contents and a sliced image's sprites are both "what is under
/// this row", so both fold from the same chevron and both are drawn here — the
/// listing itself is flat, and this is where a row and its children read as one
/// thing.
pub(crate) fn listing_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    scenes: SceneRoles<'_>,
    state: &mut BrowserState,
    editing: Option<&mut String>,
) -> Option<BrowserAction> {
    if entry.kind == AssetKind::Folder {
        return folder_listing_row(ui, entry, depth, searching, scenes, state, editing);
    }
    // A search shows a flat list, so parts under an image would be pointing at
    // a parent the search may have removed.
    let sliced = !entry.sprites.is_empty() && !searching;
    let mut showing = state.expanded_sheets.contains(&entry.path);
    let row = asset_row(
        ui,
        entry,
        depth,
        searching,
        scenes,
        state,
        RowEdit {
            expanded: sliced.then_some(&mut showing),
            editing,
        },
    );
    if sliced {
        if showing {
            state.expanded_sheets.insert(entry.path.clone());
            for sprite in &entry.sprites {
                sprite_row(ui, sprite, depth + 1);
            }
        } else {
            state.expanded_sheets.remove(&entry.path);
        }
    }
    if let Some(action) = renamed(&row, entry) {
        return Some(action);
    }
    let mut asked = row_menu(&row.response, entry, scenes);
    if row.response.double_clicked() && entry.kind == AssetKind::Scene {
        asked = Some(BrowserAction::Open(entry.path.clone()));
    } else if row.response.clicked() {
        asked = Some(BrowserAction::Select(entry.path.clone()));
    }
    asked
}

/// What a finished rename asked for, or `None` while it is still being typed.
fn renamed(row: &AssetRow, entry: &ProjectEntry) -> Option<BrowserAction> {
    Some(if row.renamed? {
        BrowserAction::CommitRename(entry.path.clone())
    } else {
        BrowserAction::CancelRename
    })
}

/// A folder in the listing: it folds, and a double click looks inside it.
fn folder_listing_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    scenes: SceneRoles<'_>,
    state: &mut BrowserState,
    editing: Option<&mut String>,
) -> Option<BrowserAction> {
    let mut showing = !state.is_folded(&entry.path);
    let was = showing;
    let row = asset_row(
        ui,
        entry,
        depth,
        searching,
        scenes,
        state,
        RowEdit {
            expanded: (!searching).then_some(&mut showing),
            editing,
        },
    );
    if showing != was {
        state.toggle_fold(&entry.path);
    }
    if let Some(action) = renamed(&row, entry) {
        return Some(action);
    }
    let mut asked = row_menu(&row.response, entry, scenes);
    if row.response.double_clicked() {
        asked = Some(BrowserAction::LookIn(entry.path.clone()));
    } else if row.response.clicked() {
        asked = Some(BrowserAction::Select(entry.path.clone()));
    }
    asked
}

/// One named part of a sliced image, under the image it came from.
///
/// Not a `ProjectEntry`: a sprite has no file, and giving it one would put it in
/// the directory listing as something that could be opened, renamed, or deleted
/// on its own. It is a row and nothing more.
fn sprite_row(ui: &mut egui::Ui, sprite: &str, depth: usize) {
    asset::row(
        ui,
        asset::Entry {
            icon: asset_icon(AssetKind::Sprite),
            name: sprite,
            kind: AssetKind::Sprite.label(),
            depth,
            selected: false,
            current: false,
            expanded: None,
            editing: None,
        },
    );
}

/// What a row is doing beyond listing a file.
///
/// Together rather than as two more arguments, because they are the same kind
/// of thing — the state a row carries while it is being interacted with — and
/// a row that is neither folded nor being renamed passes `RowEdit::none()`.
pub(crate) struct RowEdit<'a> {
    /// `Some` for a folder or a sliced image, carrying whether it is open.
    pub(crate) expanded: Option<&'a mut bool>,
    /// `Some` while this row is being renamed in place.
    pub(crate) editing: Option<&'a mut String>,
}

/// One asset as a row: what it is called, what it is, whether the browser has
/// it selected, and whether it is the scene the editor has open.
///
/// Every row answers a click, because a row that cannot be selected cannot
/// carry a right-click menu either. What each kind can *do* is said on hover:
/// a scene opens, an image slices, and everything else is a listing that can
/// still be pointed at.
pub(crate) fn asset_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    scenes: SceneRoles<'_>,
    state: &BrowserState,
    edit: RowEdit<'_>,
) -> AssetRow {
    let RowEdit { expanded, editing } = edit;
    // Under a search the path below the root is what tells two files of the
    // same name apart.
    let name = if searching {
        &entry.relative
    } else {
        &entry.name
    };
    let row = asset::row(
        ui,
        asset::Entry {
            icon: asset_icon(entry.kind),
            name,
            kind: entry.kind.label(),
            depth,
            selected: state.selected.as_deref() == Some(entry.path.as_path()),
            current: scenes.is_open(&entry.path),
            expanded,
            editing,
        },
    );
    AssetRow {
        response: row.response.on_hover_text(match entry.kind {
            AssetKind::Scene if scenes.is_main(&entry.path) => {
                "This is what the project opens. Double-click to open it now."
            }
            AssetKind::Scene => "Double-click to open this scene",
            AssetKind::Texture => "Click to slice this image into sprites",
            AssetKind::Folder => "Double-click to look inside this folder",
            AssetKind::Script | AssetKind::Sheet => "Click to read this file",
            AssetKind::Audio => "Click to hear this clip",
            AssetKind::Font => "Click to see this typeface",
            _ => entry.kind.label(),
        }),
        renamed: row.renamed,
    }
}

/// The menu a right-click opens on one asset.
///
/// Only what the editor can actually carry out: the verb the row's double
/// click already performs, said in words rather than left to be guessed, and
/// the two paths worth copying. A component that names an asset names it by
/// the path the scene resolves against, which until now had to be typed from
/// reading the row.
pub(crate) fn row_menu(
    response: &Response,
    entry: &ProjectEntry,
    scenes: SceneRoles<'_>,
) -> Option<BrowserAction> {
    let mut asked = None;
    menu::on_right_click(response, |ui| {
        menu::subject(ui, &entry.name);
        let opens = match entry.kind {
            AssetKind::Scene => Some(("Open scene", BrowserAction::Open(entry.path.clone()))),
            AssetKind::Folder => Some(("Look inside", BrowserAction::LookIn(entry.path.clone()))),
            AssetKind::Texture => Some((
                "Slice into sprites",
                BrowserAction::Select(entry.path.clone()),
            )),
            _ => None,
        };
        if let Some((label, action)) = opens
            && menu::item(ui, label).clicked()
        {
            asked = Some(action);
            ui.close();
        }
        // Offered on a scene that is not already the one its project opens on.
        // The scene that *is* says so on hover instead: an entry that sets what
        // is already set is an entry that does nothing when it is pressed.
        if entry.kind == AssetKind::Scene
            && scenes.in_project
            && !scenes.is_main(&entry.path)
            && menu::item(ui, "Set as main scene").clicked()
        {
            asked = Some(BrowserAction::SetMainScene(entry.path.clone()));
            ui.close();
        }
        ui.separator();
        // The file operations. None of these go through the undo history —
        // they are disk writes, and the history describes a world rather than
        // a directory — so the destructive one asks first and the rest refuse
        // rather than overwrite.
        if menu::item_with_key(ui, "Rename", "F2").clicked() {
            asked = Some(BrowserAction::Rename(entry.path.clone()));
            ui.close();
        }
        if menu::item(ui, "Duplicate").clicked() {
            asked = Some(BrowserAction::Duplicate(entry.path.clone()));
            ui.close();
        }
        if menu::item(ui, "New folder here").clicked() {
            asked = Some(BrowserAction::NewFolder(entry.path.clone()));
            ui.close();
        }
        if menu::item(ui, "New script here").clicked() {
            asked = Some(BrowserAction::NewScript(entry.path.clone()));
            ui.close();
        }
        if menu::item(ui, "Import files…").clicked() {
            asked = Some(BrowserAction::Import(entry.path.clone()));
            ui.close();
        }
        ui.separator();
        // The path a component field wants is the one the open scene resolves
        // against, which is not the path from the project root whenever a
        // project keeps its scene under `assets/`. A file with no such path —
        // a folder, or anything the loader cannot reach — is offered only its
        // own, because copying a reference that will not load is worse than
        // copying nothing.
        if let Some(reference) = &entry.reference
            && menu::item(ui, "Copy asset path").clicked()
        {
            asked = Some(BrowserAction::Copy(reference.clone()));
            ui.close();
        }
        if menu::item(ui, "Copy full path").clicked() {
            asked = Some(BrowserAction::Copy(
                entry.path.to_string_lossy().into_owned(),
            ));
            ui.close();
        }
        ui.separator();
        if menu::danger(ui, "Delete", "Del").clicked() {
            asked = Some(BrowserAction::ConfirmDelete(entry.path.clone()));
            ui.close();
        }
    });
    asked
}

/// A folder in the browser's tree pane, which navigates rather than folds.
pub(super) fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) -> Response {
    asset::folder(ui, label, selected, depth)
}
