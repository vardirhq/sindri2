//! `sindri.tile_volume`: choose a block and build at an explicit height.

use std::path::Path;

use eframe::egui;
use serde_json::Value;

use crate::tile_volume::{self, TilePlacement, TileVolumeTool};
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
        ui.label("Target");
        ui.selectable_value(&mut tool.placement, TilePlacement::Surface, "Surface");
        ui.selectable_value(&mut tool.placement, TilePlacement::Level, "Level");
    });
    ui.horizontal(|ui| {
        ui.label("Level");
        ui.add_enabled(
            tool.placement == TilePlacement::Level,
            egui::DragValue::new(&mut tool.level).speed(0.1),
        );
    });
    ui.horizontal(|ui| {
        if ui.selectable_label(!tool.erase, "Place").clicked() {
            tool.erase = false;
        }
        if ui.selectable_label(tool.erase, "Remove").clicked() {
            tool.erase = true;
        }
    });
    let help = match tool.placement {
        TilePlacement::Surface => "Click a top face to stack or remove one block.",
        TilePlacement::Level => "Click or drag to paint the selected Z level.",
    };
    section::caption(ui, help);

    section::group(ui, icons::SPRITE, "Blocks");
    tool.palette.ensure(assets_root, &volume.tileset);
    let tiles = tool.palette.tiles().to_vec();
    if !tool.erase
        && tool
            .tile
            .as_ref()
            .is_none_or(|chosen| !tiles.contains(chosen))
    {
        tool.tile = tiles.first().cloned();
    }
    ui.horizontal_wrapped(|ui| {
        for tile in &tiles {
            if ui
                .selectable_label(
                    !tool.erase && tool.tile.as_deref() == Some(tile.as_str()),
                    tile,
                )
                .clicked()
            {
                tool.erase = false;
                tool.tile = Some(tile.clone());
            }
        }
    });
    if let Some(problem) = tool.palette.problem() {
        panel::problem(ui, problem);
    } else if tiles.is_empty() {
        panel::note(ui, "Add at least one block definition to this tile set.");
    }
}
