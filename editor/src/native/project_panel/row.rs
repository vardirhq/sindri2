//! One entry in the project browser, as a row.
//!
//! The row itself is `ui::widgets::asset`, which the hierarchy's tree row
//! shares its banding and indentation with. What is here is what a *project*
//! row is: what the editor can do with each kind of file, and what hangs
//! underneath a folder or a sliced image.

use std::path::Path;

use eframe::egui::{self, Response};

use crate::project::{AssetKind, ProjectEntry};
use crate::ui::widgets::asset;

use super::state::BrowserState;
use super::{BrowserAction, asset_icon};

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
    open: Option<&Path>,
    state: &mut BrowserState,
) -> Option<BrowserAction> {
    if entry.kind == AssetKind::Folder {
        return folder_listing_row(ui, entry, depth, searching, open, state);
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
        open,
        state,
        sliced.then_some(&mut showing),
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
    if row.double_clicked() && entry.kind == AssetKind::Scene {
        return Some(BrowserAction::Open(entry.path.clone()));
    }
    if row.clicked() {
        return Some(BrowserAction::Select(entry.path.clone()));
    }
    None
}

/// A folder in the listing: it folds, and a double click looks inside it.
fn folder_listing_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
    state: &mut BrowserState,
) -> Option<BrowserAction> {
    let mut showing = !state.is_folded(&entry.path);
    let was = showing;
    let row = asset_row(
        ui,
        entry,
        depth,
        searching,
        open,
        state,
        (!searching).then_some(&mut showing),
    );
    if showing != was {
        state.toggle_fold(&entry.path);
    }
    if row.double_clicked() {
        return Some(BrowserAction::LookIn(entry.path.clone()));
    }
    if row.clicked() {
        return Some(BrowserAction::Select(entry.path.clone()));
    }
    None
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
        },
    );
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
    open: Option<&Path>,
    state: &BrowserState,
    // `Some` for a folder or a sliced image, carrying whether it is open.
    expanded: Option<&mut bool>,
) -> Response {
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
            current: open.is_some_and(|path| path == entry.path),
            expanded,
        },
    );
    row.on_hover_text(match entry.kind {
        AssetKind::Scene => "Double-click to open this scene",
        AssetKind::Texture => "Click to slice this image into sprites",
        AssetKind::Folder => "Double-click to look inside this folder",
        _ => entry.kind.label(),
    })
}

/// A folder in the browser's tree pane, which navigates rather than folds.
pub(super) fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) -> Response {
    asset::folder(ui, label, selected, depth)
}
