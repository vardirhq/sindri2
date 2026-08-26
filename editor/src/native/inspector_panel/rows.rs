//! The generic rows a value gets when no typed section claims it.

use eframe::egui::{self, Align, Layout, RichText};
use serde_json::Value;
use sindri_core::ComponentMetadata;

use crate::inspector;

use super::super::{TEXT, TEXT_FAINT, TEXT_MUTED, theme::property_label};

/// The rows of one payload, indented under its heading.
///
/// `skip_properties` keeps a script's authored values from appearing twice:
/// they are drawn above as typed fields, from what the script declared.
pub(crate) fn object_rows(
    ui: &mut egui::Ui,
    type_name: &str,
    payload: &mut Value,
    skip_properties: bool,
) {
    let Value::Object(fields) = payload else {
        return;
    };
    for (key, value) in fields.iter_mut() {
        if skip_properties && key == "properties" {
            continue;
        }
        if !inspector::applies(type_name, key) {
            continue;
        }
        value_row(ui, key, value, 10.0);
    }
}

/// One field, drawn as whatever its stored shape deserves.
pub(crate) fn value_row(ui: &mut egui::Ui, key: &str, value: &mut Value, indent: f32) {
    let label = inspector::humanize(key);
    match inspector::value_kind(value) {
        inspector::ValueKind::Number => {
            let mut number = value.as_f64().unwrap_or_default();
            // Integers stay integers, so editing a layer does not turn `3`
            // into `3.0` and change a scene byte for byte.
            let whole = value.is_i64() || value.is_u64();
            if number_row(ui, &label, &mut number, indent, whole) {
                *value = if whole {
                    #[allow(clippy::cast_possible_truncation)]
                    Value::from(number.round() as i64)
                } else {
                    Value::from(number)
                };
            }
        }
        inspector::ValueKind::Bool => {
            let mut flag = value.as_bool().unwrap_or_default();
            if bool_row(ui, &label, &mut flag, indent) {
                *value = Value::Bool(flag);
            }
        }
        inspector::ValueKind::Text => {
            let mut text = value.as_str().unwrap_or_default().to_owned();
            if text_row(ui, &label, &mut text, indent) {
                *value = Value::String(text);
            }
        }
        inspector::ValueKind::Numbers(len) => {
            let labels = inspector::axis_labels(key, len);
            let mut numbers: Vec<f64> = value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.as_f64().unwrap_or_default())
                        .collect()
                })
                .unwrap_or_default();
            if numbers_row(ui, &label, &labels, &mut numbers, indent) {
                *value = Value::Array(numbers.into_iter().map(Value::from).collect());
            }
        }
        inspector::ValueKind::Object => {
            ui.horizontal(|ui| {
                ui.add_space(indent);
                ui.label(RichText::new(&label).size(11.0).color(TEXT_MUTED));
            });
            let Value::Object(nested) = value else {
                return;
            };
            for (key, value) in nested.iter_mut() {
                value_row(ui, key, value, indent + 12.0);
            }
        }
        // Shown as stored and left alone. A text field over a tilemap's tiles
        // or a clip table is a way to break a scene, not a way to edit one.
        inspector::ValueKind::Opaque => {
            property_label(ui, &label, &opaque_summary(value));
        }
    }
}

/// What an uneditable value says about itself.
pub(crate) fn opaque_summary(value: &Value) -> String {
    match value {
        Value::Null => "not set".to_owned(),
        Value::Array(items) => format!("{} items", items.len()),
        other => other.to_string(),
    }
}

/// A labelled drag, reporting whether it moved.
pub(crate) fn number_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    indent: f32,
    whole: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            let drag = egui::DragValue::new(value).speed(if whole { 1.0 } else { 0.01 });
            let drag = if whole { drag.fixed_decimals(0) } else { drag };
            changed = ui.add(drag).changed();
        });
    });
    changed
}

pub(crate) fn bool_row(ui: &mut egui::Ui, label: &str, value: &mut bool, indent: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            changed = ui.checkbox(value, "").changed();
        });
    });
    changed
}

pub(crate) fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, indent: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            changed = ui
                .add(egui::TextEdit::singleline(value).desired_width(150.0))
                .changed();
        });
    });
    changed
}

/// A row of drags for a short numeric array, each under its own axis letter.
pub(crate) fn numbers_row(
    ui: &mut egui::Ui,
    label: &str,
    axes: &[String],
    values: &mut [f64],
    indent: f32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            for (index, value) in values.iter_mut().enumerate().rev() {
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .speed(0.01)
                            .prefix(format!("{} ", axes.get(index).map_or("", String::as_str))),
                    )
                    .changed();
            }
        });
    });
    changed
}

/// The Add Component menu, offering only what can actually be added.
///
/// Absent entirely when there is nothing to add, rather than shown disabled: an
/// entity that already has everything is not a state worth drawing a greyed-out
/// control for.
pub(crate) fn add_component_button(
    ui: &mut egui::Ui,
    addable: &[ComponentMetadata],
) -> Option<String> {
    if addable.is_empty() {
        return None;
    }
    let mut chosen = None;
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        // Words rather than a bare "+", because an inspector has several things
        // it could plausibly be adding. Drawn like the File and View menus,
        // which is what it is.
        ui.menu_button(
            RichText::new("Add Component").size(12.0).color(TEXT),
            |ui| {
                ui.set_min_width(170.0);
                for metadata in addable {
                    if ui.button(&metadata.display_name).clicked() {
                        chosen = Some(metadata.type_name.clone());
                        ui.close();
                    }
                }
            },
        );
    });
    chosen
}

/// Three drags for a vector, with the last one optionally taken away.
///
/// `lock_z` is what a transform that declares its Z locked looks like here: the
/// number is still shown, because what layer a thing is on is worth reading
/// even when it is not yours to change.
pub(crate) fn vector_row(
    ui: &mut egui::Ui,
    label: &str,
    values: &mut [f32; 3],
    lock_z: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            let locked = lock_z && index == 2;
            ui.label(
                RichText::new(["X", "Y", "Z"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_enabled_ui(!locked, |ui| {
                changed |= ui
                    .add_sized(
                        [48.0, 23.0],
                        egui::DragValue::new(value).speed(0.05).max_decimals(3),
                    )
                    .changed();
            });
        }
    });
    changed
}
