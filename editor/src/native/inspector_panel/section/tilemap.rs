//! `sindri.tilemap`: the grid, the palette, and painting into it.

use std::path::Path;

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
};
use egui_material_icons::icons::{ICON_GRID_VIEW, ICON_IMAGE};
use serde_json::Value;

use crate::tilemap::{self, PaletteSprite, TilemapTool, resize as resize_tilemap};

use super::super::super::slicer_view::grid_side;
use super::super::super::{
    ACCENT_BRIGHT, ACCENT_SOFT, BORDER, BORDER_SUBTLE, PROBLEM, TEXT, TEXT_MUTED,
    theme::{FIELD_BG, section_header},
};
use super::super::rows::number_row;

/// The part of a tilemap that cannot be represented as independent JSON rows:
/// its dimensions, compact palette, and the brush that writes its cell array.
pub(super) fn tilemap_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    project_root: Option<&Path>,
    tool: &mut TilemapTool,
) {
    let Ok(mut map) = tilemap::component(payload) else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("This tilemap cannot be read; repair its stored fields first")
                    .size(10.0)
                    .color(PROBLEM),
            );
        });
        return;
    };

    section_header(ui, ICON_GRID_VIEW, "Map");
    let mut columns = f64::from(map.columns);
    let mut rows = f64::from(map.rows);
    let mut resized = number_row(ui, "Columns", &mut columns, 10.0, true);
    resized |= number_row(ui, "Rows", &mut rows, 10.0, true);
    if resized && let Err(error) = resize_tilemap(payload, grid_side(columns), grid_side(rows)) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(error).size(10.0).color(PROBLEM));
        });
    }
    // The resize above changes the payload this frame. Read it again so the
    // palette and the cell count below describe what the command will write.
    if let Ok(resized) = tilemap::component(payload) {
        map = resized;
    }

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let label = if tool.enabled {
            "Painting in Scene view"
        } else {
            "Paint in Scene view"
        };
        if ui.selectable_label(tool.enabled, label).clicked() {
            tool.enabled = !tool.enabled;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(if tool.enabled {
                "Primary drag paints. Middle or Shift-drag pans; secondary drag orbits."
            } else {
                "Enable painting, then choose a sprite or the eraser."
            })
            .size(9.0)
            .color(TEXT_MUTED),
        );
    });

    tool.palette.ensure(project_root, &map.texture);
    let texture = tool.palette.texture_id(ui.ctx());
    let mut sprites = tool.palette.sprites().to_vec();
    // A broken or changed sheet must not make a sprite already used by the map
    // impossible to select and replace. It stays visible as a named fallback,
    // without a thumbnail that would pretend it still resolves.
    for name in &map.palette {
        if !sprites.iter().any(|sprite| sprite.name == *name) {
            sprites.push(PaletteSprite {
                name: name.clone(),
                rect: None,
            });
        }
    }
    if !tool.erase
        && tool
            .sprite
            .as_ref()
            .is_none_or(|chosen| !sprites.iter().any(|sprite| sprite.name == *chosen))
    {
        tool.sprite = sprites.first().map(|sprite| sprite.name.clone());
    }

    section_header(ui, ICON_IMAGE, "Palette");
    tile_palette(ui, texture, &sprites, tool);
    if let Some(problem) = tool.palette.problem() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(problem).size(9.0).color(PROBLEM));
        });
    }
}

pub(super) fn tile_palette(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    sprites: &[PaletteSprite],
    tool: &mut TilemapTool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0);
        if palette_cell(ui, None, None, tool.erase) {
            tool.erase = true;
        }
        for sprite in sprites {
            let selected = !tool.erase && tool.sprite.as_deref() == Some(sprite.name.as_str());
            if palette_cell(ui, Some(sprite), texture, selected) {
                tool.erase = false;
                tool.sprite = Some(sprite.name.clone());
            }
        }
    });
    if sprites.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Slice and name the map's texture to populate this palette.")
                    .size(9.0)
                    .color(TEXT_MUTED),
            );
        });
    }
}

/// One compact palette swatch. Drawn directly so a named slice can preview a
/// UV rectangle without creating one egui texture per sprite.
pub(super) fn palette_cell(
    ui: &mut egui::Ui,
    sprite: Option<&PaletteSprite>,
    texture: Option<egui::TextureId>,
    selected: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(72.0, 72.0), Sense::click());
    let painter = ui.painter_at(rect);
    let border = if selected {
        ACCENT_BRIGHT
    } else if response.hovered() {
        TEXT_MUTED
    } else {
        BORDER
    };
    painter.rect_filled(rect, 4.0, if selected { ACCENT_SOFT } else { FIELD_BG });
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(if selected { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let preview = Rect::from_min_max(
        rect.min + Vec2::new(7.0, 6.0),
        Pos2::new(rect.max.x - 7.0, rect.max.y - 20.0),
    );
    match (sprite, texture) {
        (Some(sprite), Some(texture)) if sprite.rect.is_some() => {
            let [x, y, width, height] = sprite.rect.expect("checked above");
            painter.image(
                texture,
                preview,
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
                Color32::WHITE,
            );
        }
        (Some(_), _) => {
            painter.rect_stroke(
                preview,
                2.0,
                Stroke::new(1.0, BORDER_SUBTLE),
                StrokeKind::Inside,
            );
        }
        (None, _) => {
            painter.line_segment(
                [preview.left_top(), preview.right_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
            painter.line_segment(
                [preview.right_top(), preview.left_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
        }
    }
    let label = sprite.map_or("Erase", |sprite| sprite.name.as_str());
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 12.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        if selected { TEXT } else { TEXT_MUTED },
    );
    response.on_hover_text(label).clicked()
}
