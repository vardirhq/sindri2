//! The sprite slicer's panel and the preview it draws over a texture.

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};

use crate::slicer::Slicer;
use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, text};
use crate::ui::widgets::{
    button::{self, Intent},
    panel, property, section,
};

use super::EditorApp;
use super::inspector_panel::rows::number_row;

/// The heading above one section of the inspector.
///
/// A collapse chevron and an overflow menu used to sit at either end of it.
/// Neither was handled: nothing collapsed and nothing overflowed. Adding and
/// removing a component is what the menu would hold, and that is a real build
/// against the schema registry rather than a glyph.
/// An image dimension as a length to lay out with.
///
/// No image this can draw is anywhere near the width an `f32` stops counting
/// exactly, and one that were would not fit in a panel either.
#[allow(clippy::cast_precision_loss)]
fn pixels(value: u32) -> f32 {
    value as f32
}

/// A pixel measurement, as a drag leaves it.
///
/// Clamped to something an image could plausibly carry, for the reason a grid
/// side is: a drag that got away should not become a slice with no cells.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_count(value: f64) -> u32 {
    value.clamp(0.0, 4096.0) as u32
}

/// One side of a slicing grid, as a drag leaves it.
///
/// Clamped rather than validated after the fact: a grid of zero has no cells
/// and one of ten thousand is a drag that got away, and neither is a slice
/// anybody meant. The cast cannot lose anything once the value is inside that
/// range.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn grid_side(value: f64) -> u32 {
    value.clamp(1.0, 256.0) as u32
}

/// The numbers that decide where the cells fall.
///
/// Six drags in one place rather than scattered through the panel: they are one
/// idea — how the sheet is divided — and every one of them moves every cell.
fn slice_grid(ui: &mut egui::Ui, slicer: &mut Slicer) {
    section::group(ui, icons::TILEMAP, "Slice");
    let mut columns = f64::from(slicer.columns);
    let mut rows = f64::from(slicer.rows);
    let mut resized = number_row(ui, "Columns", &mut columns, 10.0, true);
    resized |= number_row(ui, "Rows", &mut rows, 10.0, true);
    if resized {
        slicer.columns = grid_side(columns);
        slicer.rows = grid_side(rows);
        slicer.fit_names();
        slicer.clamp_selection();
    }

    let mut margin_x = f64::from(slicer.margin[0]);
    let mut margin_y = f64::from(slicer.margin[1]);
    let mut spacing_x = f64::from(slicer.spacing[0]);
    let mut spacing_y = f64::from(slicer.spacing[1]);
    let mut measured = number_row(ui, "Margin X", &mut margin_x, 10.0, true);
    measured |= number_row(ui, "Margin Y", &mut margin_y, 10.0, true);
    measured |= number_row(ui, "Spacing X", &mut spacing_x, 10.0, true);
    measured |= number_row(ui, "Spacing Y", &mut spacing_y, 10.0, true);
    if measured {
        slicer.margin = [pixel_count(margin_x), pixel_count(margin_y)];
        slicer.spacing = [pixel_count(spacing_x), pixel_count(spacing_y)];
    }
}

/// Naming the chosen cell, and a list of the ones already named.
///
/// A field per cell is fine at four and unusable at two hundred and fifty-six,
/// so the sheet is named the way it is looked at: pick a cell on the image, give
/// it a name. Everything unnamed already has an answer — its index — so a list
/// of the named ones is the whole of what there is to review.
fn slice_names(ui: &mut egui::Ui, slicer: &mut Slicer) {
    section::group(ui, icons::LABEL, "Names");
    slicer.fit_names();
    slicer.clamp_selection();

    let selected = slicer.selected;
    let placeholder = selected.to_string();
    if let Some(name) = slicer.names.get_mut(selected as usize) {
        property::Property::new(&format!("Cell {selected}")).show(ui, |ui| {
            ui.add_sized(
                [property::value_width(ui), metric::CONTROL_HEIGHT],
                egui::TextEdit::singleline(name).hint_text(&placeholder),
            );
        });
    }
    section::caption(
        ui,
        "Click a cell on the image to name it. A cell left blank is called by its index.",
    );

    let named = slicer.named();
    if named.is_empty() {
        return;
    }
    ui.add_space(6.0);
    panel::note(ui, &format!("{} named", named.len()));
    let mut jump = None;
    for (index, name) in named {
        // A named cell is a row that jumps to it, drawn with the tree's own
        // banding so it reads as a list of things rather than as a paragraph.
        let row = crate::ui::widgets::tree::row(
            ui,
            icons::LABEL,
            &format!("{index}  ·  {name}"),
            crate::ui::widgets::tree::RowStyle {
                selected: index == selected,
                depth: 1,
                ..crate::ui::widgets::tree::RowStyle::default()
            },
        );
        if row.select.clicked() {
            jump = Some(index);
        }
    }
    // The list is also how a cell is found again on a sheet too large to scan.
    if let Some(jump) = jump {
        slicer.selected = jump;
    }
}

/// The image, with the slice drawn over it, and a click choosing a cell.
///
/// The whole point of doing this on the picture: a grid of numbers in a panel
/// tells you nothing about whether the cells fall on the frames, and the cells
/// falling on the frames is the entire job. The rects drawn are the ones the
/// document produces, not a second calculation that could disagree with it.
fn slice_preview(ui: &mut egui::Ui, slicer: &mut Slicer) {
    let (width, height) = slicer.size();
    let rects = slicer.cell_rects();
    let selected = slicer.selected;
    let Some(texture) = slicer.texture(ui.ctx()) else {
        panel::note(ui, "No preview: this build cannot read the image");
        return;
    };
    if width == 0 || height == 0 {
        return;
    }

    // Fitted to the panel and never enlarged past a readable multiple: a 16x16
    // sheet blown up to panel width is mostly interpolation artefacts, and a
    // 2048px one has to come down.
    let available = (ui.available_width() - 20.0).max(64.0);
    let (wide, tall) = (pixels(width), pixels(height));
    let scale = (available / wide).min(8.0);
    let size = Vec2::new(wide * scale, tall * scale);
    let texture = texture.id();

    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        // A flat ground behind the image, so a transparent sheet reads as
        // transparent rather than as the panel's own background.
        painter.rect_filled(rect, radius(), color::WELL);
        painter.image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        // Each cell as its own outline rather than lines across the image,
        // because a cell inset by a margin is not on any dividing line and
        // drawing one would say the gutters belong to a sprite.
        let cell_rect = |[x, y, w, h]: [f32; 4]| {
            egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + x * rect.width(),
                    rect.top() + y * rect.height(),
                ),
                Vec2::new(w * rect.width(), h * rect.height()),
            )
        };
        let faint = Stroke::new(1.0, color::FORGE.gamma_multiply(0.5));
        for (index, cell) in rects.iter().enumerate() {
            if u32::try_from(index).is_ok_and(|index| index == selected) {
                continue;
            }
            painter.rect_stroke(cell_rect(*cell), 0.0, faint, egui::StrokeKind::Inside);
        }
        if let Some(cell) = rects.get(selected as usize) {
            let bright = Stroke::new(2.0, color::FORGE_BRIGHT);
            painter.rect_stroke(cell_rect(*cell), 0.0, bright, egui::StrokeKind::Inside);
        }

        // Picked by hit-testing the drawn rects rather than by dividing the
        // pointer's position, so a click lands on the cell it looks like it
        // landed on even when gutters mean the cells do not tile.
        if let Some(pointer) = response.interact_pointer_pos()
            && response.clicked()
        {
            picked = rects
                .iter()
                .position(|cell| cell_rect(*cell).contains(pointer))
                .and_then(|index| u32::try_from(index).ok());
        }
    });
    if let Some(picked) = picked {
        slicer.selected = picked;
    }
}

impl EditorApp {
    /// The slicer, drawn on the image it is cutting.
    pub(super) fn slicer_panel(&mut self, ui: &mut egui::Ui) {
        let Some(slicer) = &mut self.slicer else {
            return;
        };
        let mut save = false;
        let mut close = false;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let (width, height) = slicer.size();
                // The image being cut is the subject of the panel while the slicer
                // is open, so it gets the identity card an entity would.
                egui::Frame::new()
                    .fill(color::RAISED)
                    .stroke(Stroke::new(1.0, color::LINE_SOFT))
                    .corner_radius(radius())
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .outer_margin(egui::Margin::symmetric(metric::GUTTER_EDGE, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                icons::SPRITE
                                    .outlined()
                                    .rich_text()
                                    .size(17.0)
                                    .color(color::FORGE),
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(slicer.name())
                                        .size(text::BODY)
                                        .color(color::TEXT),
                                );
                                ui.label(
                                    RichText::new(format!("{width} × {height} px"))
                                        .size(text::NOTE)
                                        .color(color::TEXT_FAINT),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if button::row_icon(
                                        ui,
                                        icons::CLOSE,
                                        Intent::Quiet,
                                        "Stop slicing and go back to the entity inspector",
                                    )
                                    .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });
                    });

                slice_preview(ui, slicer);
                ui.add_space(8.0);
                slice_grid(ui, slicer);
                ui.add_space(6.0);
                slice_names(ui, slicer);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(metric::GUTTER);
                    if button::labelled(
                        ui,
                        "Save slice",
                        Intent::Primary,
                        "Write the sheet beside the image so the project can use its sprites",
                    )
                    .clicked()
                    {
                        save = true;
                    }
                });
                if let Some(problem) = &slicer.problem {
                    panel::problem(ui, problem);
                }
                ui.add_space(8.0);
            });

        if save {
            let name = slicer.name();
            if slicer.save() {
                self.console.info(format!("Sliced {name}"));
                // The browser lists a texture's sprites from the sidecar, so a
                // save it did not notice would leave the new names invisible.
                self.refresh_project();
            } else if let Some(problem) = self.slicer.as_ref().and_then(|s| s.problem.clone()) {
                self.console.error(problem);
            }
        }
        if close {
            self.slicer = None;
        }
    }
}
