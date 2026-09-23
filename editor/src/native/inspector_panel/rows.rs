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

use sindri_core::{ComponentSchemaRegistry, FieldMeaning};

use crate::inspector;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{property, vector};

use super::field::{FieldAssets, asset_list, colour_row};
use super::list;

/// What the schema says about the component being drawn, where there is one.
///
/// `None` for values with no component behind them — a script's exported
/// properties, a profile's own data — which are drawn by shape alone, exactly
/// as they were before any of this existed.
#[derive(Clone, Copy)]
pub(crate) struct Described<'a> {
    pub(crate) registry: &'a ComponentSchemaRegistry,
    pub(crate) type_name: &'a str,
    pub(crate) assets: FieldAssets<'a>,
    /// The whole component as it stood when drawing began, for a field whose
    /// meaning depends on another part of it: a key reference reads the list
    /// it names. Only taken for components that declare keys, because it is a
    /// copy of the payload every frame.
    pub(crate) whole: Option<&'a Value>,
}

/// Where in a component a value sits, and what the component says about it.
///
/// The path is the whole dotted route to this value — `outline.color`,
/// `pieces.2.friction` — because that is what a meaning is keyed by. Before
/// this, rows knew only a field's own name, so a meaning below the top level
/// was recorded and never read.
#[derive(Clone, Copy)]
pub(crate) struct At<'a> {
    pub(crate) described: Option<Described<'a>>,
    pub(crate) path: &'a str,
}

impl<'a> At<'a> {
    /// The context for a value nested inside this one.
    pub(crate) const fn into(self, path: &'a str) -> Self {
        Self {
            described: self.described,
            path,
        }
    }

    /// A value with no component behind it.
    pub(crate) const fn loose() -> Self {
        Self {
            described: None,
            path: "",
        }
    }

    pub(crate) fn meaning(self) -> Option<&'a FieldMeaning> {
        let described = self.described?;
        described.registry.meaning(described.type_name, self.path)
    }

    /// What the schema says a value here looks like when nobody has said.
    pub(crate) fn exemplar(self) -> Option<&'a Value> {
        let described = self.described?;
        described.registry.exemplar(described.type_name, self.path)
    }
}

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

/// One field, drawn as whatever it means, or failing that as whatever it is.
///
/// Meaning is asked for first and at every depth, which is the whole of what
/// `at` adds: a colour inside a text component's outline is a colour, and so is
/// the one inside the third piece of a collider.
pub(crate) fn value_row(
    ui: &mut egui::Ui,
    at: At<'_>,
    key: &str,
    value: &mut Value,
    indent: f32,
    authored: Authored,
) {
    let label = inspector::humanize(key);
    if described_row(ui, at, &label, key, value, indent) {
        return;
    }
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
            // A section coordinate is whole numbers, and showing it as
            // `0.00` invited a fraction the engine would refuse.
            let whole = value
                .as_array()
                .is_some_and(|items| items.iter().all(|item| item.is_i64() || item.is_u64()));
            if numbers_row(ui, &label, &labels, &mut numbers, indent, whole) {
                #[allow(clippy::cast_possible_truncation)]
                let written = numbers
                    .into_iter()
                    .map(|number| {
                        if whole {
                            Value::from(number.round() as i64)
                        } else {
                            Value::from(number)
                        }
                    })
                    .collect();
                *value = Value::Array(written);
            }
        }
        inspector::ValueKind::Object => {
            let advanced = at
                .described
                .is_some_and(|described| inspector::is_advanced(described.type_name, key));
            if advanced {
                advanced_object(ui, at, &label, value, indent);
                return;
            }
            ui.horizontal(|ui| {
                ui.add_space(metric::GUTTER + indent);
                ui.label(
                    RichText::new(&label)
                        .size(text::LABEL)
                        .color(color::TEXT_FAINT),
                );
            });
            object_rows(ui, at, value, indent);
        }
        // Shown as stored and left alone. A text field over a tilemap's tiles
        // or a clip table is a way to break a scene, not a way to edit one —
        // but a row with no control and no explanation is the complaint this
        // panel keeps earning, so it says on hover why it is a readout.
        inspector::ValueKind::Opaque => {
            property::readout_indented(
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
                indent,
            );
        }
    }
}

/// One dotted path, extended by one step.
/// The rows of a nested object, under whatever heading drew it.
///
/// A field that decides what the rest of this object holds is drawn first, and
/// applied to the object rather than to itself: that is the edit it actually
/// is.
fn object_rows(ui: &mut egui::Ui, at: At<'_>, value: &mut Value, indent: f32) {
    let tag = variant_row(ui, at, value, indent + 10.0);
    let Value::Object(nested) = value else {
        return;
    };
    for (key, value) in nested.iter_mut() {
        if Some(key.as_str()) == tag.as_deref() {
            continue;
        }
        let nested_path = join(at.path, key);
        value_row(
            ui,
            at.into(&nested_path),
            key,
            value,
            indent + 10.0,
            Authored::Default,
        );
    }
}

/// An advanced object: folded away until asked for, and restorable.
///
/// Collapsed by default so the panel opens on the ordinary path through the
/// component, and open already when the scene has said something — a value
/// somebody authored should not be hidden behind a fold nothing marks. The
/// reset is what makes opening it safe: eight number boxes are easy to wander
/// away from and hard to put back by hand, and the schema already knows what
/// "nobody has said" looks like here.
fn advanced_object(ui: &mut egui::Ui, at: At<'_>, label: &str, value: &mut Value, indent: f32) {
    let identity = at.exemplar().cloned();
    let modified = identity.as_ref().is_some_and(|blank| blank != value);
    let heading = if modified {
        format!("{label} (edited)")
    } else {
        label.to_owned()
    };
    let tint = if modified {
        color::TEXT
    } else {
        color::TEXT_FAINT
    };
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER + indent);
        egui::CollapsingHeader::new(RichText::new(heading).size(text::LABEL).color(tint))
            .id_salt((at.path, "advanced"))
            .default_open(modified)
            .show(ui, |ui| {
                object_rows(ui, at, value, indent);
                let Some(identity) = identity else {
                    return;
                };
                ui.horizontal(|ui| {
                    ui.add_space(metric::GUTTER + indent + 10.0);
                    if ui
                        .add_enabled(modified, egui::Button::new("Reset"))
                        .on_hover_text("Put every channel back to what it is when nobody has said")
                        .clicked()
                    {
                        *value = identity;
                    }
                });
            });
    });
}

pub(crate) fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// The row a value gets because the component said what it is, if it said.
///
/// Returns whether it drew one. Everything here would otherwise be a number, a
/// string, or a readout: the schema is what turns it into a picker, a swatch,
/// or a list somebody can add to.
fn described_row(
    ui: &mut egui::Ui,
    at: At<'_>,
    label: &str,
    key: &str,
    value: &mut Value,
    indent: f32,
) -> bool {
    // A list is decided by the template rather than by the value, because an
    // empty list still has items it *would* hold, and that is exactly when
    // somebody needs to add the first one.
    if list::is_list(at) && value.is_array() {
        list::list_rows(ui, at, label, value, indent);
        return true;
    }
    let (Some(described), Some(meaning)) = (at.described, at.meaning()) else {
        return false;
    };
    match meaning {
        FieldMeaning::Asset(_) => {
            let Some(list) = asset_list(Some(meaning), described.assets) else {
                return false;
            };
            let pictures = super::field::pictures_for(Some(meaning), described.assets);
            super::field::asset_row(ui, at.path, key, value, list, pictures, indent);
            true
        }
        FieldMeaning::Colour if super::field::is_colour(Some(meaning), value) => {
            colour_row(ui, key, value);
            true
        }
        FieldMeaning::Range { min, max } if value.is_number() => {
            super::field::range_row(ui, label, value, (*min, *max), indent);
            true
        }
        FieldMeaning::OneOf(options) if value.is_number() => {
            super::field::one_of_row(ui, at.path, label, value, options, indent);
            true
        }
        FieldMeaning::Key => {
            super::keys::key_row(ui, at, label, value, indent);
            true
        }
        FieldMeaning::KeyOf(target) => {
            super::keys::key_of_row(ui, at, label, target, value, indent)
        }
        FieldMeaning::Block { set, or_key_of } => {
            super::blocks::block_row(ui, at, label, set, value, indent)
                || super::keys::key_of_row(ui, at, label, or_key_of, value, indent)
        }
        FieldMeaning::Choice(options) => {
            if described
                .registry
                .variants(described.type_name, at.path)
                .is_some()
            {
                // A tag that decides what its object holds is drawn by that
                // object, which is the only caller that can write the fields
                // the arriving variant needs. Reaching it here means somebody
                // drew the tag on its own, and the word alone would leave a
                // payload the schema refuses -- so it says that instead.
                property::readout(
                    ui,
                    label,
                    value.as_str().unwrap_or_default(),
                    Some("Choosing another changes what else this holds, so it is chosen above"),
                );
            } else if let Some(chosen) = super::field::choice_row(
                ui,
                at.path,
                key,
                value.as_str().unwrap_or_default(),
                options,
                indent,
            ) {
                *value = Value::String(chosen.to_owned());
            }
            true
        }
        _ => false,
    }
}

/// The field of this object that decides what else it holds, drawn as a picker.
///
/// Returns the tag it drew, so the object does not draw it a second time as an
/// ordinary string. `None` when the object has no such field, which is the
/// ordinary case.
///
/// This is where a variant switch has to happen: the arriving variant needs
/// fields written beside the tag, and the object is the only thing that holds
/// both. A collider piece switched from a box to a circle loses its half
/// extents and gains a radius here, in one edit, and the registry proved at
/// startup that the result is a piece the engine accepts.
fn variant_row(ui: &mut egui::Ui, at: At<'_>, value: &mut Value, indent: f32) -> Option<String> {
    let described = at.described?;
    let object = value.as_object()?;
    let (tag, options) = object.keys().find_map(|key| {
        let path = join(at.path, key);
        let FieldMeaning::Choice(options) =
            described.registry.meaning(described.type_name, &path)?
        else {
            return None;
        };
        described.registry.variants(described.type_name, &path)?;
        Some((key.clone(), options.clone()))
    })?;
    let path = join(at.path, &tag);
    let current = object
        .get(&tag)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if let Some(chosen) = super::field::choice_row(ui, &path, &tag, &current, &options, indent) {
        described
            .registry
            .switch_variant(described.type_name, &path, chosen, value);
    }
    Some(tag)
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
    whole: bool,
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
                    whole,
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
    whole: bool,
) -> bool {
    let (speed, decimals) = if whole { (0.1, 0) } else { (0.01, 3) };
    if letter.eq_ignore_ascii_case(vector::AXES[index.min(2)]) {
        return vector::axis(ui, index, value, width, speed, decimals);
    }
    ui.add_sized(
        [width, metric::CONTROL_HEIGHT],
        egui::DragValue::new(value)
            .speed(speed)
            .max_decimals(decimals)
            .prefix(format!("{letter} ")),
    )
    .changed()
}
