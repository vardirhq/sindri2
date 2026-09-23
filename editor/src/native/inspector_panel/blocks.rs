//! Choosing a block from a block set, by name and by what it looks like.
//!
//! A voxel world built from a block set names its blocks: its surface is
//! "grass", its sea "water". The picker offers every block in the set, each
//! drawn as the cube it is, so choosing one is recognising it rather than
//! remembering what it is called.
//!
//! A world with no block set names its own materials by number, and the field
//! is the material picker it always was.

use eframe::egui::{self, RichText};
use serde_json::Value;
use sindri_core::{TileDefinition, TileFace, TileSetDocument};
use sindri_scene::TileSetBindings;

use crate::inspector;
use crate::ui::theme::{color, text};
use crate::ui::widgets::cube::{self, Picture};
use crate::ui::widgets::property;

use super::super::thumbnails::Pictures;
use super::rows::At;

/// The block set a component names in its `set` field, if one is bound.
fn block_set<'a>(at: At<'a>, set: &str) -> Option<(&'a str, &'a TileSetDocument)> {
    let described = at.described?;
    let reference = described.whole?.get(set)?.as_str()?;
    if reference.trim().is_empty() {
        return None;
    }
    Some((reference, described.assets.block_sets?.get(reference)?))
}

/// Whether the component names a block set at all, loaded or not.
fn names_a_set(at: At<'_>, set: &str) -> bool {
    at.described
        .and_then(|described| described.whole)
        .and_then(|whole| whole.get(set))
        .and_then(Value::as_str)
        .is_some_and(|reference| !reference.trim().is_empty())
}

/// What a block looks like from above and from the side.
pub(crate) fn block_faces(
    block: &TileDefinition,
    pictures: &Pictures,
) -> (Option<Picture>, Option<Picture>) {
    let picture = |face| {
        block
            .faces
            .resolved(face)
            .and_then(|(_, visual)| pictures.get(&visual.sprite).copied())
    };
    (picture(TileFace::Top), picture(TileFace::South))
}

/// Every sprite a set's blocks draw with, so the panel has a picture of each.
pub(crate) fn block_sprites(tile_set: &TileSetDocument) -> impl Iterator<Item = String> + '_ {
    tile_set.tiles.values().flat_map(|block| {
        [TileFace::Top, TileFace::South]
            .into_iter()
            .filter_map(|face| block.faces.resolved(face))
            .map(|(_, visual)| visual.sprite.clone())
    })
}

/// The block set the voxel world among `components` names, alone, so the
/// panel holds what it draws from without copying every set in the project.
pub(crate) fn named_block_set(
    components: &std::collections::BTreeMap<String, Value>,
    bound: &TileSetBindings,
) -> TileSetBindings {
    let mut named = TileSetBindings::new();
    let reference = components
        .values()
        .filter_map(|payload| payload.get("blocks"))
        .find_map(Value::as_str);
    if let Some(reference) = reference
        && let Some(tile_set) = bound.get(reference)
    {
        let _ = named.bind(reference, tile_set.clone());
    }
    named
}

/// A field naming a block, drawn as a menu of the set's blocks.
///
/// Returns false, drawing nothing, while the component names no block set:
/// the caller then draws it as the material picker it falls back to.
pub(crate) fn block_row(
    ui: &mut egui::Ui,
    at: At<'_>,
    label: &str,
    set: &str,
    value: &mut Value,
    indent: f32,
) -> bool {
    if !names_a_set(at, set) {
        return false;
    }
    let pictures = at
        .described
        .map(|described| described.assets.pictures)
        .filter(|pictures| !pictures.is_empty());
    let loaded = block_set(at, set);
    let current = value.as_str().map(str::to_owned);
    let known = loaded.is_some_and(|(_, tile_set)| {
        current
            .as_deref()
            .is_some_and(|name| tile_set.tiles.contains_key(name))
    });
    // A slot the component may leave empty offers None beside the blocks.
    let optional = value.is_null()
        || at.described.is_some_and(|described| {
            described
                .registry
                .exemplar(described.type_name, &super::keys::exemplar_path(at.path))
                .is_some_and(Value::is_null)
        });
    let faces = |name: &str| {
        let (_, tile_set) = loaded?;
        Some(block_faces(tile_set.tiles.get(name)?, pictures?))
    };
    let shown = match (&current, &*value) {
        (Some(name), _) if known => inspector::humanize(name),
        (Some(name), _) => format!("{name} (not in the set)"),
        (None, Value::Null) => NONE.to_owned(),
        // A number from before the world named a set: a material this world
        // no longer has, until a block is chosen for it.
        (None, other) => format!("Material {other} (choose a block)"),
    };
    let problem = !known && !value.is_null() && loaded.is_some();
    let mut chosen = value.clone();
    property::Property::new(label)
        .indent(indent)
        .show(ui, |ui| {
            let mut width = property::picker_width(ui);
            let (top, side) = current.as_deref().and_then(faces).unwrap_or_default();
            cube::cube(ui, cube::ROW, top, side);
            width -= cube::row_width();
            egui::ComboBox::from_id_salt(("block", at.path))
                .selected_text(RichText::new(shown).size(text::LABEL).color(if problem {
                    color::WARNING
                } else {
                    color::TEXT_MUTED
                }))
                .width(width)
                .height(360.0)
                .show_ui(ui, |ui| {
                    let Some((reference, tile_set)) = loaded else {
                        ui.label(
                            RichText::new("The block set is still loading")
                                .size(text::NOTE)
                                .color(color::TEXT_FAINT),
                        );
                        return;
                    };
                    if optional {
                        ui.horizontal(|ui| {
                            ui.add_space(cube::row_width());
                            ui.selectable_value(&mut chosen, Value::Null, NONE);
                        });
                    }
                    for name in tile_set.tiles.keys() {
                        ui.horizontal(|ui| {
                            let (top, side) = faces(name).unwrap_or_default();
                            cube::cube(ui, cube::ROW, top, side);
                            ui.selectable_value(
                                &mut chosen,
                                Value::String(name.clone()),
                                inspector::humanize(name),
                            )
                            .on_hover_text(format!("{name}, from {reference}"));
                        });
                    }
                })
                .response
                .on_hover_text(if problem {
                    "Not a block in this world's block set. Choose one that is."
                } else {
                    "A block from this world's block set"
                });
        });
    if chosen != *value {
        *value = chosen;
    }
    true
}

/// What an unset optional block is called in its picker.
const NONE: &str = "None";
