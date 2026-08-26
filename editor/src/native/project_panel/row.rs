//! One entry in the project browser, as a row or as a tile.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use eframe::egui::{self, Align, Layout, Response, RichText, Sense, Stroke};
use egui_material_icons::icons::{
    ICON_FOLDER, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
};

use crate::project::{AssetKind, ProjectEntry};

use super::super::hierarchy::row::hierarchy_indent;
use super::super::project_panel::{BrowserAction, asset_icon};
use super::super::theme::{ACCENT, BORDER_SUBTLE, PANEL_RAISED, TEXT, TEXT_FAINT, TEXT_MUTED};

/// One asset as a row: what it is called, what it is, and whether it is the
/// scene the editor has open.
///
/// A scene row answers a double click, because opening one is the only thing
/// the editor can do with a file. Every other row is a listing and says so by
/// not responding — a listing that lists is not the same as a control that
/// looks like it does something.
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
    ui.horizontal(|ui| {
        ui.add_space(8.0 + hierarchy_indent(depth, 12.0));
        ui.label(
            asset_icon(AssetKind::Sprite)
                .outlined()
                .rich_text()
                .size(13.0)
                .color(TEXT_MUTED),
        );
        ui.label(RichText::new(sprite).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            ui.label(
                RichText::new(AssetKind::Sprite.label())
                    .size(9.0)
                    .color(TEXT_MUTED),
            );
        });
    });
}

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
    let openable = matches!(entry.kind, AssetKind::Scene | AssetKind::Texture);
    let highlighted = open.is_some_and(|path| path == entry.path);
    let sense = if openable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let row = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0 + hierarchy_indent(depth, 12.0));
            if let Some(expanded) = expanded {
                let triangle = ui.add(
                    egui::Label::new(
                        if *expanded {
                            ICON_KEYBOARD_ARROW_DOWN
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT
                        }
                        .outlined()
                        .rich_text()
                        .size(13.0)
                        .color(TEXT_FAINT),
                    )
                    .sense(Sense::click()),
                );
                if triangle.clicked() {
                    *expanded = !*expanded;
                }
                ui.add_space(2.0);
            } else {
                // The same space a triangle would take, so names in a listing
                // line up whether or not their image is sliced.
                ui.add_space(12.0);
            }
            // Every label in the row is given the row's own sense. A widget
            // inside a scope takes precedence over the scope, and an ordinary
            // label is selectable text, so it answers a double click by
            // selecting a word rather than letting the row have it.
            let icon = ui.add(
                egui::Label::new(
                    asset_icon(entry.kind)
                        .outlined()
                        .rich_text()
                        .size(15.0)
                        .color(if highlighted { ACCENT } else { TEXT_FAINT }),
                )
                .sense(sense),
            );
            ui.add_space(2.0);
            // Under a search the path below the root is what tells two files of
            // the same name apart.
            let text = if searching {
                &entry.relative
            } else {
                &entry.name
            };
            let label = ui.add(
                egui::Label::new(RichText::new(text).size(11.0).color(if highlighted {
                    TEXT
                } else {
                    TEXT_MUTED
                }))
                // Not selectable text: a double click on a file name means
                // open it, and selecting the word "json" is not a thing anyone
                // wanted from a file listing.
                .selectable(false)
                .sense(sense),
            );
            let kind = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(entry.kind.label())
                                .size(10.0)
                                .color(TEXT_FAINT),
                        )
                        .sense(sense),
                    )
                })
                .inner;
            icon | label | kind
        })
        .inner
    });
    let row = row.response | row.inner;
    if openable {
        row.on_hover_text("Double-click to open")
    } else {
        row
    }
}

pub(super) fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + hierarchy_indent(depth, 12.0));
        ui.label(
            ICON_FOLDER
                .outlined()
                .rich_text()
                .size(15.0)
                .color(if selected { ACCENT } else { TEXT_FAINT }),
        );
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(if selected { TEXT } else { TEXT_MUTED }),
        );
    });
}

pub(super) fn asset_tile(ui: &mut egui::Ui, entry: &ProjectEntry, open: Option<&Path>) -> Response {
    let highlighted = open.is_some_and(|path| path == entry.path);
    ui.vertical(|ui| {
        let tile = ui.add_sized(
            [62.0, 54.0],
            egui::Button::new(
                asset_icon(entry.kind)
                    .outlined()
                    .rich_text()
                    .size(27.0)
                    .color(if highlighted { ACCENT } else { TEXT_MUTED }),
            )
            .fill(PANEL_RAISED)
            .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
        );
        ui.add_sized(
            [62.0, 17.0],
            egui::Label::new(RichText::new(&entry.name).size(10.0).color(TEXT_MUTED)).truncate(),
        );
        if entry.kind == AssetKind::Scene {
            tile.on_hover_text("Double-click to open")
        } else {
            tile
        }
    })
    .inner
}
