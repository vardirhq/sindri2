//! The frame around the work: title bar, menus, and status bar.
//!
//! The window's own furniture, as distinct from any one panel's. What belongs
//! here is what is true of the whole editor — which scene is open, whether it
//! is running, whether anything has gone wrong — and nothing that belongs to
//! the thing being edited.

use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke, Vec2};

use crate::preferences::{CameraProjection, Layout as WorkspaceLayout};
use crate::ui::icons;
use crate::ui::theme::{color, hairline, metric, text};
use crate::ui::widgets::{button, panel, toolbar};

use super::runtime::{PLAYING_TIP, Transport, play_button, transport_icon};
use super::unsaved::Discarding;
use super::{EditorApp, WorkspaceTab};

/// How much room the transport group takes, so the bar can centre it.
///
/// Measured from the controls rather than guessed: the bar used to push the
/// transport along with a fraction of the available width, which put it in a
/// different place at every window size.
const TRANSPORT_WIDTH: f32 = 214.0;

/// The mark in the corner: Sindri's forge diamond, painted rather than set.
///
/// The bundled Inter subset has no geometric glyph that would do, and the
/// documentation site's brandmark is a rotated square — three lines of painter
/// rather than an image asset the editor would have to load.
fn brandmark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
    let centre = rect.center();
    let arm = 6.5;
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            centre + Vec2::new(0.0, -arm),
            centre + Vec2::new(arm, 0.0),
            centre + Vec2::new(0.0, arm),
            centre + Vec2::new(-arm, 0.0),
        ],
        color::FORGE,
        Stroke::NONE,
    ));
}

/// A menu whose button reads as part of the bar rather than as a control.
fn bar_menu<R>(
    ui: &mut egui::Ui,
    label: &str,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    ui.menu_button(
        RichText::new(label)
            .size(text::LABEL)
            .color(color::TEXT_MUTED),
        |ui| {
            ui.set_min_width(196.0);
            contents(ui)
        },
    )
}

pub(super) fn projection_choice(ui: &mut egui::Ui, current: &mut CameraProjection) {
    button::Segmented::new(current)
        .option(
            CameraProjection::Perspective,
            "Perspective",
            "Look at the scene the way a perspective camera would",
        )
        .option(
            CameraProjection::Orthographic,
            "Ortho",
            "Look at the scene without perspective, for placing things exactly",
        )
        .show(ui);
}

impl EditorApp {
    pub(super) fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(metric::TOP_BAR_HEIGHT)
            .frame(egui::Frame::new().fill(color::HEADER))
            .show(ui, |ui| {
                let base = ui.max_rect();
                ui.painter()
                    .hline(base.x_range(), base.bottom() - 0.5, hairline());
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_space(12.0);
                    brandmark(ui);
                    ui.label(
                        RichText::new("SINDRI")
                            .strong()
                            .size(text::TITLE)
                            .color(color::TEXT),
                    );
                    toolbar::divider(ui);
                    self.file_menu(ui);
                    self.edit_menu(ui);
                    self.view_menu(ui);
                    // Centred on the window rather than on whatever the menus
                    // left over: measured from the bar itself, so the transport
                    // does not drift sideways as the menu labels change.
                    let centre = base.center().x - TRANSPORT_WIDTH / 2.0;
                    let lead = (centre - ui.cursor().left()).max(12.0);
                    ui.add_space(lead);
                    self.transport(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        // Which arrangement the window is in, said where the
                        // View menu that changes it can be reached.
                        ui.label(
                            RichText::new(self.preferences.layout.label())
                                .size(text::NOTE)
                                .color(color::TEXT_FAINT),
                        );
                    });
                });
            });
    }

    /// Undo, redo, and the three states the engine can be in.
    fn transport(&mut self, ui: &mut egui::Ui) {
        let transport = Transport::of(self.lifecycle.state());
        toolbar::group(ui, |ui| {
            let undo_tip = self.history.undo_label().map_or_else(
                || "Nothing to undo".to_owned(),
                |label| format!("Undo {label}  (Ctrl+Z)"),
            );
            if transport_icon(ui, icons::UNDO, false, self.history.can_undo(), &undo_tip).clicked()
            {
                self.undo();
            }
            let redo_tip = self.history.redo_label().map_or_else(
                || "Nothing to redo".to_owned(),
                |label| format!("Redo {label}  (Ctrl+Shift+Z)"),
            );
            if transport_icon(ui, icons::REDO, false, self.history.can_redo(), &redo_tip).clicked()
            {
                self.redo();
            }
        });
        ui.add_space(6.0);
        // Two controls and a word, for three states. Stop puts back everything
        // playing changed; going back to the *authored* scene is File → Discard
        // changes, which says what it will cost.
        if play_button(ui, transport).clicked() {
            self.toggle_play_mode();
        }
        if transport_icon(
            ui,
            icons::PAUSE,
            transport == Transport::Paused,
            transport.is_playing(),
            transport.pause_tip(),
        )
        .clicked()
        {
            self.toggle_pause();
        }
        ui.add_space(4.0);
        // What the editor is doing, in a word, so the state is read rather than
        // inferred from which control looks lit.
        toolbar::chip(
            ui,
            transport.label(),
            if transport.is_playing() {
                color::FORGE_BRIGHT
            } else {
                color::TEXT_FAINT
            },
        );
    }

    /// Chooses how the workspace is arranged.
    ///
    /// The choice is a preference rather than session state, so it survives a
    /// restart: rearranging the editor every time it opens is the thing this
    /// exists to stop.
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        bar_menu(ui, "View", |ui| {
            ui.label(
                RichText::new("LAYOUT")
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            );
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
    /// scene, so the reason it cannot be used is visible. The same is true of
    /// every entry here while the scene is playing: each of them either writes
    /// the running world to the file or replaces the world a run is using.
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let saveable = self.file.path().is_some();
        let authoring = self.authoring_enabled();
        let playing_tip = PLAYING_TIP;
        bar_menu(ui, "File", |ui| {
            if ui
                .add_enabled(authoring, egui::Button::new("Open scene…"))
                .clicked()
            {
                self.discard_or_confirm(Discarding::OpenAnother, ui.ctx());
                ui.close();
            }
            ui.separator();
            let save = ui.add_enabled(
                saveable && authoring,
                egui::Button::new("Save scene").shortcut_text("Ctrl+S"),
            );
            if save.clicked() {
                self.save();
                ui.close();
            }
            if !authoring {
                save.on_disabled_hover_text(playing_tip);
            }
            if ui
                .add_enabled(saveable && authoring, egui::Button::new("Reload from disk"))
                .clicked()
            {
                self.discard_or_confirm(Discarding::Reload, ui.ctx());
                ui.close();
            }
            ui.separator();
            // Drawn in the colour the editor uses for anything that throws work
            // away, so it does not read as one more neutral menu entry.
            if ui
                .add_enabled(
                    authoring,
                    egui::Button::new(RichText::new("Discard changes").color(color::DANGER_TEXT)),
                )
                .clicked()
            {
                self.discard_or_confirm(Discarding::Reset, ui.ctx());
                ui.close();
            }
        });
    }

    /// Undo and redo, in the menu people look in for them.
    ///
    /// The same two actions as the toolbar icons and the keyboard, labelled
    /// with what they would undo, which is the thing a menu can say and an icon
    /// cannot.
    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        let authoring = self.authoring_enabled();
        bar_menu(ui, "Edit", |ui| {
            let undo = self.history.undo_label().map_or_else(
                || "Undo".to_owned(),
                |label| format!("Undo {}", label.to_lowercase()),
            );
            if ui
                .add_enabled(
                    authoring && self.history.can_undo(),
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
                    authoring && self.history.can_redo(),
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
            .exact_size(metric::STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(color::HEADER))
            .show(ui, |ui| {
                let base = ui.max_rect();
                ui.painter()
                    .hline(base.x_range(), base.top() + 0.5, hairline());
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(10.0);
                    let healthy = self.problem().is_none();
                    panel::status_dot(
                        ui,
                        if healthy {
                            color::SUCCESS
                        } else {
                            color::DANGER
                        },
                    );
                    ui.label(
                        // Not "the renderer reported an error": what went wrong
                        // is as likely to be a file that would not open, and
                        // the notice beside the viewport says which.
                        RichText::new(if healthy {
                            "Renderer ready"
                        } else {
                            "Something went wrong"
                        })
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                    );
                    toolbar::divider(ui);
                    ui.label(
                        icons::SCENE
                            .outlined()
                            .rich_text()
                            .size(13.0)
                            .color(color::TEXT_FAINT),
                    );
                    ui.label(
                        RichText::new(self.file.label())
                            .size(text::LABEL)
                            .color(color::TEXT_MUTED),
                    );
                    // Unsaved work is a state worth spotting from across the
                    // room, so it is a marked chip rather than a word in
                    // brackets after the file name.
                    if self.unsaved() {
                        toolbar::chip(ui, "Unsaved", color::FORGE);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);
                        // Counted rather than guessed from whether a notice is
                        // showing, which is what this used to do: it said "1
                        // Error" for anything at all and never mentioned a
                        // warning.
                        let counts = self.console.counts();
                        ui.label(RichText::new(counts.summary()).size(text::LABEL).color(
                            if counts.errors > 0 {
                                color::DANGER_TEXT
                            } else {
                                color::TEXT_FAINT
                            },
                        ));
                        if counts.errors > 0 {
                            panel::status_dot(ui, color::DANGER);
                        }
                    });
                });
            });
    }
}

/// Which workspace a tab selects, and what it is called.
pub(super) const fn workspace_label(tab: WorkspaceTab) -> &'static str {
    match tab {
        WorkspaceTab::Scene => "Scene",
        WorkspaceTab::Game => "Game",
    }
}
