//! The palette on screen: what it offers, how it is driven, and what it does.
//!
//! The model in `crate::palette` knows how to rank candidates and nothing about
//! the editor. This is the other half: gathering what there is to find, drawing
//! the field and the list over everything else, and carrying out whichever
//! result was chosen.

use eframe::egui::{self, Align, Align2, FontId, Layout, Pos2, Rect, RichText, Stroke, Vec2};

use crate::dock::{Panel as DockPanel, Preset};
use crate::palette::{Action, Candidate, Kind, VISIBLE, Verb};
use crate::ui::icons;
use crate::ui::theme::{color, hairline, metric, text};
use crate::ui::widgets::panel;

use super::EditorApp;
use super::unsaved::Discarding;

/// How wide the hint in the title bar is.
pub(super) const HINT_WIDTH: f32 = 200.0;

/// How wide the palette is, and how far down from the top of the window.
const WIDTH: f32 = 620.0;
const FROM_TOP: f32 = 96.0;
const ROW_HEIGHT: f32 = 34.0;

impl EditorApp {
    /// Everything the palette can currently find.
    ///
    /// Built in this order on purpose: the things there are a handful of come
    /// before the things there are hundreds of, because an empty query shows
    /// the first ten and ten assets in alphabetical order tell nobody anything.
    fn palette_candidates(&self) -> Vec<Candidate> {
        let mut found = Vec::new();
        for panel in DockPanel::ALL {
            let open = self.preferences.workspace.is_open(panel);
            found.push(
                Candidate::new(panel.label(), Kind::Panel, Action::ShowPanel(panel))
                    .noted(if open { "Show" } else { "Reopen" }),
            );
        }
        for (verb, label, kind) in Verb::ALL {
            found.push(Candidate::new(label, kind, Action::Run(verb)));
        }
        for preset in Preset::ALL {
            found.push(
                Candidate::new(preset.label(), Kind::Arrangement, Action::UsePreset(preset))
                    .noted(preset.note()),
            );
        }
        // The scene's entities, by the name the hierarchy shows.
        for (entity, _) in self.world.entities() {
            if let Some(record) = self.world.get(entity) {
                found.push(Candidate::new(
                    super::hierarchy::row::entity_name(record),
                    Kind::Entity,
                    Action::GoToEntity(entity),
                ));
            }
        }
        // Then the project, which is the long tail.
        for entry in self.project.entries() {
            let scene = matches!(entry.kind, crate::project::AssetKind::Scene);
            let path = entry.path.display().to_string();
            let action = if scene {
                Action::OpenScene(path)
            } else {
                Action::SelectAsset(path)
            };
            found.push(
                Candidate::new(
                    entry.name.clone(),
                    if scene { Kind::Scene } else { Kind::Asset },
                    action,
                )
                .noted(entry.relative.clone()),
            );
        }
        found
    }

    /// Opens, closes, drives and draws the palette.
    ///
    /// Called before anything else reads the keyboard, because while it is open
    /// it owns every key: an arrow that moved the selection and also nudged an
    /// entity would be the worst of both.
    pub(super) fn palette(&mut self, context: &egui::Context) -> bool {
        let toggle = context.input_mut(|input| {
            input.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::K,
            ))
        });
        if toggle {
            if self.palette.is_open() {
                self.palette.close();
            } else {
                self.palette.open();
            }
        }
        if !self.palette.is_open() {
            return false;
        }
        let results = self.palette.rank(self.palette_candidates());
        let (escaped, up, down, entered) = context.input_mut(|input| {
            (
                input.key_pressed(egui::Key::Escape),
                input.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                input.key_pressed(egui::Key::Enter),
            )
        });
        if escaped {
            self.palette.close();
            return true;
        }
        let moved = isize::try_from(down).unwrap_or(0) - isize::try_from(up).unwrap_or(0);
        if moved != 0 {
            self.palette.step(moved, results.len());
        }
        let mut taken = if entered {
            results.get(self.palette.chosen(results.len())).cloned()
        } else {
            None
        };
        if let Some(clicked) = self.palette_window(context, &results) {
            taken = Some(clicked);
        }
        if let Some(candidate) = taken {
            self.palette.close();
            self.run_palette_action(candidate.action, context);
        }
        true
    }

    /// The field and the list, over everything.
    fn palette_window(
        &mut self,
        context: &egui::Context,
        results: &[Candidate],
    ) -> Option<Candidate> {
        let window = context.content_rect();
        // `VISIBLE` is small, so the row count fits a `u8` and converts to a
        // float without the lossy cast clippy rightly objects to.
        let rows = u8::try_from(results.len().min(VISIBLE)).unwrap_or(0);
        let height = 46.0 + ROW_HEIGHT * f32::from(rows) + 8.0;
        let rect = Rect::from_min_size(
            Pos2::new(window.center().x - WIDTH / 2.0, window.top() + FROM_TOP),
            Vec2::new(WIDTH, height),
        );
        let mut clicked = None;
        // Drawn above every panel and every overlay, because it is the one
        // thing that is never part of the arrangement.
        egui::Area::new(egui::Id::new("command-palette"))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(context, |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                panel::overlay_frame().show(ui, |ui| {
                    ui.set_clip_rect(rect.shrink(1.0));
                    self.palette_field(ui, rect);
                    clicked = self.palette_results(ui, results);
                });
            });
        clicked
    }

    /// The search field, focused the frame it appears.
    fn palette_field(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let row = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 44.0));
        ui.painter().text(
            Pos2::new(row.left() + 16.0, row.center().y),
            Align2::LEFT_CENTER,
            icons::SEARCH.outlined().codepoint,
            FontId::new(16.0, icons::SEARCH.outlined().font_family()),
            color::TEXT_FAINT,
        );
        let field = Rect::from_min_max(
            Pos2::new(row.left() + 42.0, row.top()),
            Pos2::new(row.right() - 12.0, row.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(field)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let response = child.add(
            egui::TextEdit::singleline(&mut self.palette.query)
                .frame(egui::Frame::NONE)
                .desired_width(field.width())
                .font(FontId::proportional(text::BODY))
                .hint_text("Find a panel, an entity, a file, or something to do"),
        );
        if self.palette.take_focus() {
            response.request_focus();
        }
        ui.allocate_rect(row, egui::Sense::hover());
        ui.painter()
            .hline(row.x_range(), row.bottom() - 0.5, hairline());
    }

    /// The results, one row each, with the chosen one lit.
    fn palette_results(&mut self, ui: &mut egui::Ui, results: &[Candidate]) -> Option<Candidate> {
        if results.is_empty() {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Nothing matches that")
                        .size(text::LABEL)
                        .color(color::TEXT_FAINT),
                );
            });
            return None;
        }
        let chosen = self.palette.chosen(results.len());
        let mut clicked = None;
        for (index, candidate) in results.iter().enumerate() {
            let (row, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), ROW_HEIGHT),
                egui::Sense::click(),
            );
            let lit = index == chosen;
            if lit || response.hovered() {
                ui.painter().rect_filled(
                    row.shrink2(Vec2::new(4.0, 1.0)),
                    4.0,
                    if lit { color::RAISED } else { color::PANEL },
                );
            }
            if lit {
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        Pos2::new(row.left() + 4.0, row.top() + 6.0),
                        Vec2::new(2.0, row.height() - 12.0),
                    ),
                    1.0,
                    color::FORGE,
                );
            }
            if response.hovered() {
                self.palette.choose(index);
            }
            if response.clicked() {
                clicked = Some(candidate.clone());
            }
            let painter = ui.painter_at(row);
            painter.text(
                Pos2::new(row.left() + 16.0, row.center().y),
                Align2::LEFT_CENTER,
                &candidate.label,
                FontId::proportional(text::BODY),
                if lit { color::TEXT } else { color::TEXT_MUTED },
            );
            let label_width = painter
                .layout_no_wrap(
                    candidate.label.clone(),
                    FontId::proportional(text::BODY),
                    color::TEXT,
                )
                .size()
                .x;
            if !candidate.note.is_empty() {
                painter.text(
                    Pos2::new(row.left() + 26.0 + label_width, row.center().y),
                    Align2::LEFT_CENTER,
                    &candidate.note,
                    FontId::proportional(text::NOTE),
                    color::TEXT_FAINT,
                );
            }
            painter.text(
                Pos2::new(row.right() - metric::GUTTER, row.center().y),
                Align2::RIGHT_CENTER,
                candidate.kind.label(),
                FontId::proportional(text::NOTE),
                color::TEXT_FAINT,
            );
        }
        clicked
    }

    /// Carries out whichever result was chosen.
    fn run_palette_action(&mut self, action: Action, context: &egui::Context) {
        match action {
            Action::ShowPanel(panel) => {
                if !self.preferences.workspace.is_open(panel) {
                    self.preferences.workspace.toggle(panel);
                } else if let Some((place, index)) = self.preferences.workspace.location(panel) {
                    // Already placed, so showing it means selecting its tab and
                    // unrolling the overlay it is in rather than moving it.
                    self.preferences.workspace.select(place, index);
                }
            }
            Action::UsePreset(preset) => {
                self.preferences.workspace = crate::dock::Workspace::preset(preset);
            }
            Action::GoToEntity(entity) => {
                self.select(Some(entity));
                self.focus_selection();
            }
            Action::SelectAsset(path) => self.select_asset(std::path::Path::new(&path)),
            Action::OpenScene(path) => {
                self.discard_or_confirm(Discarding::OpenPath(path.into()), context);
            }
            Action::Run(verb) => self.run_verb(verb, context),
        }
    }

    fn run_verb(&mut self, verb: Verb, context: &egui::Context) {
        match verb {
            Verb::Save => self.save(),
            Verb::SaveAs => self.save_as(),
            Verb::ReloadFromDisk => self.discard_or_confirm(Discarding::Reload, context),
            Verb::NewScene => self.discard_or_confirm(Discarding::NewScene, context),
            Verb::OpenScene => self.discard_or_confirm(Discarding::OpenAnother, context),
            Verb::OpenProject => self.browse_for_project(context),
            Verb::Welcome => self.open_welcome(),
            Verb::Undo => self.undo(),
            Verb::Redo => self.redo(),
            Verb::TogglePlay => self.toggle_play_mode(),
            Verb::TogglePause => self.toggle_pause(),
            Verb::Step => self.single_step(context),
            Verb::DiscardChanges => self.discard_or_confirm(Discarding::Reset, context),
        }
    }
}

/// A hint in the title bar that the palette is there at all.
///
/// A keyboard shortcut nobody is told about is a keyboard shortcut nobody uses,
/// and the palette is the answer to "where did that panel go" — which is a
/// question people have before they have read any documentation.
pub(super) fn palette_hint(ui: &mut egui::Ui, open: &mut bool) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(HINT_WIDTH, 24.0), egui::Sense::click());
    let painter = ui.painter_at(rect);
    // Raised rather than sunken. Drawn in `INK` this sat invisible against the
    // scrim it floats on -- near-black on near-black -- which for the one
    // control that advertises the palette is the whole job undone.
    painter.rect_filled(
        rect,
        6.0,
        if response.hovered() {
            color::RAISED
        } else {
            color::PANEL
        },
    );
    painter.rect_stroke(
        rect,
        5.0,
        if response.hovered() {
            Stroke::new(1.0, color::FORGE_DIM)
        } else {
            hairline()
        },
        egui::StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        icons::SEARCH.outlined().codepoint,
        FontId::new(13.0, icons::SEARCH.outlined().font_family()),
        color::TEXT_FAINT,
    );
    painter.text(
        Pos2::new(rect.left() + 27.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Find anything…",
        FontId::proportional(text::LABEL),
        color::TEXT_MUTED,
    );
    painter.text(
        Pos2::new(rect.right() - 9.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "Ctrl K",
        FontId::proportional(text::NOTE),
        color::TEXT_FAINT,
    );
    if response.clicked() {
        *open = true;
    }
}
