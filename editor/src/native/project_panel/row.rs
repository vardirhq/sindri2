//! One entry in the project browser, as a row.
//!
//! The row itself is `ui::widgets::asset`, which the hierarchy's tree row
//! shares its banding and indentation with. What is here is what a *project*
//! row is: which files the editor can do something with, and what hangs
//! underneath a sliced image.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Response};

use crate::project::{AssetKind, ProjectEntry};
use crate::ui::widgets::asset;

use super::super::project_panel::{BrowserAction, asset_icon};

/// One asset row, plus the sprites under it when its image is sliced and
/// showing.
///
/// Its own function because the row and its children are one thing to a reader
/// — an image and its parts — even though the browser draws them as sibling
/// rows.
pub(super) fn sliceable_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
    expanded: &mut BTreeSet<PathBuf>,
) -> Option<BrowserAction> {
    // A search shows a flat list, so parts under an image would be pointing at
    // a parent the search may have removed.
    let sliced = !entry.sprites.is_empty() && !searching;
    let mut showing = expanded.contains(&entry.path);
    let row = asset_row(
        ui,
        entry,
        depth,
        searching,
        open,
        sliced.then_some(&mut showing),
    );
    if sliced {
        if showing {
            expanded.insert(entry.path.clone());
            for sprite in &entry.sprites {
                sprite_row(ui, sprite, depth + 1);
            }
        } else {
            expanded.remove(&entry.path);
        }
    }
    if row.double_clicked() && entry.kind == AssetKind::Scene {
        return Some(BrowserAction::Open(entry.path.clone()));
    }
    if row.clicked() && entry.kind == AssetKind::Texture {
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
            current: false,
            actionable: false,
            expanded: None,
        },
    );
}

/// One asset as a row: what it is called, what it is, and whether it is the
/// scene the editor has open.
///
/// A scene row answers a double click, because opening one is the only thing
/// the editor can do with a file. A texture row answers a click, because
/// selecting it opens the slicer. Every other row is a listing and says so by
/// not responding — a listing that lists is not the same as a control that
/// looks like it does something.
pub(crate) fn asset_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
    // `Some` for a sliced image, carrying whether its parts are showing. A row
    // with nothing under it gets no triangle rather than a disabled one.
    expanded: Option<&mut bool>,
) -> Response {
    let actionable = matches!(entry.kind, AssetKind::Scene | AssetKind::Texture);
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
            current: open.is_some_and(|path| path == entry.path),
            actionable,
            expanded,
        },
    );
    match entry.kind {
        AssetKind::Scene => row.on_hover_text("Double-click to open this scene"),
        AssetKind::Texture => row.on_hover_text("Click to slice this image into sprites"),
        _ => row,
    }
}

/// A folder in the browser's tree.
pub(super) fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) {
    asset::folder(ui, label, selected, depth);
}
