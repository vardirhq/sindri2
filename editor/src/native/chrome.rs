//! The frame around the work: title bar, menus, tabs, and status bar.

use eframe::egui::{self, Align, Layout, RichText, Stroke};
use egui_material_icons::icons::{ICON_PAUSE, ICON_PLAY_ARROW, ICON_REDO, ICON_STOP, ICON_UNDO};
use sindri_core::EngineState;

use crate::preferences::{BottomTab, CameraProjection, Layout as WorkspaceLayout};

use super::runtime::{play_button, transport_icon};
use super::theme::{
    ACCENT, ACCENT_BRIGHT, ACCENT_SOFT, BORDER, PANEL_RAISED, SUCCESS, TEXT, TEXT_FAINT,
    TEXT_MUTED, TOP_BG, status_dot,
};
use super::unsaved::Discarding;
use super::{EditorApp, WorkspaceTab};

pub(super) fn workspace_tab(
    ui: &mut egui::Ui,
    current: &mut WorkspaceTab,
    value: WorkspaceTab,
    label: &str,
) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

pub(super) fn bottom_tab(
    ui: &mut egui::Ui,
    current: &mut BottomTab,
    value: BottomTab,
    label: &str,
) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

pub(super) fn projection_button(
    ui: &mut egui::Ui,
    current: &mut CameraProjection,
    value: CameraProjection,
    label: &str,
) {
    let selected = *current == value;
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(11.0).color(if selected {
                ACCENT_BRIGHT
            } else {
                TEXT_FAINT
            }))
            .selected(selected)
            .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
            .stroke(Stroke::new(1.0, if selected { ACCENT } else { BORDER })),
        )
        .clicked()
    {
        *current = value;
    }
}

impl EditorApp {
    pub(super) fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 11.0;
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("SINDRI")
                            .strong()
                            .size(15.0)
                            .color(ACCENT_BRIGHT),
                    );
                    ui.add_space(8.0);
                    self.file_menu(ui);
                    self.edit_menu(ui);
                    self.view_menu(ui);
                    // "Scene", "Build", "Tools", and "Help" used to sit here.
                    // None of them opened: they were labels shaped like menus,
                    // which is a promise about four features that do not exist.
                    ui.add_space((ui.available_width() * 0.22).max(16.0));
                    let undo_tip = self.history.undo_label().map_or_else(
                        || "Nothing to undo".to_owned(),
                        |label| format!("Undo {label}"),
                    );
                    if transport_icon(ui, ICON_UNDO, false, self.history.can_undo(), &undo_tip)
                        .clicked()
                    {
                        self.undo();
                    }
                    let redo_tip = self.history.redo_label().map_or_else(
                        || "Nothing to redo".to_owned(),
                        |label| format!("Redo {label}"),
                    );
                    if transport_icon(ui, ICON_REDO, false, self.history.can_redo(), &redo_tip)
                        .clicked()
                    {
                        self.redo();
                    }
                    let running = self.lifecycle.state() == EngineState::Running;
                    // Stop stops. It used to reset the scene to the file,
                    // which is what the symbol between Pause and Play means to
                    // nobody, and it did that without asking. Going back to
                    // the authored scene is File → Discard changes, which now
                    // says what it will cost.
                    let playing = matches!(
                        self.lifecycle.state(),
                        EngineState::Running | EngineState::Paused
                    );
                    if transport_icon(ui, ICON_STOP, false, playing, "Stop").clicked() {
                        self.stop_playback();
                    }
                    if transport_icon(ui, ICON_PAUSE, !running, running, "Pause").clicked() {
                        self.pause();
                    }
                    if transport_icon(ui, ICON_PLAY_ARROW, running, true, "Play").clicked()
                        || play_button(ui, running).clicked()
                    {
                        self.toggle_playback();
                    }
                    // A project name and a chevron used to sit at this end,
                    // naming a project that did not exist and opening nothing.
                    // What project is open is the browser's business, and it
                    // says so from the directory it is reading.
                });
            });
    }

    /// Chooses how the workspace is arranged.
    ///
    /// The choice is a preference rather than session state, so it survives a
    /// restart: rearranging the editor every time it opens is the thing this
    /// exists to stop.
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("View").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(170.0);
            ui.label(RichText::new("Layout").size(11.0).color(TEXT_FAINT));
            for layout in WorkspaceLayout::ALL {
                if ui
                    .selectable_label(self.preferences.layout == layout, layout.label())
                    .clicked()
                {
                    self.preferences.layout = layout;
                    ui.close();
                }
            }
        });
    }

    /// Save is disabled rather than hidden when there is no file behind the
    /// scene, so the reason it cannot be used is visible.
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let saveable = self.file.path().is_some();
        ui.menu_button(RichText::new("File").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(190.0);
            if ui.button("Open scene…").clicked() {
                self.discard_or_confirm(Discarding::OpenAnother, ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    saveable,
                    egui::Button::new("Save scene").shortcut_text("Ctrl+S"),
                )
                .clicked()
            {
                self.save();
                ui.close();
            }
            if ui
                .add_enabled(saveable, egui::Button::new("Reload from disk"))
                .clicked()
            {
                self.discard_or_confirm(Discarding::Reload, ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button("Discard changes").clicked() {
                self.discard_or_confirm(Discarding::Reset, ui.ctx());
                ui.close();
            }
        });
    }

    /// Undo and redo, in the menu people look in for them.
    ///
    /// The same two actions as the toolbar icons and the keyboard, labelled
    /// with what they would undo, which is the thing a menu can say and an icon
    /// cannot. "Edit" was a label shaped like a menu until this.
    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("Edit").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(190.0);
            let undo = self.history.undo_label().map_or_else(
                || "Undo".to_owned(),
                |label| format!("Undo {}", label.to_lowercase()),
            );
            if ui
                .add_enabled(
                    self.history.can_undo(),
                    egui::Button::new(undo).shortcut_text("Ctrl+Z"),
                )
                .clicked()
            {
                self.undo();
                ui.close();
            }
            let redo = self.history.redo_label().map_or_else(
                || "Redo".to_owned(),
                |label| format!("Redo {}", label.to_lowercase()),
            );
            if ui
                .add_enabled(
                    self.history.can_redo(),
                    egui::Button::new(redo).shortcut_text("Ctrl+Shift+Z"),
                )
                .clicked()
            {
                self.redo();
                ui.close();
            }
        });
    }

    pub(super) fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("editor-status")
            .exact_size(26.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    let healthy = self.problem().is_none();
                    status_dot(ui, if healthy { SUCCESS } else { ACCENT_BRIGHT });
                    ui.label(
                        // Not "the renderer reported an error": what went wrong
                        // is as likely to be a file that would not open, and
                        // the notice beside the viewport says which.
                        RichText::new(if healthy {
                            "Renderer ready"
                        } else {
                            "Something went wrong"
                        })
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("|").size(11.0).color(BORDER));
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(if self.unsaved() {
                            format!("{} (unsaved)", self.file.label())
                        } else {
                            self.file.label()
                        })
                        .size(11.0)
                        .color(if self.unsaved() {
                            ACCENT
                        } else {
                            TEXT_MUTED
                        }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        // Counted rather than guessed from whether a notice is
                        // showing, which is what this used to do: it said "1
                        // Error" for anything at all and never mentioned a
                        // warning, because nothing in the editor could produce
                        // one.
                        let counts = self.console.counts();
                        ui.label(RichText::new(counts.summary()).size(11.0).color(
                            if counts.errors > 0 {
                                ACCENT_BRIGHT
                            } else {
                                TEXT_FAINT
                            },
                        ));
                    });
                });
            });
    }
}
