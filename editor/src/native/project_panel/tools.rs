//! The browser's own controls, and what it says when there is nothing to list.
//!
//! Separate from the listing because they are answering a different question.
//! The listing is about the project on disk — which files are there, what each
//! one is, what can be done with it. These are about the browser: how it
//! presents, what it is filtered to, and how much of the project it is looking
//! at.

use eframe::egui::{self, Align, Layout, RichText};

use crate::preferences::{AssetScope, AssetView};
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{button, panel};

use super::BrowserAction;

/// How much room the controls on the right of the toolbar take together.
///
/// A measured constant rather than a guess: the search box beside them is
/// allocated the rest, and getting it wrong is how a toolbar overflows its
/// panel. The scope switch is counted whether or not it is drawn, so the search
/// box does not change width between two projects.
const CONTROLS_WIDTH: f32 = 182.0;

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
pub(super) fn browser_tools(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    scope: Option<&mut AssetScope>,
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
            if let Some(scope) = scope {
                scope_switch(ui, scope);
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

/// The switch between the assets and the whole project.
///
/// Drawn only where the two differ, which the caller decides: a project whose
/// scene sits beside its `textures/` has one listing either way, and a control
/// that swaps a listing for the same listing is a control that does nothing.
fn scope_switch(ui: &mut egui::Ui, scope: &mut AssetScope) {
    let showing_all = *scope == AssetScope::Project;
    if button::icon(
        ui,
        icons::ALL_FILES,
        showing_all,
        if showing_all {
            "Showing every file in the project. Press to list only the assets a scene can name."
        } else {
            "Showing the assets a scene can name. Press to list every file in the project."
        },
    )
    .clicked()
    {
        *scope = if showing_all {
            AssetScope::Assets
        } else {
            AssetScope::Project
        };
    }
}

/// The way back out of a folder, for the arrangement that has no folder tree.
pub(super) fn back_out(ui: &mut egui::Ui, to: &str) -> bool {
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        button::labelled(
            ui,
            &format!("Back to {to}"),
            button::Intent::Quiet,
            "List everything again",
        )
        .clicked()
    })
    .inner
}

/// What the browser says when the listing is empty, which depends on why.
pub(super) fn empty_listing(ui: &mut egui::Ui, searching: bool, scoped: bool) {
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
