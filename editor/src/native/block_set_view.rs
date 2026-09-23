//! A block set in the inspector: its blocks as cubes, and the one chosen laid
//! out as its faces and what it does.

use eframe::egui::{self, RichText, Sense, Vec2};
use serde_json::Value;
use sindri_core::{TileDefinition, TileFace, TileFaceVisual};

use crate::block_set::face;
use crate::inspector;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{button, button::Intent, cube, panel, property};

use super::EditorApp;
use super::inspector_panel::blocks::block_faces;
use super::inspector_panel::field::asset_row;
use super::thumbnails::Pictures;
use super::thumbnails::drawable_textures;

/// The six faces, in the order an author thinks of a block: what is on top,
/// what is underneath, then its sides.
const FACES: [(TileFace, &str); 6] = [
    (TileFace::Top, "Top"),
    (TileFace::Bottom, "Bottom"),
    (TileFace::North, "North"),
    (TileFace::South, "South"),
    (TileFace::East, "East"),
    (TileFace::West, "West"),
];

impl EditorApp {
    pub(super) fn block_set_panel(&mut self, ui: &mut egui::Ui) {
        let mut references = drawable_textures(&self.project, self.textures.bindings());
        if let Some(document) = self
            .block_set
            .as_ref()
            .and_then(|editor| editor.document.as_ref())
        {
            references.extend(super::inspector_panel::blocks::block_sprites(document));
        }
        // The set's art may be nothing the scene draws with, so it is asked
        // for here; otherwise every block would be a grey cube.
        let shown: std::collections::BTreeSet<String> = self
            .block_set
            .as_ref()
            .and_then(|editor| editor.document.as_ref())
            .map(|document| {
                document
                    .tiles
                    .values()
                    .flat_map(|block| block.faces.iter().map(|(_, visual)| visual.sprite.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if self.textures.pin(shown) {
            let notes = self.textures.request(&self.world, &mut self.renderers.text);
            self.record_texture_notes(notes);
        }
        self.thumbnails
            .refresh(&self.render_state, &self.textures, &references);
        let pictures = self.thumbnails.pictures().clone();
        let Some(editor) = self.block_set.as_mut() else {
            return;
        };
        let save = header(ui, editor);
        if let Some(error) = &editor.error {
            panel::problem(ui, error);
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if button::labelled(
                        ui,
                        "Add block",
                        Intent::Normal,
                        "A new block with placeholder art",
                    )
                    .clicked()
                    {
                        editor.add_block();
                    }
                    let chosen = editor.selected.is_some();
                    if ui
                        .add_enabled(chosen, egui::Button::new("Duplicate"))
                        .clicked()
                    {
                        editor.duplicate_selected();
                    }
                    if ui
                        .add_enabled(chosen, egui::Button::new("Delete"))
                        .clicked()
                    {
                        editor.remove_selected();
                    }
                });
                ui.add_space(metric::GAP);
                if let Some(document) = editor.document.as_ref() {
                    let mut picked = None;
                    ui.horizontal_wrapped(|ui| {
                        for (name, block) in &document.tiles {
                            let current = editor.selected.as_deref() == Some(name.as_str());
                            if block_tile(ui, name, block, &pictures, current).clicked() {
                                picked = Some(name.clone());
                            }
                        }
                    });
                    if picked.is_some() {
                        editor.selected = picked;
                    }
                }
                ui.add_space(metric::GAP);
                name_field(ui, editor);
                let Some(block) = editor.selected_block_mut() else {
                    return;
                };
                let changed = block_fields(ui, block, &references, &pictures);
                editor.dirty |= changed;
            });
        if save && let Err(error) = editor.save() {
            editor.error = Some(error);
        }
    }
}

/// The chosen block's name, renamed when the field is left.
///
/// Typed into a draft rather than renamed on every keystroke: renaming
/// `clay` to `stones` passes through `stone`, which another block may
/// already be, and refusing that halfway would leave the name impossible
/// to type. A name that is taken or empty when the field is left puts the
/// old one back.
fn name_field(ui: &mut egui::Ui, editor: &mut crate::block_set::BlockSetEditor) {
    let Some(selected) = editor.selected.clone() else {
        return;
    };
    if editor.naming.as_ref().is_none_or(|(of, _)| *of != selected) {
        editor.naming = Some((selected.clone(), selected));
    }
    let mut left = false;
    if let Some((_, draft)) = editor.naming.as_mut() {
        property::Property::new("Name").show(ui, |ui| {
            left = ui
                .add_sized(
                    [property::value_width(ui), metric::CONTROL_HEIGHT],
                    egui::TextEdit::singleline(draft),
                )
                .on_hover_text("What voxel worlds and tile volumes call this block")
                .lost_focus();
        });
    }
    if left {
        let wanted = editor
            .naming
            .take()
            .map(|(_, draft)| draft)
            .unwrap_or_default();
        editor.rename_selected(&wanted);
    }
}

/// One block in the grid: its cube, and its name under it.
fn block_tile(
    ui: &mut egui::Ui,
    name: &str,
    block: &TileDefinition,
    pictures: &Pictures,
    current: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(64.0, 72.0), Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if current {
        color::EMBER
    } else if response.hovered() {
        color::RAISED
    } else {
        egui::Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 4.0, fill);
    let (top, side) = block_faces(block, pictures);
    cube::paint(
        &painter,
        egui::Rect::from_center_size(rect.center_top() + egui::vec2(0.0, 26.0), Vec2::splat(40.0)),
        top,
        side,
    );
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        inspector::humanize(name),
        egui::FontId::proportional(text::NOTE),
        if current {
            color::TEXT
        } else {
            color::TEXT_MUTED
        },
    );
    response.on_hover_text(name)
}

/// The chosen block's faces and what it does. Returns whether anything changed.
fn block_fields(
    ui: &mut egui::Ui,
    block: &mut TileDefinition,
    textures: &[String],
    pictures: &Pictures,
) -> bool {
    let before = block.clone();
    panel::note(ui, "A face left empty shows the face opposite it.");
    for (which, label) in FACES {
        let slot = face_slot(block, which);
        let mut value = Value::String(
            slot.as_ref()
                .map(|visual| visual.sprite.clone())
                .unwrap_or_default(),
        );
        asset_row(ui, label, label, &mut value, textures, Some(pictures), 0.0);
        let sprite = value.as_str().unwrap_or_default().trim();
        let wanted = (!sprite.is_empty()).then(|| {
            slot.clone().map_or_else(
                || face(sprite),
                |mut kept| {
                    sprite.clone_into(&mut kept.sprite);
                    kept
                },
            )
        });
        if wanted != *slot {
            *slot = wanted;
        }
    }
    ui.add_space(metric::GAP);
    flag(
        ui,
        "Hides neighbours",
        &mut block.occludes,
        "Whether a block beside it skips the face they share. Off for glass, leaves and water.",
    );
    flag(
        ui,
        "Supports",
        &mut block.supports,
        "Whether things can rest on top of it",
    );
    flag(
        ui,
        "Walkable",
        &mut block.walkable,
        "Whether a walker can stand on it",
    );
    let mut tags = block.tags.join(", ");
    let before_tags = tags.clone();
    crate::native::inspector_panel::rows::text_row(ui, "Tags", &mut tags, 0.0);
    if tags != before_tags {
        block.tags = tags
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect();
    }
    property::Property::new("Height").show(ui, |ui| {
        ui.add(
            egui::DragValue::new(&mut block.height)
                .range(0.05..=1.0)
                .speed(0.01)
                .fixed_decimals(2),
        )
        .on_hover_text(
            "How much of its cell the block fills, from the floor up. A half is a slab.",
        );
    });
    *block != before
}

fn face_slot(block: &mut TileDefinition, which: TileFace) -> &mut Option<TileFaceVisual> {
    let faces = &mut block.faces;
    match which {
        TileFace::Top => &mut faces.top,
        TileFace::Bottom => &mut faces.bottom,
        TileFace::North => &mut faces.north,
        TileFace::South => &mut faces.south,
        TileFace::East => &mut faces.east,
        TileFace::West => &mut faces.west,
    }
}

fn flag(ui: &mut egui::Ui, label: &str, value: &mut bool, tip: &str) {
    property::Property::new(label).show(ui, |ui| {
        ui.checkbox(value, "").on_hover_text(tip);
    });
}

/// The set's file name, and Save. Returns whether Save was pressed.
fn header(ui: &mut egui::Ui, editor: &crate::block_set::BlockSetEditor) -> bool {
    let mut save = false;
    panel::body(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    editor
                        .path()
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                )
                .size(text::BODY)
                .color(color::TEXT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                save = button::labelled(
                    ui,
                    if editor.dirty { "Save*" } else { "Save" },
                    Intent::Primary,
                    "Write this block set to disk",
                )
                .clicked();
            });
        });
    });
    save
}
