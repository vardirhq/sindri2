//! `sindri.tilemap`: the grid, the palette, and painting into it.

use std::path::Path;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use serde_json::Value;

use crate::tilemap::{self, PaletteSprite, TilemapTool, resize as resize_tilemap};
use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, text};
use crate::ui::widgets::{panel, property, section, toolbar};

use super::super::super::slicer_view::grid_side;
use super::super::rows::number_row;

/// The part of a tilemap that cannot be represented as independent JSON rows:
/// its dimensions, compact palette, and the brush that writes its cell array.
pub(super) fn tilemap_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    assets_root: Option<&Path>,
    tool: &mut TilemapTool,
) {
    let Ok(mut map) = tilemap::component(payload) else {
        panel::problem(
            ui,
            "This tilemap cannot be read; repair its stored fields first",
        );
        return;
    };

    section::group(ui, icons::TILEMAP, "Map");
    let mut columns = f64::from(map.columns);
    let mut rows = f64::from(map.rows);
    let mut resized = number_row(ui, "Columns", &mut columns, 10.0, true);
    resized |= number_row(ui, "Rows", &mut rows, 10.0, true);
    if resized && let Err(error) = resize_tilemap(payload, grid_side(columns), grid_side(rows)) {
        panel::problem(ui, &error);
    }
    // The resize above changes the payload this frame. Read it again so the
    // palette and the cell count below describe what the command will write.
    if let Ok(resized) = tilemap::component(payload) {
        map = resized;
    }

    // Painting takes the primary drag away from selection in the Scene view,
    // which is a mode the author needs to be able to see they are in.
    property::toggle(ui, "Brush", &mut tool.enabled, "Painting", "Off");
    section::caption(
        ui,
        if tool.enabled {
            "Primary drag paints. Middle or Shift-drag pans; secondary drag orbits."
        } else {
            "Enable painting, then choose a sprite or the eraser."
        },
    );

    tool.palette.ensure(assets_root, &map.texture);
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

    section::group(ui, icons::SPRITE, "Palette");
    tile_palette(ui, texture, &sprites, tool);
    if let Some(problem) = tool.palette.problem() {
        panel::problem(ui, problem);
    }
}

pub(super) fn tile_palette(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    sprites: &[PaletteSprite],
    tool: &mut TilemapTool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
        ui.add_space(metric::GUTTER);
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
        panel::note(
            ui,
            "Slice and name the map's texture to populate this palette.",
        );
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
    // The eraser is deliberately the same shape as a sprite: it is one more
    // thing the brush can be holding, not a different kind of control.
    let (rect, response) = ui.allocate_exact_size(Vec2::new(64.0, 64.0), Sense::click());
    let painter = ui.painter_at(rect);
    let border = if selected {
        color::FORGE
    } else if response.hovered() {
        color::LINE
    } else {
        color::LINE_SOFT
    };
    painter.rect_filled(
        rect,
        radius(),
        if selected { color::EMBER } else { color::WELL },
    );
    painter.rect_stroke(
        rect,
        radius(),
        Stroke::new(if selected { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let preview = Rect::from_min_max(
        rect.min + Vec2::new(6.0, 5.0),
        Pos2::new(rect.max.x - 6.0, rect.max.y - 17.0),
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
                crate::ui::theme::radius_tight(),
                Stroke::new(1.0, color::LINE_SOFT),
                StrokeKind::Inside,
            );
        }
        (None, _) => {
            painter.line_segment(
                [preview.left_top(), preview.right_bottom()],
                Stroke::new(2.0, color::DANGER),
            );
            painter.line_segment(
                [preview.right_top(), preview.left_bottom()],
                Stroke::new(2.0, color::DANGER),
            );
        }
    }
    let label = sprite.map_or("Erase", |sprite| sprite.name.as_str());
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 10.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(text::NOTE),
        if selected {
            color::FORGE_BRIGHT
        } else {
            color::TEXT_MUTED
        },
    );
    let _ = toolbar::chip;
    response.on_hover_text(label).clicked()
}
