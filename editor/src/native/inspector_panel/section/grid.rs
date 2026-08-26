//! The grid components: navigation costs and what occupies a cell.

use std::collections::BTreeSet;

use eframe::egui::{self, Align, Layout, RichText};
use egui_material_icons::icons::ICON_GRID_4X4;
use serde_json::Value;
use sindri_core::World;

use crate::tilemap::{self};

use super::super::super::{
    PROBLEM, TEXT_MUTED,
    theme::{property_label, section_header},
};
use super::super::rows::numbers_row;

pub(crate) fn grid_choices(world: &World) -> Vec<(String, String)> {
    world
        .entities()
        .filter_map(|(_, data)| {
            data.components
                .contains_key(tilemap::TYPE_NAME)
                .then_some(())?;
            let id = data.source_id.as_ref()?.as_str().to_owned();
            let label = data.name.clone().unwrap_or_else(|| id.clone());
            Some((label, id))
        })
        .collect()
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub(super) fn grid_coord_row(ui: &mut egui::Ui, label: &str, value: &mut Value) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    if items.len() != 2 {
        return false;
    }
    let mut numbers = [
        items[0].as_i64().unwrap_or_default() as f64,
        items[1].as_i64().unwrap_or_default() as f64,
    ];
    let labels = ["X".to_owned(), "Y".to_owned()];
    if !numbers_row(ui, label, &labels, &mut numbers, 18.0) {
        return false;
    }
    *value = serde_json::json!([numbers[0].round() as i64, numbers[1].round() as i64]);
    true
}

pub(super) fn grid_navigation_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    grid_size: Option<(u32, u32)>,
) {
    section_header(ui, ICON_GRID_4X4, "Walls");
    let Some(walls) = payload.get_mut("walls").and_then(Value::as_array_mut) else {
        property_label(ui, "Walls", "stored value is not a wall list");
        return;
    };
    let mut remove = None;
    for (index, wall) in walls.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Wall {}", index + 1))
                    .size(10.0)
                    .color(TEXT_MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(7.0);
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        });
        if let Some(coord) = wall.get_mut("first") {
            grid_coord_row(ui, "First", coord);
        }
        if let Some(coord) = wall.get_mut("second") {
            grid_coord_row(ui, "Second", coord);
        }
        let first = wall.get("first").and_then(Value::as_array);
        let second = wall.get("second").and_then(Value::as_array);
        let valid = first.zip(second).is_some_and(|(first, second)| {
            if first.len() != 2 || second.len() != 2 {
                return false;
            }
            let (Some(ax), Some(ay), Some(bx), Some(by)) = (
                first[0].as_i64(),
                first[1].as_i64(),
                second[0].as_i64(),
                second[1].as_i64(),
            ) else {
                return false;
            };
            let adjacent = (ax - bx).abs() + (ay - by).abs() == 1;
            let inside = grid_size.is_none_or(|(columns, rows)| {
                let inside = |x: i64, y: i64| {
                    x >= 0 && y >= 0 && x < i64::from(columns) && y < i64::from(rows)
                };
                inside(ax, ay) && inside(bx, by)
            });
            adjacent && inside
        });
        if !valid {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(18.0);
                ui.label(
                    RichText::new("Wall endpoints must be adjacent cells inside the tilemap.")
                        .size(9.0)
                        .color(PROBLEM),
                );
            });
        }
    }
    if let Some(index) = remove {
        walls.remove(index);
    }
    let can_add = grid_size.is_some_and(|(columns, rows)| columns > 1 || rows > 1);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui
            .add_enabled(can_add, egui::Button::new("Add wall"))
            .clicked()
        {
            let second = if grid_size.is_some_and(|(columns, _)| columns > 1) {
                [1, 0]
            } else {
                [0, 1]
            };
            walls.push(serde_json::json!({ "first": [0, 0], "second": second }));
        }
    });
}

pub(super) fn grid_occupant_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    grids: &[(String, String)],
) {
    section_header(ui, ICON_GRID_4X4, "Occupancy");
    let current = payload
        .get("grid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Grid").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("grid-occupant-grid")
                .selected_text(
                    grids
                        .iter()
                        .find(|(_, id)| *id == chosen)
                        .map_or(chosen.as_str(), |(label, _)| label.as_str()),
                )
                .width(170.0)
                .show_ui(ui, |ui| {
                    for (label, id) in grids {
                        ui.selectable_value(&mut chosen, id.clone(), label);
                    }
                });
        });
    });
    if chosen != current && !chosen.is_empty() {
        payload["grid"] = Value::String(chosen);
    }
    if grids.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Add a tilemap before authoring an occupant.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
    }

    let Some(footprint) = payload.get_mut("footprint").and_then(Value::as_array_mut) else {
        property_label(ui, "Footprint", "stored value is not a cell list");
        return;
    };
    let may_remove = footprint.len() > 1;
    let mut remove = None;
    for (index, cell) in footprint.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Cell {}", index + 1))
                    .size(10.0)
                    .color(TEXT_MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(7.0);
                if ui
                    .add_enabled(may_remove, egui::Button::new("Remove"))
                    .clicked()
                {
                    remove = Some(index);
                }
            });
        });
        grid_coord_row(ui, "Offset", cell);
    }
    if let Some(index) = remove {
        footprint.remove(index);
    }
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui.button("Add cell").clicked() {
            let next_x = footprint
                .iter()
                .filter_map(Value::as_array)
                .filter_map(|cell| cell.first()?.as_i64())
                .max()
                .unwrap_or(-1)
                + 1;
            footprint.push(serde_json::json!([next_x, 0]));
        }
    });
    let mut seen = BTreeSet::new();
    let duplicate = footprint.iter().filter_map(Value::as_array).any(|cell| {
        let key = (
            cell.first().and_then(Value::as_i64),
            cell.get(1).and_then(Value::as_i64),
        );
        !seen.insert(key)
    });
    if duplicate {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Footprint cells must be unique offsets.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
    }
}
