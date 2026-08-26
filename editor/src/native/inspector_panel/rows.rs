//! The generic rows a value gets when no typed control claims it.
//!
//! By shape alone: a number is a drag, a string is a field, four numbers are a
//! row. `field` is where a value gets a control that knows what it *means*, and
//! everything it does not claim lands here.
//!
//! Every row here is one `property::Property`, so a component nobody wrote a
//! typed editor for still lines its labels and values up with the ones that
//! have.

use eframe::egui::{self, RichText};
use serde_json::Value;

use crate::inspector;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{property, vector};

use super::draft::Offer;

/// Whether a field holds what the author put there or what the schema did.
///
/// Threaded through the rows rather than worked out inside them, because only
/// the caller knows what the blank for this component looked like.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Authored {
    /// The stored value is the schema's default, untouched.
    #[default]
    Default,
    /// The scene set this one.
    Set,
}

impl Authored {
    pub(crate) fn of(modified: bool) -> Self {
        if modified { Self::Set } else { Self::Default }
    }

    const fn marked(self) -> bool {
        matches!(self, Self::Set)
    }
}

/// One field, drawn as whatever its stored shape deserves.
pub(crate) fn value_row(
    ui: &mut egui::Ui,
    key: &str,
    value: &mut Value,
    indent: f32,
    authored: Authored,
) {
    let label = inspector::humanize(key);
    match inspector::value_kind(value) {
        inspector::ValueKind::Number => {
            let mut number = value.as_f64().unwrap_or_default();
            // Integers stay integers, so editing a layer does not turn `3`
            // into `3.0` and change a scene byte for byte.
            let whole = value.is_i64() || value.is_u64();
            if number_row_marked(ui, &label, &mut number, indent, whole, authored) {
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
                ui.add_space(metric::GUTTER + indent);
                ui.label(
                    RichText::new(&label)
                        .size(text::LABEL)
                        .color(color::TEXT_FAINT),
                );
            });
            let Value::Object(nested) = value else {
                return;
            };
            for (key, value) in nested.iter_mut() {
                value_row(ui, key, value, indent + 10.0, Authored::Default);
            }
        }
        // Shown as stored and left alone. A text field over a tilemap's tiles
        // or a clip table is a way to break a scene, not a way to edit one —
        // but a row with no control and no explanation is the complaint this
        // panel keeps earning, so it says on hover why it is a readout.
        inspector::ValueKind::Opaque => {
            property::readout(
                ui,
                &label,
                &opaque_summary(value),
                Some(match value {
                    Value::Null => "Not set, and nothing here can say what it should be",
                    Value::Array(_) => {
                        "A list of values with no single control that could edit it safely"
                    }
                    _ => "Shown as it is stored: editing it as text could break the scene",
                }),
            );
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
    number_row_marked(ui, label, value, indent, whole, Authored::Default)
}

fn number_row_marked(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    indent: f32,
    whole: bool,
    authored: Authored,
) -> bool {
    let mut changed = false;
    property::Property::new(label)
        .indent(indent)
        .modified(authored.marked())
        .show(ui, |ui| {
            let drag = egui::DragValue::new(value).speed(if whole { 1.0 } else { 0.01 });
            let drag = if whole { drag.fixed_decimals(0) } else { drag };
            changed = ui
                .add_sized([property::value_width(ui), metric::CONTROL_HEIGHT], drag)
                .changed();
        });
    changed
}

pub(crate) fn bool_row(ui: &mut egui::Ui, label: &str, value: &mut bool, indent: f32) -> bool {
    let mut changed = false;
    property::Property::new(label)
        .indent(indent)
        .show(ui, |ui| {
            changed = ui.checkbox(value, "").changed();
        });
    changed
}

pub(crate) fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, indent: f32) -> bool {
    let mut changed = false;
    property::Property::new(label)
        .indent(indent)
        .show(ui, |ui| {
            changed = ui
                .add_sized(
                    [property::value_width(ui), metric::CONTROL_HEIGHT],
                    egui::TextEdit::singleline(value),
                )
                .changed();
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
    property::Property::new(label)
        .indent(indent)
        .show(ui, |ui| {
            let width = vector::axis_width(ui, values.len());
            for (index, value) in values.iter_mut().enumerate() {
                // Axis letters come from the field's own meaning — a tint's are
                // R, G, B, A — so the well is labelled with what it holds
                // rather than with the letter its position would imply.
                changed |= labelled_drag(
                    ui,
                    axes.get(index).map_or("", String::as_str),
                    index,
                    value,
                    width,
                );
            }
        });
    changed
}

/// One well of a multi-part value: its letter, and the number beside it.
fn labelled_drag(
    ui: &mut egui::Ui,
    letter: &str,
    index: usize,
    value: &mut f64,
    width: f32,
) -> bool {
    if letter.eq_ignore_ascii_case(vector::AXES[index.min(2)]) {
        return vector::axis(ui, index, value, width, 0.01, 3);
    }
    ui.add_sized(
        [width, metric::CONTROL_HEIGHT],
        egui::DragValue::new(value)
            .speed(0.01)
            .max_decimals(3)
            .prefix(format!("{letter} ")),
    )
    .changed()
}

/// The Add Component menu, offering only what can actually be added.
///
/// Absent entirely when there is nothing to add, rather than shown disabled: an
/// entity that already has everything is not a state worth drawing a greyed-out
/// control for.
pub(crate) fn add_component_button(ui: &mut egui::Ui, addable: &[Offer]) -> Option<String> {
    if addable.is_empty() {
        return None;
    }
    let mut chosen = None;
    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        // Words rather than a bare "+", because an inspector has several things
        // it could plausibly be adding. Given the panel's width so it reads as
        // the one thing left to do at the bottom of the list. The label is
        // plain text: the bundled Inter subset carries 192 glyphs, and a
        // decorative plus sign outside it draws as a missing-glyph box.
        let width = (ui.available_width() - 2.0 * metric::GUTTER).max(120.0);
        ui.allocate_ui(egui::vec2(width, metric::CONTROL_HEIGHT + 6.0), |ui| {
            ui.menu_button(
                RichText::new("Add Component")
                    .size(text::BODY)
                    .color(color::TEXT),
                |ui| {
                    ui.set_min_width(200.0);
                    for offer in addable {
                        // Listed either way. An entry that cannot be used says
                        // what to go and make; absent, it said nothing, and the
                        // menu was simply shorter than the documentation.
                        let entry = ui.add_enabled(
                            offer.withheld.is_none(),
                            egui::Button::new(&offer.metadata.display_name),
                        );
                        if let Some(reason) = offer.withheld {
                            entry.on_disabled_hover_text(reason);
                        } else if entry.clicked() {
                            chosen = Some(offer.metadata.type_name.clone());
                            ui.close();
                        }
                    }
                },
            );
        });
    });
    ui.add_space(10.0);
    chosen
}
