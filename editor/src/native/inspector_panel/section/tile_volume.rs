//! `sindri.tile_volume`: choose a block and build at an explicit height.

use std::path::Path;

use eframe::egui;
use serde_json::Value;
use sindri_core::TileSetDocument;

use crate::tile_volume::{self, TileVolumeTool};
use crate::ui::icons;
use crate::ui::widgets::{panel, property, section};

pub(super) fn tile_volume_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    assets_root: Option<&Path>,
    tool: &mut TileVolumeTool,
) {
    let Ok(volume) = tile_volume::component(payload) else {
        panel::problem(
            ui,
            "This tile volume cannot be read; repair its stored fields first",
        );
        return;
    };
    section::group(ui, icons::TILEMAP, "Build");
    property::toggle(ui, "Brush", &mut tool.enabled, "Building", "Off");
    ui.horizontal(|ui| {
        ui.label("Level");
        ui.add(egui::DragValue::new(&mut tool.level).speed(0.1));
    });
    ui.horizontal(|ui| {
        if ui.selectable_label(!tool.erase, "Place").clicked() {
            tool.erase = false;
        }
        if ui.selectable_label(tool.erase, "Remove").clicked() {
            tool.erase = true;
        }
    });
    section::caption(
        ui,
        "Primary click or drag edits this Z level. Middle or Shift-drag pans.",
    );

    section::group(ui, icons::SPRITE, "Blocks");
    match tile_ids(assets_root, &volume.tileset) {
        Ok(tiles) => {
            if !tool.erase
                && tool
                    .tile
                    .as_ref()
                    .is_none_or(|chosen| !tiles.contains(chosen))
            {
                tool.tile = tiles.first().cloned();
            }
            ui.horizontal_wrapped(|ui| {
                for tile in tiles {
                    if ui
                        .selectable_label(!tool.erase && tool.tile.as_deref() == Some(&tile), &tile)
                        .clicked()
                    {
                        tool.erase = false;
                        tool.tile = Some(tile);
                    }
                }
            });
        }
        Err(error) => panel::problem(ui, &error),
    }
}

fn tile_ids(root: Option<&Path>, reference: &str) -> Result<Vec<String>, String> {
    let root = root.ok_or_else(|| "Save the scene before loading its tile set".to_owned())?;
    let text = std::fs::read_to_string(root.join(reference)).map_err(|error| error.to_string())?;
    let set = TileSetDocument::from_json(&text).map_err(|error| error.to_string())?;
    Ok(set.tiles.keys().cloned().collect())
}
