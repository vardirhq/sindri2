//! Structured authoring for reusable profile assets.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText};
use serde_json::{Map, Value};

use crate::native::inspector_panel::rows::{Authored, value_row};
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{button, button::Intent, panel};

use super::EditorApp;

impl EditorApp {
    pub(super) fn profile_panel(&mut self, ui: &mut egui::Ui) {
        let mut save = false;
        let Some(profile) = self.profile.as_mut() else {
            return;
        };
        panel::body(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(
                        profile
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
                        if profile.dirty { "Save*" } else { "Save" },
                        Intent::Primary,
                        "Write this profile to disk",
                    )
                    .clicked();
                });
            });
        });
        if let Some(error) = &profile.error {
            panel::problem(ui, error);
            return;
        }
        let Some(document) = profile.document.as_mut() else {
            return;
        };
        let mut changed = false;
        changed |= text_field(ui, "Name", &mut document.name);
        changed |= text_field(ui, "Type", &mut document.profile_type);
        ui.add_space(metric::GAP);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                changed |= object_editor(ui, &mut document.values, "values");
            });
        profile.dirty |= changed;
        if save
            && let Err(error) = profile.save()
        {
            profile.error = Some(error);
        }
    }
}

fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let before = value.clone();
    crate::native::inspector_panel::rows::text_row(ui, label, value, 0.0);
    *value != before
}

fn object_editor(ui: &mut egui::Ui, values: &mut BTreeMap<String, Value>, id: &str) -> bool {
    let mut map: Map<String, Value> = std::mem::take(values).into_iter().collect();
    let changed = map_editor(ui, &mut map, id);
    *values = map.into_iter().collect();
    changed
}

fn map_editor(ui: &mut egui::Ui, values: &mut Map<String, Value>, id: &str) -> bool {
    let mut changed = false;
    let keys: Vec<String> = values.keys().cloned().collect();
    let mut remove = None;
    let mut rename = None;
    for key in keys {
        ui.push_id((id, &key), |ui| {
            let draft_id = ui.id().with("profile-key-draft");
            let mut edited_key = ui.data_mut(|data| {
                data.get_temp_mut_or(draft_id, key.clone()).clone()
            });
            let mut commit_key = false;
            ui.horizontal(|ui| {
                let response = ui
                    .add_sized(
                        [150.0, metric::CONTROL_HEIGHT],
                        egui::TextEdit::singleline(&mut edited_key),
                    );
                if response.changed() {
                    ui.data_mut(|data| data.insert_temp(draft_id, edited_key.clone()));
                }
                commit_key = response.lost_focus();
                if button::icon(ui, icons::REMOVE, false, "Remove this value").clicked() {
                    remove = Some(key.clone());
                }
            });
            if commit_key {
                let edited_key = edited_key.trim().to_owned();
                if edited_key != key
                    && !edited_key.is_empty()
                    && !values.contains_key(&edited_key)
                {
                    rename = Some((key.clone(), edited_key));
                }
                ui.data_mut(|data| data.remove_temp::<String>(draft_id));
            }
            if let Some(value) = values.get_mut(&key) {
                changed |= value_editor(ui, value, &key);
            }
            ui.add_space(3.0);
        });
    }
    if let Some(key) = remove {
        values.remove(&key);
        changed = true;
    }
    if let Some((old, new)) = rename
        && let Some(value) = values.remove(&old)
    {
        values.insert(new, value);
        changed = true;
    }
    ui.horizontal_wrapped(|ui| {
        ui.add_space(metric::GUTTER);
        changed |= add_value(ui, values, "Number", Value::from(0.0));
        changed |= add_value(ui, values, "Text", Value::String(String::new()));
        changed |= add_value(ui, values, "Flag", Value::Bool(false));
        changed |= add_value(ui, values, "List", Value::Array(Vec::new()));
        changed |= add_value(ui, values, "Group", Value::Object(Map::new()));
    });
    changed
}

fn add_value(
    ui: &mut egui::Ui,
    values: &mut Map<String, Value>,
    label: &str,
    value: Value,
) -> bool {
    if !button::labelled(ui, label, Intent::Quiet, &format!("Add a {label} value")).clicked() {
        return false;
    }
    let mut index = 1;
    let mut key = "value".to_owned();
    while values.contains_key(&key) {
        index += 1;
        key = format!("value_{index}");
    }
    values.insert(key, value);
    true
}

fn value_editor(ui: &mut egui::Ui, value: &mut Value, key: &str) -> bool {
    match value {
        Value::Array(items) => array_editor(ui, items, key),
        Value::Object(values) => {
            let mut changed = false;
            egui::CollapsingHeader::new(format!("{} fields", values.len()))
                .id_salt((key, "object"))
                .default_open(false)
                .show(ui, |ui| changed |= map_editor(ui, values, key));
            changed
        }
        _ => {
            let before = value.clone();
            value_row(ui, key, value, 8.0, Authored::Set);
            *value != before
        }
    }
}

fn array_editor(ui: &mut egui::Ui, items: &mut Vec<Value>, key: &str) -> bool {
    let mut changed = false;
    let mut remove = None;
    let mut duplicate = None;
    egui::CollapsingHeader::new(format!("{} items", items.len()))
        .id_salt((key, "array"))
        .default_open(false)
        .show(ui, |ui| {
            for (index, item) in items.iter_mut().enumerate() {
                ui.push_id((key, index), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("#{index}")).color(color::TEXT_FAINT));
                        if button::icon(ui, icons::DUPLICATE, false, "Duplicate this item")
                            .clicked()
                        {
                            duplicate = Some(index);
                        }
                        if button::icon(ui, icons::REMOVE, false, "Remove this item").clicked() {
                            remove = Some(index);
                        }
                    });
                    changed |= value_editor(ui, item, &format!("{key}_{index}"));
                });
            }
            if button::labelled(ui, "Add item", Intent::Quiet, "Append a new list item").clicked() {
                let item = items
                    .last()
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new()));
                items.push(item);
                changed = true;
            }
        });
    if let Some(index) = duplicate {
        let copy = items[index].clone();
        items.insert(index + 1, copy);
        changed = true;
    }
    if let Some(index) = remove {
        items.remove(index);
        changed = true;
    }
    changed
}
