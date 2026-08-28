//! What the welcome window looks like.
//!
//! Four regions and no more: what this program is, the projects you have, the
//! two ways to get another one, and what happens next time. Everything is drawn
//! from `ui::theme` tokens and `ui::widgets`, so the window that opens before
//! the editor is recognisably the same tool as the editor.
//!
//! A project is a card rather than a list row. The editor's rows are 21 points
//! tall because a hierarchy is a hundred of them and the question is always
//! "which one"; there are at most a dozen projects here, the question is "which
//! am I working on", and the answer needs a name and the path under it — the
//! same project name in two checkouts is the normal case, not an odd one.

use eframe::egui::{self, Align, Align2, Layout, Pos2, RichText, UiBuilder};

use crate::ui::theme::{color, hairline, hairline_soft, metric, radius, text};
use crate::ui::widgets::{
    button::{self, Intent},
    panel,
};
use crate::ui::{icons, widgets::button::outline};

use std::path::{Path, PathBuf};

use super::{Listing, NewProject, Request, Welcome};

/// How tall a project card is: a name, and the path under it.
const CARD_HEIGHT: f32 = 46.0;

/// How wide the column of actions is.
const RAIL_WIDTH: f32 = 236.0;

/// What one project card was clicked with.
enum Clicked {
    /// Open it.
    Open,
    /// Take it off the list.
    Forget,
}

impl Welcome {
    pub(super) fn draw(&mut self, ui: &mut egui::Ui) {
        Self::header(ui);
        self.footer(ui);
        self.rail(ui);
        self.projects(ui);
        // Last, so the question sits over everything it is about.
        self.creating_form(ui);
    }

    /// The mark, the name, and which build this is.
    fn header(ui: &mut egui::Ui) {
        egui::Panel::top("welcome-header")
            .exact_size(metric::TOP_BAR_HEIGHT)
            .frame(egui::Frame::new().fill(color::HEADER))
            .show(ui, |ui| {
                let base = ui.max_rect();
                ui.painter()
                    .hline(base.x_range(), base.bottom() - 0.5, hairline());
                ui.horizontal_centered(|ui| {
                    ui.add_space(metric::GUTTER + 4.0);
                    super::super::chrome::brandmark(ui);
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Sindri")
                            .size(text::TITLE)
                            .strong()
                            .color(color::TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(metric::GUTTER + 4.0);
                        ui.label(
                            RichText::new(format!("Editor {}", env!("CARGO_PKG_VERSION")))
                                .size(text::NOTE)
                                .color(color::TEXT_FAINT),
                        );
                    });
                });
            });
    }

    /// What happens next launch, and whatever went wrong last.
    fn footer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("welcome-footer")
            .exact_size(metric::TOP_BAR_HEIGHT)
            .frame(egui::Frame::new().fill(color::HEADER))
            .show(ui, |ui| {
                let base = ui.max_rect();
                ui.painter()
                    .hline(base.x_range(), base.top() + 0.5, hairline());
                ui.horizontal_centered(|ui| {
                    ui.add_space(metric::GUTTER + 4.0);
                    let mut open_last = self.open_last;
                    if ui
                        .checkbox(
                            &mut open_last,
                            RichText::new("Open my last project next time, skipping this window")
                                .size(text::LABEL)
                                .color(color::TEXT_MUTED),
                        )
                        .changed()
                    {
                        self.open_last = open_last;
                        self.changed = true;
                    }
                    if let Some(problem) = self.problem.clone() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(metric::GUTTER + 4.0);
                            ui.label(
                                RichText::new(problem)
                                    .size(text::LABEL)
                                    .color(color::DANGER_TEXT),
                            );
                        });
                    }
                });
            });
    }

    /// The two ways to get a project that is not on the list, and the ones this
    /// repository ships.
    fn rail(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("welcome-actions")
            .exact_size(RAIL_WIDTH)
            .frame(panel::frame())
            .show(ui, |ui| {
                panel::body(ui, |ui| {
                    ui.add_space(metric::GAP);
                    if button::wide(
                        ui,
                        icons::ADD,
                        "New project",
                        Intent::Primary,
                        "Make a project: a folder, a scene, and somewhere to put assets",
                    )
                    .clicked()
                    {
                        self.creating = Some(NewProject::default());
                        self.problem = None;
                    }
                    ui.add_space(metric::GAP);
                    if button::wide(
                        ui,
                        icons::FOLDER,
                        "Open project",
                        Intent::Normal,
                        "Open a folder that already holds a sindri.toml",
                    )
                    .clicked()
                    {
                        self.browse();
                    }

                    // A shipped project already on the list is not offered
                    // twice: it is on the list because it has been opened, and
                    // a second row that opens the same project is a second row
                    // saying nothing. Taking it off the recents puts it back.
                    //
                    // Taken by value so the borrow ends before a click is
                    // written back into `self`.
                    let samples: Vec<(String, PathBuf)> = self
                        .samples
                        .iter()
                        .filter(|sample| {
                            !self
                                .recent
                                .entries()
                                .iter()
                                .any(|entry| Path::new(&entry.path) == sample.root)
                        })
                        .map(|sample| (sample.name.clone(), sample.root.clone()))
                        .collect();
                    if !samples.is_empty() {
                        ui.add_space(metric::GROUP_GAP);
                        panel::rule(ui);
                        ui.label(
                            RichText::new("SHIPPED WITH SINDRI")
                                .size(text::NOTE)
                                .color(color::TEXT_FAINT),
                        );
                        ui.add_space(metric::GAP);
                        for (name, root) in samples {
                            if sample_row(ui, &name, &root) {
                                self.request = Some(Request::Open(root));
                            }
                        }
                    }
                });
            });
    }

    /// The projects, or the reason there are none.
    fn projects(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(panel::frame())
            .show(ui, |ui| {
                panel::body(ui, |ui| {
                    ui.label(
                        RichText::new("PROJECTS")
                            .size(text::NOTE)
                            .color(color::TEXT_FAINT),
                    );
                    ui.add_space(metric::GAP);
                    let rows = self.rows();
                    if rows.is_empty() {
                        panel::empty_state(
                            ui,
                            icons::PROJECT,
                            "No projects yet",
                            "Make one, open a folder that holds a sindri.toml, or start \
                             from a project shipped with Sindri.",
                        );
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for row in &rows {
                                match card(ui, row) {
                                    Some(Clicked::Open) => {
                                        // A project that is not there is not
                                        // opened: the row already says so, and
                                        // the click would fail on the load.
                                        if row.present {
                                            self.request = Some(Request::Open(row.root.clone()));
                                        } else {
                                            self.problem = Some(format!(
                                                "{} is not there any more",
                                                row.root.display()
                                            ));
                                        }
                                    }
                                    Some(Clicked::Forget) => {
                                        self.recent.forget(&row.root.display().to_string());
                                        self.changed = true;
                                    }
                                    None => {}
                                }
                            }
                        });
                });
            });
    }

    /// Asks for a folder, and takes the project in it.
    ///
    /// The dialog is opened from the window that asked for it rather than
    /// handed back to the editor, because the editor is hidden and throttled to
    /// ten frames a second while this window is up — a folder picker that took
    /// a tenth of a second to appear would feel like a click that missed.
    fn browse(&mut self) {
        let Some(root) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        if crate::project::manifest::is_project(&root) {
            self.request = Some(Request::Open(root));
        } else {
            self.problem = Some(format!(
                "{} is not a Sindri project: it has no {}",
                root.display(),
                crate::project::MANIFEST_NAME
            ));
        }
    }
}

/// One shipped project, as a compact row.
///
/// The row allocates its own rect and paints into it, rather than sensing a
/// scope wrapped around two labels. A scope built around labels reported no
/// hover and swallowed the click — and painting the hover fill afterwards would
/// have covered the text it was meant to be behind.
fn sample_row(ui: &mut egui::Ui, name: &str, root: &Path) -> bool {
    let (rect, response) = button::row_sense(ui, metric::ROW_HEIGHT);
    if response.hovered() {
        ui.painter().rect_filled(rect, radius(), color::EMBER_FAINT);
    }
    let foreground = if response.hovered() {
        color::TEXT
    } else {
        color::TEXT_MUTED
    };
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        icons::SCENE.outlined().codepoint,
        egui::FontId::new(13.0, icons::SCENE.outlined().font_family()),
        color::TEXT_FAINT,
    );
    painter.text(
        Pos2::new(rect.left() + 22.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(text::BODY),
        foreground,
    );
    response.on_hover_text(root.display().to_string()).clicked()
}

/// One project, as a card: what it is called and where it is.
fn card(ui: &mut egui::Ui, listing: &Listing) -> Option<Clicked> {
    let (rect, response) = button::row_sense(ui, CARD_HEIGHT);
    if response.hovered() {
        ui.painter().rect_filled(rect, radius(), color::EMBER_FAINT);
        outline(ui, rect, hairline());
    }
    ui.painter()
        .hline(rect.x_range(), rect.bottom() - 0.5, hairline_soft());

    let named = if listing.present {
        color::TEXT
    } else {
        color::TEXT_FAINT
    };
    let painter = ui.painter_at(rect);
    let name = painter.layout_no_wrap(
        listing.name.clone(),
        egui::FontId::proportional(text::BODY),
        named,
    );
    let left = rect.left() + metric::GUTTER;
    painter.galley(Pos2::new(left, rect.top() + 8.0), name.clone(), named);
    if !listing.present {
        painter.text(
            Pos2::new(left + name.size().x + metric::GAP, rect.top() + 9.0),
            Align2::LEFT_TOP,
            "MISSING",
            egui::FontId::proportional(text::NOTE),
            color::DANGER_TEXT,
        );
    }
    painter.text(
        Pos2::new(left, rect.bottom() - 9.0),
        Align2::LEFT_BOTTOM,
        listing.root.display().to_string(),
        egui::FontId::proportional(text::NOTE),
        color::TEXT_FAINT,
    );

    // The one control inside the row, placed in a region of its own so that it
    // answers the pointer before the row does.
    let mut forget = false;
    let corner = egui::Rect::from_min_size(
        Pos2::new(rect.right() - 26.0, rect.center().y - 9.0),
        egui::Vec2::splat(18.0),
    );
    ui.scope_builder(UiBuilder::new().max_rect(corner), |ui| {
        forget = button::row_icon(
            ui,
            icons::CLOSE,
            Intent::Quiet,
            "Take this project off the list. Nothing on disk is touched.",
        )
        .clicked();
    });

    if forget {
        return Some(Clicked::Forget);
    }
    // The card's own click means open; the remove button inside it has already
    // said what it means, and must not also read as one.
    response.clicked().then_some(Clicked::Open)
}
