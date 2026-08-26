//! The grid components: navigation costs and what occupies a cell.

use std::collections::BTreeSet;

use eframe::egui;
use serde_json::Value;
use sindri_core::World;

use crate::tilemap::{self};
use crate::ui::icons;
use crate::ui::theme::metric;
use crate::ui::widgets::{
    button::{self, Intent},
    panel, property, section,
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
    if !numbers_row(ui, label, &labels, &mut numbers, 10.0) {
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
    section::group(ui, icons::GRID, "Walls");
    let Some(walls) = payload.get_mut("walls").and_then(Value::as_array_mut) else {
        property::readout(ui, "Walls", "stored value is not a wall list", None);
        return;
    };
    let mut remove = None;
    for (index, wall) in walls.iter_mut().enumerate() {
        property::Property::new(&format!("Wall {}", index + 1)).show(ui, |ui| {
            if button::row_icon(ui, icons::REMOVE, Intent::Danger, "Remove this wall").clicked() {
                remove = Some(index);
            }
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
            panel::problem(
                ui,
                "Wall endpoints must be adjacent cells inside the tilemap.",
            );
        }
    }
    if let Some(index) = remove {
        walls.remove(index);
    }
    let can_add = grid_size.is_some_and(|(columns, rows)| columns > 1 || rows > 1);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        if ui
            .add_enabled_ui(can_add, |ui| {
                button::labelled(
                    ui,
                    "Add wall",
                    Intent::Normal,
                    "Block movement between two cells",
                )
            })
            .inner
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
    section::group(ui, icons::GRID, "Occupancy");
    let current = payload
        .get("grid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    property::Property::new("Grid").show(ui, |ui| {
        egui::ComboBox::from_id_salt("grid-occupant-grid")
            .selected_text(
                grids
                    .iter()
                    .find(|(_, id)| *id == chosen)
                    .map_or(chosen.as_str(), |(label, _)| label.as_str()),
            )
            .width(property::picker_width(ui))
            .show_ui(ui, |ui| {
                for (label, id) in grids {
                    ui.selectable_value(&mut chosen, id.clone(), label);
                }
            });
    });
    if chosen != current && !chosen.is_empty() {
        payload["grid"] = Value::String(chosen);
    }
    if grids.is_empty() {
        panel::problem(ui, "Add a tilemap before authoring an occupant.");
    }

    let Some(footprint) = payload.get_mut("footprint").and_then(Value::as_array_mut) else {
        property::readout(ui, "Footprint", "stored value is not a cell list", None);
        return;
    };
    let may_remove = footprint.len() > 1;
    let mut remove = None;
    for (index, cell) in footprint.iter_mut().enumerate() {
        property::Property::new(&format!("Cell {}", index + 1)).show(ui, |ui| {
            if ui
                .add_enabled_ui(may_remove, |ui| {
                    button::row_icon(ui, icons::REMOVE, Intent::Danger, "Remove this cell")
                })
                .inner
                .clicked()
            {
                remove = Some(index);
            }
        });
        grid_coord_row(ui, "Offset", cell);
    }
    if let Some(index) = remove {
        footprint.remove(index);
    }
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        if button::labelled(
            ui,
            "Add cell",
            Intent::Normal,
            "Give this occupant another cell of footprint",
        )
        .clicked()
        {
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
        panel::problem(ui, "Footprint cells must be unique offsets.");
    }
}
