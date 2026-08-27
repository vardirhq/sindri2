//! Which control a field gets, once the panel knows more than its JSON shape.
//!
//! `rows` draws a value by what it is — a number, a string, four numbers. That
//! is the floor, and for a lot of fields it is also wrong: a projection is one
//! of two words rather than any word, a texture is a file in the project rather
//! than free text, and a tint is a colour rather than four drags between zero
//! and one that an author has to imagine.
//!
//! Everything here is a control that knows what the field means. Each answers
//! the same way — it edits the drawn payload, and the caller turns the
//! difference into a checked command — so a richer control is never a second
//! path into the world.

use eframe::egui::{self, Color32, RichText};
use serde_json::Value;

use crate::inspector::{self, choices, fields};
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::property;

use super::rows::{Authored, value_row};

/// What the panel knows about the project while drawing a field.
///
/// Grouped rather than passed one by one because a field asks for whichever
/// list its own meaning names, and the list only grows.
#[derive(Clone, Copy)]
pub(crate) struct FieldAssets<'a> {
    pub(crate) textures: &'a [String],
    pub(crate) fonts: &'a [String],
    pub(crate) scripts: &'a [String],
    pub(crate) audio: &'a [String],
}

/// The rows of one payload, indented under its heading.
///
/// Every field the component has, whether or not this one wrote it down: the
/// panel draws the registry's *field template* filled out with what is stored,
/// so two of one component show the same rows. What is written back is only
/// what changed, which `fields::merge_edits` decides.
///
/// The template, not the default payload. Those were the same value until a
/// type with no honest fresh instance — `sindri.ui.text` — turned out to have
/// no template either, and so drew two rows for a component with seven.
///
/// `skip_properties` keeps a script's authored values from appearing twice:
/// they are drawn above as typed fields, from what the script declared.
pub(crate) fn object_rows(
    ui: &mut egui::Ui,
    type_name: &str,
    payload: &mut Value,
    fields: Option<&Value>,
    assets: FieldAssets<'_>,
    skip_properties: bool,
) {
    // The blank is the one for the variant this payload is, not whichever
    // variant the registry's fresh component happens to be.
    let blank = fields.map(|fields| choices::blank_for(type_name, fields, payload));
    let mut drawn = fields::drawn_payload(blank.as_ref(), payload);
    for key in fields::ordered_keys(&drawn) {
        if (skip_properties && key == "properties") || !inspector::applies(type_name, &key) {
            continue;
        }
        // A choice can decide what else the component holds, so it is offered
        // the whole payload rather than one field of it.
        if let Some(options) = choices::choices(type_name, &key)
            && let Some(chosen) = choice_row(ui, &key, &drawn, &options)
        {
            choices::choose(type_name, &key, chosen, &mut drawn);
            continue;
        }
        if choices::choices(type_name, &key).is_some() {
            continue;
        }
        // Whether the scene set this field or the schema did. The panel already
        // has both, and "did I change this?" is the question an inspector is
        // asked most often.
        let authored = Authored::of(
            blank
                .as_ref()
                .and_then(|blank| blank.get(&key))
                .is_some_and(|default| drawn.get(&key) != Some(default)),
        );
        let Some(value) = drawn.get_mut(&key) else {
            continue;
        };
        if let Some(list) = asset_list(type_name, &key, assets) {
            asset_row(ui, &key, value, list);
            continue;
        }
        if is_colour(&key, value) {
            colour_row(ui, &key, value);
            continue;
        }
        value_row(ui, &key, value, 0.0, authored);
    }
    fields::merge_edits(blank.as_ref(), payload, &drawn);
}

/// The project list a field names, if it names one.
fn asset_list<'a>(type_name: &str, key: &str, assets: FieldAssets<'a>) -> Option<&'a [String]> {
    match (type_name, key) {
        (_, "texture") => Some(assets.textures),
        (_, "font") => Some(assets.fonts),
        (_, "clip") => Some(assets.audio),
        ("sindri.script", "source") => Some(assets.scripts),
        _ => None,
    }
}

fn is_colour(key: &str, value: &Value) -> bool {
    matches!(key, "tint" | "color" | "colour")
        && matches!(
            inspector::value_kind(value),
            inspector::ValueKind::Numbers(4)
        )
}

/// One of a few named values, chosen rather than typed.
///
/// Returns the chosen name only when it is a change, so a menu that opened and
/// closed does not rewrite a payload — which for a camera would mean rewriting
/// the fields its projection decides.
fn choice_row(
    ui: &mut egui::Ui,
    key: &str,
    payload: &Value,
    options: &[&'static str],
) -> Option<&'static str> {
    let current = payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    property::Property::new(&inspector::humanize(key)).show(ui, |ui| {
        egui::ComboBox::from_id_salt(("choice", key))
            .selected_text(
                RichText::new(inspector::humanize(&chosen))
                    .size(text::LABEL)
                    .color(color::TEXT_MUTED),
            )
            .width(property::picker_width(ui))
            .show_ui(ui, |ui| {
                for option in options {
                    ui.selectable_value(
                        &mut chosen,
                        (*option).to_owned(),
                        inspector::humanize(option),
                    );
                }
            });
    });
    (chosen != current)
        .then(|| options.iter().copied().find(|option| *option == chosen))
        .flatten()
}

/// A reference to something in the project, picked from what is there.
///
/// The field stays typeable, because a reference the project cannot currently
/// see is still a reference worth keeping: a texture arriving later, or a path
/// being fixed. What the picker adds is that the ordinary case — naming one of
/// the files sitting beside the scene — no longer means typing a path exactly.
///
/// A reference the project cannot see is marked rather than silently accepted:
/// the field turns to the editor's warning colour and says why on hover, which
/// is the difference between a typo found here and a scene that will not load.
fn asset_row(ui: &mut egui::Ui, key: &str, value: &mut Value, available: &[String]) {
    let mut typed = value.as_str().unwrap_or_default().to_owned();
    let known = typed.is_empty() || available.contains(&typed);
    let mut changed = false;
    let label = inspector::humanize(key);
    let mut row = property::Property::new(&label);
    if !known {
        row = row.tip("This reference is not in the project");
    }
    row.show(ui, |ui| {
        // The picker is a chevron and nothing else: the reference itself is in
        // the field beside it, and a combo repeating it would take half the row
        // to say the same thing twice.
        let picker = 12.0;
        let field_width =
            (property::value_width(ui) - picker - property::PICKER_FURNITURE - 4.0).max(56.0);
        if !known {
            ui.visuals_mut().extreme_bg_color = color::DANGER.gamma_multiply(0.12);
            ui.visuals_mut().widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0, color::DANGER.gamma_multiply(0.7));
        }
        changed |= ui
            .add_sized(
                [field_width, metric::CONTROL_HEIGHT],
                egui::TextEdit::singleline(&mut typed).hint_text("None"),
            )
            .changed();
        egui::ComboBox::from_id_salt(("asset", key))
            .selected_text("")
            .width(picker)
            .show_ui(ui, |ui| {
                ui.set_min_width(240.0);
                if available.is_empty() {
                    ui.label(
                        RichText::new("Nothing of this kind in the project")
                            .size(text::NOTE)
                            .color(color::TEXT_FAINT),
                    );
                }
                for option in available {
                    if ui.selectable_label(*option == typed, option).clicked() {
                        typed.clone_from(option);
                        changed = true;
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(if known {
                "Choose another from the project"
            } else {
                "This reference is not in the project; choose one that is"
            });
    });
    if changed {
        *value = Value::String(typed);
    }
}

/// A colour, as a colour.
///
/// Four drags between zero and one are the numbers a tint is stored as, and
/// nobody reads a colour that way. The swatch opens egui's own picker; the
/// numbers stay beside it, because a tint is also a number someone may want to
/// type exactly.
fn colour_row(ui: &mut egui::Ui, key: &str, value: &mut Value) {
    let mut rgba = [0.0_f32; 4];
    for (index, channel) in rgba.iter_mut().enumerate() {
        // A channel outside 0..1 is not a colour anything can show, and the
        // picker would clamp it silently on the way in. Clamping here means the
        // numbers beside the swatch agree with it.
        #[allow(clippy::cast_possible_truncation)]
        {
            *channel = value
                .get(index)
                .and_then(Value::as_f64)
                .unwrap_or(1.0)
                .clamp(0.0, 1.0) as f32;
        }
    }
    let mut changed = false;
    property::Property::new(&inspector::humanize(key)).show(ui, |ui| {
        let mut colour = Color32::from_rgba_unmultiplied(
            to_byte(rgba[0]),
            to_byte(rgba[1]),
            to_byte(rgba[2]),
            to_byte(rgba[3]),
        );
        // The swatch comes first: it is what the row is about, and the numbers
        // are how it is said exactly.
        if ui.color_edit_button_srgba(&mut colour).changed() {
            let [r, g, b, a] = colour.to_srgba_unmultiplied();
            rgba = [
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                f32::from(a) / 255.0,
            ];
            changed = true;
        }
        let width = ((property::value_width(ui) - 12.0) / 4.0).clamp(34.0, 64.0);
        for (index, channel) in rgba.iter_mut().enumerate() {
            changed |= ui
                .add_sized(
                    [width, metric::CONTROL_HEIGHT],
                    egui::DragValue::new(channel)
                        .speed(0.01)
                        .range(0.0..=1.0)
                        .max_decimals(3)
                        .prefix(format!("{} ", ["R", "G", "B", "A"][index])),
                )
                .changed();
        }
    });
    if changed {
        *value = Value::Array(rgba.iter().map(|channel| Value::from(*channel)).collect());
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}
