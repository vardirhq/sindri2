//! Items of a list that other fields name by key.
//!
//! A voxel world's generator names its materials by ID, and an ID nothing
//! defines is a world the engine refuses. The list's own buttons did not know
//! that: Add copied the schema's exemplar, whose ID every world already used,
//! and Remove took a material the terrain was made of. Each left a component
//! the engine would not draw.
//!
//! The component says which field is a key and which fields refer to one
//! (`FieldMeaning::Key` and `FieldMeaning::KeyOf`), so the panel can number a
//! new item, refuse to remove one still named, and offer only keys that exist.

use eframe::egui::{self, RichText};
use serde_json::Value;
use sindri_core::{ComponentSchemaRegistry, FieldMeaning};

use crate::inspector;
use crate::ui::theme::{color, text};
use crate::ui::widgets::cube::{self, Picture};
use crate::ui::widgets::property;

use super::super::thumbnails::Pictures;
use super::rows::{At, join};

/// Whether drawing this component needs the whole of it at hand: a reference
/// is checked against the list it names, which is somewhere else in the
/// component than the field being drawn.
pub(crate) fn has_keys(registry: &ComponentSchemaRegistry, type_name: &str) -> bool {
    registry.meanings(type_name).any(|(_, meaning)| {
        matches!(
            meaning,
            FieldMeaning::Key | FieldMeaning::KeyOf(_) | FieldMeaning::Block { .. }
        )
    })
}

/// The fields of a list item that are its keys.
fn key_fields(at: At<'_>, item: &Value, index: usize) -> Vec<String> {
    let Some(fields) = item.as_object() else {
        return Vec::new();
    };
    let item_path = join(at.path, &index.to_string());
    fields
        .keys()
        .filter(|key| {
            let path = join(&item_path, key);
            at.into(&path).meaning() == Some(&FieldMeaning::Key)
        })
        .cloned()
        .collect()
}

/// One more than the largest key the other items hold, and never below one.
///
/// Zero is left alone because a voxel ID of zero is air, and the rule costs
/// nothing for a list whose keys mean something else.
fn next_key(items: &[Value], field: &str, skip: usize) -> u64 {
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != skip)
        .filter_map(|(_, item)| item.get(field).and_then(Value::as_u64))
        .max()
        .map_or(1, |largest| largest + 1)
}

/// Gives the item at `index` keys no other item holds, in each of `fields`.
fn assign_next_keys(items: &mut [Value], index: usize, fields: &[String]) {
    for field in fields {
        let key = next_key(items, field, index);
        if let Some(slot) = items.get_mut(index).and_then(|item| item.get_mut(field)) {
            *slot = Value::from(key);
        }
    }
}

/// Numbers a freshly added item, which arrived as a copy of the exemplar and
/// so as a duplicate of whichever item already has the exemplar's key.
pub(crate) fn number_new_item(at: At<'_>, items: &mut [Value], index: usize) {
    let Some(item) = items.get(index) else {
        return;
    };
    let fields = key_fields(at, item, index);
    assign_next_keys(items, index, &fields);
}

/// What still names this item, as the labels of the naming fields.
///
/// Empty when nothing does, which is when it can be removed.
pub(crate) fn named_by(at: At<'_>, item: &Value, index: usize) -> Vec<String> {
    let Some(described) = at.described else {
        return Vec::new();
    };
    let Some(whole) = described.whole else {
        return Vec::new();
    };
    let item_path = join(at.path, &index.to_string());
    let mut names = Vec::new();
    for field in key_fields(at, item, index) {
        let Some(key) = item.get(&field) else {
            continue;
        };
        let key_path = join(&item_path, &field);
        for referring in described
            .registry
            .references_to(described.type_name, &key_path)
        {
            if values_at(whole, referring).contains(&key) {
                names.push(inspector::humanize(
                    referring.rsplit('.').next().unwrap_or(referring),
                ));
            }
        }
    }
    names
}

/// Every value at a dotted path. A numeric step indexes a list, and a step
/// ending in `[]` walks every item of one: a material a biome names is still
/// named, whichever biome names it.
fn values_at<'v>(root: &'v Value, path: &str) -> Vec<&'v Value> {
    let mut here = vec![root];
    for step in path.split('.') {
        here = here
            .into_iter()
            .flat_map(|value| -> Vec<&Value> {
                if let Some(list) = step.strip_suffix("[]") {
                    value
                        .get(list)
                        .and_then(Value::as_array)
                        .map(|items| items.iter().collect())
                        .unwrap_or_default()
                } else if let Value::Array(items) = value {
                    step.parse::<usize>()
                        .ok()
                        .and_then(|index| items.get(index))
                        .into_iter()
                        .collect()
                } else {
                    value.get(step).into_iter().collect()
                }
            })
            .collect();
    }
    here
}

/// The value at a dotted path, where a numeric step indexes a list.
fn value_at<'v>(root: &'v Value, path: &str) -> Option<&'v Value> {
    values_at(root, path).into_iter().next()
}

/// The keys a `KeyOf(target)` field may choose from, in list order.
fn keys_of(whole: &Value, target: &str) -> Vec<Value> {
    let Some((list, field)) = target.split_once("[].") else {
        return Vec::new();
    };
    value_at(whole, list)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get(field).cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// A field naming a list item, chosen from the keys the list holds.
///
/// A number typed or dragged here passed through every ID between the old and
/// the new, each one a world the engine refused. A menu of the keys that exist
/// has no such values in it. A stored key the list no longer holds is shown in
/// the warning colour rather than hidden, because it is the reason the
/// component is not drawing.
///
/// Returns false, drawing nothing, when there is no whole component to read
/// the keys from; the caller then draws the field by shape.
pub(crate) fn key_of_row(
    ui: &mut egui::Ui,
    at: At<'_>,
    label: &str,
    target: &str,
    value: &mut Value,
    indent: f32,
) -> bool {
    let Some(whole) = at.described.and_then(|described| described.whole) else {
        return false;
    };
    let options = keys_of(whole, target);
    let (list, field) = target.split_once("[].").unwrap_or((target, target));
    let naming = format!(
        "One of the {}, named by its {}",
        inspector::humanize(list),
        field.replace('_', " ")
    );
    // A reference the component may leave unset holds null, and its exemplar
    // does too: a world with no water names no water material. Such a field
    // offers "None" beside the keys, and holding it is not a problem.
    let optional = value.is_null()
        || at.described.is_some_and(|described| {
            described
                .registry
                .exemplar(described.type_name, &exemplar_path(at.path))
                .is_some_and(Value::is_null)
        });
    let missing = !value.is_null() && !options.contains(value);
    let mut chosen = value.clone();
    // A list of blocks shows each as the cube it is, so choosing water is
    // choosing the water block rather than remembering that it is 12.
    let pictures = at.described.map(|described| described.assets.pictures);
    let faces = |key: &Value| pictures.and_then(|pictures| faces_of(whole, target, key, pictures));
    let blocks = options.first().is_some_and(|key| faces(key).is_some());
    property::Property::new(label)
        .indent(indent)
        .show(ui, |ui| {
            let shown = RichText::new(if missing {
                format!("{value} (not defined)")
            } else if value.is_null() {
                NONE.to_owned()
            } else {
                value.to_string()
            })
            .size(text::LABEL)
            .color(if missing {
                color::WARNING
            } else {
                color::TEXT_MUTED
            });
            let mut width = property::picker_width(ui);
            if blocks {
                let (top, side) = faces(value).unwrap_or_default();
                cube::cube(ui, cube::ROW, top, side);
                width -= cube::row_width();
            }
            egui::ComboBox::from_id_salt(("key-of", at.path))
                .selected_text(shown)
                .width(width)
                .show_ui(ui, |ui| {
                    if optional {
                        ui.horizontal(|ui| {
                            if blocks {
                                ui.add_space(cube::row_width());
                            }
                            ui.selectable_value(&mut chosen, Value::Null, NONE);
                        });
                    }
                    for option in &options {
                        ui.horizontal(|ui| {
                            if let Some((top, side)) = faces(option) {
                                cube::cube(ui, cube::ROW, top, side);
                            }
                            ui.selectable_value(&mut chosen, option.clone(), option.to_string());
                        });
                    }
                })
                .response
                .on_hover_text(if missing {
                    format!("No item of the {} has this key, so the component cannot be drawn. Choose one that exists.", inspector::humanize(list))
                } else {
                    naming
                });
        });
    if chosen != *value {
        *value = chosen;
    }
    true
}

/// The faces of the item a key names, when the item is a block: a list item
/// with `top` and `side` textures, as a voxel material is.
fn faces_of(
    whole: &Value,
    target: &str,
    key: &Value,
    pictures: &Pictures,
) -> Option<(Option<Picture>, Option<Picture>)> {
    let (list, field) = target.split_once("[].")?;
    let item = value_at(whole, list)?
        .as_array()?
        .iter()
        .find(|item| item.get(field) == Some(key))?;
    faces(item, pictures)
}

fn faces(item: &Value, pictures: &Pictures) -> Option<(Option<Picture>, Option<Picture>)> {
    let top = item.get("top")?.as_str()?;
    let side = item.get("side")?.as_str()?;
    Some((pictures.get(top).copied(), pictures.get(side).copied()))
}

const KEY_HINT: &str =
    "This item's key. Other fields name it by this, so it is set when the item is added";

/// What an unset optional reference is called in its picker.
const NONE: &str = "None";

/// The template path for a field path: `biomes.2.surface_voxel` is
/// `biomes[].surface_voxel`, which is what an exemplar is looked up by.
pub(crate) fn exemplar_path(path: &str) -> String {
    let mut steps: Vec<String> = Vec::new();
    for step in path.split('.') {
        match steps.last_mut() {
            Some(previous) if step.parse::<usize>().is_ok() => previous.push_str("[]"),
            _ => steps.push(step.to_owned()),
        }
    }
    steps.join(".")
}

/// A key, shown rather than edited.
///
/// Other fields name the item by it, so changing it in place would leave them
/// naming nothing; a new item is numbered when it is added instead.
///
/// A block's key is drawn beside the block, so the list of materials is a list
/// of what they look like.
pub(crate) fn key_row(ui: &mut egui::Ui, at: At<'_>, label: &str, value: &Value, indent: f32) {
    let item_faces = at.described.and_then(|described| {
        let item = value_at(described.whole?, at.path.rsplit_once('.')?.0)?;
        faces(item, described.assets.pictures)
    });
    if let Some((top, side)) = item_faces {
        property::Property::new(label)
            .indent(indent)
            .show(ui, |ui| {
                cube::cube(ui, cube::ROW, top, side);
                ui.label(
                    RichText::new(value.to_string())
                        .size(text::LABEL)
                        .color(color::TEXT_MUTED),
                )
                .on_hover_text(KEY_HINT);
            });
        return;
    }
    property::readout_indented(ui, label, &value.to_string(), Some(KEY_HINT), indent);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn material(voxel: u64) -> Value {
        json!({ "voxel": voxel, "top": "grass.png" })
    }

    /// Add copies the exemplar, material 1; numbering it has to move it past
    /// every key already held rather than fill the first gap, which would
    /// give it the ID of one just removed and still referred to.
    #[test]
    fn a_new_item_takes_the_next_key_nobody_holds() {
        let mut items = vec![material(1), material(2), material(5), material(1)];
        assign_next_keys(&mut items, 3, &["voxel".to_owned()]);
        assert_eq!(items[3]["voxel"], json!(6));
    }

    #[test]
    fn an_item_index_becomes_the_list_step_of_its_template_path() {
        assert_eq!(
            exemplar_path("generator.biomes.2.surface_voxel"),
            "generator.biomes[].surface_voxel"
        );
        assert_eq!(
            exemplar_path("generator.water_voxel"),
            "generator.water_voxel"
        );
    }

    #[test]
    fn a_key_named_inside_any_list_item_is_found() {
        let whole = json!({ "generator": { "biomes": [
            { "surface_voxel": 1 }, { "surface_voxel": 7 }
        ] } });
        let named = values_at(&whole, "generator.biomes[].surface_voxel");
        assert!(named.contains(&&json!(7)), "{named:?}");
        assert_eq!(
            value_at(&whole, "generator.biomes.1.surface_voxel"),
            Some(&json!(7))
        );
    }

    #[test]
    fn the_first_key_is_one() {
        let mut items = vec![material(0)];
        assign_next_keys(&mut items, 0, &["voxel".to_owned()]);
        assert_eq!(items[0]["voxel"], json!(1));
    }

    #[test]
    fn a_reference_reads_the_keys_its_list_holds() {
        let whole = json!({
            "materials": [material(1), material(4), material(9)],
            "generator": { "surface_voxel": 4 }
        });
        assert_eq!(
            keys_of(&whole, "materials[].voxel"),
            vec![json!(1), json!(4), json!(9)]
        );
        assert_eq!(value_at(&whole, "generator.surface_voxel"), Some(&json!(4)));
        assert_eq!(value_at(&whole, "materials.1.voxel"), Some(&json!(4)));
    }
}
