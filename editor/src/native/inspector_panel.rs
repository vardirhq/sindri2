//! Editing what the selected entity holds.
//!
//! An inspector edit is never written to the world directly. A panel edits a
//! draft or a component payload, and what changed becomes checked commands on
//! the way out, so every edit undoes in one step and an edit the schema refuses
//! is refused rather than written.
//!
//! The sections below are the typed editors — text, animation, tilemap, grid,
//! script exports — that a component gets instead of a raw JSON field when
//! showing one a text box could turn a scene into one that will not load.

/// What the parent menu came back with.
///
/// "Move to the root" and "nothing was chosen" are both an absence of a parent
/// and are not the same answer, so they are separate variants rather than two
/// layers of `Option` the caller has to remember the order of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParentChoice {
    /// The menu offered no change: it is closed, or the current parent was
    /// picked again.
    Unchanged,
    /// Move out to the root.
    Root,
    /// Move under this entity.
    Under(EntityId),
}

/// The parent row, reporting a choice only when it is a change.
pub(super) fn inspector_parent(
    ui: &mut egui::Ui,
    entity: EntityId,
    parent: Option<EntityId>,
    choices: &[(EntityId, String)],
) -> ParentChoice {
    let mut chosen = parent;
    let current = parent
        .and_then(|parent| {
            choices
                .iter()
                .find(|(candidate, _)| *candidate == parent)
                .map(|(_, name)| name.clone())
        })
        .unwrap_or_else(|| ROOT_LABEL.to_owned());
    ui.horizontal(|ui| {
        ui.add_space(27.0);
        ui.label(RichText::new("Parent").size(11.0).color(TEXT_FAINT));
        egui::ComboBox::from_id_salt(("parent", entity.index()))
            .selected_text(RichText::new(current).size(11.0).color(TEXT_MUTED))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, ROOT_LABEL);
                for (candidate, name) in choices {
                    ui.selectable_value(&mut chosen, Some(*candidate), name);
                }
            });
    });
    if chosen == parent {
        return ParentChoice::Unchanged;
    }
    chosen.map_or(ParentChoice::Root, ParentChoice::Under)
}

pub(super) fn inspector_identity(ui: &mut egui::Ui, icon: MaterialIcon, draft: &mut EntityDraft) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(icon.outlined().rich_text().size(19.0).color(TEXT_MUTED));
        ui.add_sized(
            [ui.available_width() - 18.0, 29.0],
            egui::TextEdit::singleline(&mut draft.name).font(FontId::proportional(13.0)),
        );
    });
    // "Tag  Untagged" and "Layer  Default" used to sit under the name. Neither
    // is a thing a Sindri entity has, so they were two lines of a different
    // engine's inspector printed over this one's.
}

pub(super) fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    section_header(ui, ICON_OPEN_WITH, "Transform");
    // The Z drag is taken away rather than left to fail: the command layer
    // would refuse the edit anyway, and a control that cannot do what it looks
    // like it does is the thing this editor is trying not to grow.
    vector_row(ui, "Position", &mut transform.position, transform.z_locked);
    let rotation = Quat::from_array(transform.rotation);
    let rotation = if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
    if vector_row(ui, "Rotation", &mut degrees, false) {
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            degrees[0].to_radians(),
            degrees[1].to_radians(),
            degrees[2].to_radians(),
        )
        .to_array();
    }
    vector_row(ui, "Scale", &mut transform.scale, false);
    property_toggle(ui, "Z lock", &mut transform.z_locked, "Locked", "Free");
}

/// Stateful authoring surfaces shared across component sections.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_CAMERA_ALT, ICON_CODE, ICON_DELETE, ICON_DEPLOYED_CODE, ICON_GRID_4X4, ICON_GRID_VIEW,
        ICON_IMAGE, ICON_LABEL, ICON_OPEN_WITH, ICON_PLAY_ARROW, ICON_VIEW_IN_AR,
    },
};
use glam::{EulerRot, Quat};
use serde_json::Value;
use sindri_core::{
    CommandBuffer, ComponentMetadata, ComponentSchemaRegistry, EntityData, EntityId, SpriteRef,
    Transform3D, World, WorldCommand,
};
use sindri_decay::ScriptValue;
use sindri_scene::SpriteSpace;

use crate::{
    animation::{self, AnimationTool},
    inspector,
    scripts::SceneScripts,
    tilemap::{self, PaletteSprite, TilemapTool, resize as resize_tilemap},
};

use super::{
    ACCENT, ACCENT_BRIGHT, ACCENT_SOFT, BORDER, BORDER_SUBTLE, EditorApp,
    GRID_NAVIGATION_COMPONENT, GRID_OCCUPANT_COMPONENT, PANEL_BG, PROBLEM, ROOT_LABEL, TEXT,
    TEXT_COMPONENT, TEXT_FAINT, TEXT_MUTED, component_label, entity_icon, entity_name, grid_side,
    panel_title, reparent_choices,
    theme::{FIELD_BG, property_label, property_toggle, section_header},
};

pub(super) struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// What the inspector reads about the project it is editing inside.
///
/// Grouped rather than passed one by one because every component section wants
/// some subset of it, and the list only grows: each new component that names a
/// project asset would otherwise add another parameter to one signature.
pub(super) struct InspectorProject<'a> {
    scripts: &'a SceneScripts,
    root: Option<&'a Path>,
    fonts: &'a [String],
    animation_texture: Option<&'a str>,
    grids: &'a [(String, String)],
}

/// Draws every component on an entity, editable, and reports what changed.
///
/// The payload is edited in place on a draft; the caller diffs it and turns
/// each difference into a `SetComponent`. Nothing here writes to the world.
pub(super) fn components_sections(
    ui: &mut egui::Ui,
    components: &mut BTreeMap<String, Value>,
    project: &InspectorProject<'_>,
    tools: &mut InspectorTools<'_>,
) -> Option<String> {
    let InspectorProject {
        scripts,
        root: project_root,
        fonts,
        animation_texture,
        grids,
    } = *project;
    let grid_size = components
        .get(tilemap::TYPE_NAME)
        .and_then(|payload| tilemap::component(payload).ok())
        .map(|map| (map.columns, map.rows));
    let mut removed = None;
    for (name, payload) in components.iter_mut() {
        let icon = match name.as_str() {
            "sindri.camera" => ICON_CAMERA_ALT,
            "sindri.sprite" => ICON_IMAGE,
            "sindri.mesh" => ICON_VIEW_IN_AR,
            "sindri.script" => ICON_CODE,
            "sindri.text" => ICON_LABEL,
            "sindri.animation.sprite" => ICON_PLAY_ARROW,
            "sindri.tilemap" => ICON_GRID_VIEW,
            _ => ICON_DEPLOYED_CODE,
        };
        ui.add_space(4.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
            ui.label(
                RichText::new(component_label(name))
                    .strong()
                    .size(12.0)
                    .color(TEXT),
            );
            if inspector::is_removable(name) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(7.0);
                    if ui
                        .small_button(ICON_DELETE.outlined().rich_text().size(13.0))
                        .on_hover_text(format!("Remove {}", component_label(name)))
                        .clicked()
                    {
                        removed = Some(name.clone());
                    }
                });
            }
        });

        // A script's @export fields come first and are drawn from what the
        // script declared, which is the whole reason the language is typed.
        // The rest of the payload -- the source, the container -- follows as
        // ordinary rows.
        if name == "sindri.script" {
            script_exports_section(ui, payload, scripts);
        }
        if name == TEXT_COMPONENT {
            text_section(ui, payload, fonts);
        }
        if name == animation::TYPE_NAME {
            animation_section(
                ui,
                payload,
                project_root,
                animation_texture,
                tools.animation,
            );
        }
        if name == tilemap::TYPE_NAME {
            tilemap_section(ui, payload, project_root, tools.tilemap);
        }
        if name == GRID_NAVIGATION_COMPONENT {
            grid_navigation_section(ui, payload, grid_size);
        }
        if name == GRID_OCCUPANT_COMPONENT {
            grid_occupant_section(ui, payload, grids);
        }
        object_rows(ui, name, payload, name == "sindri.script");
    }
    removed
}

/// The two text fields whose meaning is richer than their JSON shape.
///
/// Content is multiline gameplay/UI copy, and a font is a project-owned asset
/// reference. Leaving either as an ordinary one-line string technically edits
/// the payload but makes the editor less useful than editing JSON by hand.
pub(super) fn text_section(ui: &mut egui::Ui, payload: &mut Value, fonts: &[String]) {
    let mut content = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Text").size(11.0).color(TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let width = (ui.available_width() - 7.0).max(120.0);
        if ui
            .add_sized(
                [width, 76.0],
                egui::TextEdit::multiline(&mut content)
                    .desired_rows(3)
                    .hint_text("Text shown in the game"),
            )
            .changed()
        {
            payload["text"] = Value::String(content);
        }
    });

    let current = payload
        .get("font")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Font").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("text-font-asset")
                .selected_text(if chosen.is_empty() {
                    "Choose a font"
                } else {
                    chosen.as_str()
                })
                .width(190.0)
                .show_ui(ui, |ui| {
                    for font in fonts {
                        ui.selectable_value(&mut chosen, font.clone(), font);
                    }
                });
        });
    });
    if chosen != current {
        payload["font"] = Value::String(chosen.clone());
    }

    let missing = fonts.is_empty() || chosen.is_empty() || !fonts.contains(&chosen);
    if missing {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            let message = if fonts.is_empty() {
                "Add an OpenType font to the project before adding text."
            } else {
                "The selected font is not present in this project."
            };
            ui.label(RichText::new(message).size(9.0).color(PROBLEM));
        });
    }
}

/// Clip authoring for the selected entity's sprite sheet.
///
/// The sheet owns sprite names; the animation only arranges those names into
/// timed clips. Every edit stays in the stored payload so unknown future fields
/// survive, while the typed component is used to interpret and preview it.
#[allow(clippy::too_many_lines)]
pub(super) fn animation_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    project_root: Option<&Path>,
    texture: Option<&str>,
    tool: &mut AnimationTool,
) {
    let Some(texture) = texture else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Add a Sprite component before authoring animation clips.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
        return;
    };
    tool.palette.ensure(project_root, texture);
    let sprite_names: Vec<String> = tool
        .palette
        .sprites()
        .iter()
        .map(|sprite| sprite.name.clone())
        .collect();

    let Ok(mut authored) = animation::component(payload) else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("This animation cannot be read; repair its stored fields first.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
        return;
    };

    section_header(ui, ICON_PLAY_ARROW, "Clips");
    let mut selected = tool.selected(&authored).map(str::to_owned);
    let clip_names: Vec<String> = authored.clips.keys().cloned().collect();
    let mut chosen = selected.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Clip").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("animation-clip")
                .selected_text(if chosen.is_empty() {
                    "No clips"
                } else {
                    chosen.as_str()
                })
                .width(170.0)
                .show_ui(ui, |ui| {
                    for name in &clip_names {
                        ui.selectable_value(&mut chosen, name.clone(), name);
                    }
                });
        });
    });
    if selected.as_deref() != Some(chosen.as_str()) && !chosen.is_empty() {
        tool.select(chosen.clone());
        selected = Some(chosen);
    }

    let mut add = false;
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        add = ui
            .add_enabled(!sprite_names.is_empty(), egui::Button::new("Add clip"))
            .clicked();
        remove = ui
            .add_enabled(selected.is_some(), egui::Button::new("Remove"))
            .clicked();
    });
    if add
        && let Some(first) = sprite_names.first()
        && let Ok(name) = animation::add_clip(payload, first)
    {
        tool.select(name.clone());
        selected = Some(name);
        authored = animation::component(payload).unwrap_or(authored);
    }
    if remove
        && let Some(name) = selected.as_deref()
        && animation::remove_clip(payload, name).unwrap_or(false)
    {
        tool.reset();
        tool.palette.ensure(project_root, texture);
        authored = animation::component(payload).unwrap_or(authored);
        selected = tool.selected(&authored).map(str::to_owned);
    }

    let Some(selected) = selected else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(if sprite_names.is_empty() {
                    "Slice and name the sprite texture before adding a clip."
                } else {
                    "Add a clip to arrange the sheet's sprites into playback."
                })
                .size(9.0)
                .color(TEXT_MUTED),
            );
        });
        if let Some(problem) = tool.palette.problem() {
            animation_problem(ui, problem);
        }
        return;
    };

    let mut rename_to = tool.rename().clone();
    let mut rename = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Name").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            rename = ui
                .add_enabled(rename_to.trim() != selected, egui::Button::new("Rename"))
                .clicked();
            ui.add_sized([128.0, 23.0], egui::TextEdit::singleline(&mut rename_to));
        });
    });
    tool.rename().clone_from(&rename_to);
    let mut problem = None;
    let selected = if rename {
        match animation::rename_clip(payload, &selected, &rename_to) {
            Ok(true) => {
                let renamed = rename_to.trim().to_owned();
                tool.renamed(renamed.clone());
                authored = animation::component(payload).unwrap_or(authored);
                renamed
            }
            Ok(false) => selected,
            Err(error) => {
                problem = Some(error);
                selected
            }
        }
    } else {
        selected
    };

    let mut playing = authored.playing.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Playing").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("animation-playing")
                .selected_text(if playing.is_empty() {
                    "None"
                } else {
                    playing.as_str()
                })
                .width(170.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut playing, String::new(), "None");
                    for name in authored.clips.keys() {
                        ui.selectable_value(&mut playing, name.clone(), name);
                    }
                });
        });
    });
    let stored_playing = payload.get("playing").and_then(Value::as_str).unwrap_or("");
    if playing != stored_playing {
        payload["playing"] = if playing.is_empty() {
            Value::Null
        } else {
            Value::String(playing)
        };
    }

    let Some(clip) = authored.clips.get(&selected).cloned() else {
        animation_problem(ui, "The selected clip no longer exists.");
        return;
    };
    let mut seconds = f64::from(clip.seconds_per_frame);
    if number_row(ui, "Frame time", &mut seconds, 10.0, false) {
        payload["clips"][selected.as_str()]["seconds_per_frame"] = Value::from(seconds.max(0.001));
    }
    let mut looping = clip.looping;
    if bool_row(ui, "Loop", &mut looping, 10.0) {
        payload["clips"][selected.as_str()]["looping"] = Value::Bool(looping);
    }

    section_header(ui, ICON_IMAGE, "Frames");
    let mut replace = None;
    let mut frame_action = None;
    for (index, frame) in clip.frames.iter().enumerate() {
        let mut sprite = frame.clone();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{}", index + 1))
                    .size(10.0)
                    .color(TEXT_FAINT),
            );
            egui::ComboBox::from_id_salt(("animation-frame", index))
                .selected_text(&sprite)
                .width(132.0)
                .show_ui(ui, |ui| {
                    for name in &sprite_names {
                        ui.selectable_value(&mut sprite, name.clone(), name);
                    }
                });
            if ui.small_button("Up").clicked() {
                frame_action = Some((index, -1));
            }
            if ui.small_button("Down").clicked() {
                frame_action = Some((index, 1));
            }
            if ui
                .add_enabled(clip.frames.len() > 1, egui::Button::new("Remove"))
                .clicked()
            {
                frame_action = Some((index, 0));
            }
        });
        if sprite != *frame {
            replace = Some((index, sprite));
        }
        if !sprite_names.contains(frame) {
            animation_problem(
                ui,
                &format!("Frame {} names missing sprite {frame:?}.", index + 1),
            );
        }
    }
    if let Some((index, sprite)) = replace {
        let _ = animation::set_frame(payload, &selected, index, &sprite);
    }
    if let Some((index, direction)) = frame_action {
        if direction == 0 {
            let _ = animation::remove_frame(payload, &selected, index);
        } else {
            let _ = animation::move_frame(payload, &selected, index, direction);
        }
    }
    let mut appended = None;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.menu_button("Add frame", |ui| {
            for sprite in &sprite_names {
                if ui.button(sprite).clicked() {
                    appended = Some(sprite.clone());
                    ui.close();
                }
            }
            if sprite_names.is_empty() {
                ui.label("No named sprites");
            }
        });
    });
    if let Some(sprite) = appended {
        let _ = animation::push_frame(payload, &selected, &sprite);
    }

    if let Ok(updated) = animation::component(payload)
        && let Some(clip) = updated.clips.get(&selected)
    {
        animation_preview(ui, texture, &selected, clip, tool);
    }
    if let Some(message) = problem.as_deref().or_else(|| tool.palette.problem()) {
        animation_problem(ui, message);
    }
}

pub(super) fn animation_preview(
    ui: &mut egui::Ui,
    texture_name: &str,
    clip_name: &str,
    clip: &sindri_scene::AnimationClip,
    tool: &mut AnimationTool,
) {
    section_header(ui, ICON_PLAY_ARROW, "Preview");
    let mut previewing = tool.previewing();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui
            .button(if previewing { "Stop" } else { "Play" })
            .clicked()
        {
            previewing = !previewing;
            tool.set_previewing(previewing);
        }
        ui.label(
            RichText::new(format!(
                "{} frames · {:.3}s",
                clip.frames.len(),
                clip.seconds_per_frame
            ))
            .size(10.0)
            .color(TEXT_MUTED),
        );
    });
    if previewing {
        ui.ctx().request_repaint();
    }
    let delta = ui.ctx().input(|input| input.stable_dt);
    let frame = tool.advance(clip_name, clip, delta);
    let sprite_name = clip.frames.get(frame).cloned();
    let sprite_rect = sprite_name
        .as_deref()
        .and_then(|name| tool.palette.sprite(name))
        .and_then(|sprite| sprite.rect);
    let texture = tool.palette.texture_id(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(176.0, 150.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, FIELD_BG);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, BORDER_SUBTLE),
        StrokeKind::Inside,
    );
    let image = Rect::from_min_max(
        rect.min + Vec2::splat(10.0),
        rect.max - Vec2::new(10.0, 28.0),
    );
    if let (Some(texture), Some(sprite_rect)) = (texture, sprite_rect) {
        let [x, y, width, height] = sprite_rect;
        painter.image(
            texture,
            image,
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
            Color32::WHITE,
        );
    } else {
        painter.line_segment(
            [image.left_top(), image.right_bottom()],
            Stroke::new(1.5, PROBLEM),
        );
        painter.line_segment(
            [image.right_top(), image.left_bottom()],
            Stroke::new(1.5, PROBLEM),
        );
    }
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 13.0),
        Align2::CENTER_CENTER,
        sprite_name.unwrap_or_else(|| format!("{texture_name}: no frame")),
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    let _ = response.on_hover_text("Animation preview uses the project texture and sheet");
}

pub(super) fn animation_problem(ui: &mut egui::Ui, problem: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(problem).size(9.0).color(PROBLEM));
    });
}

/// The part of a tilemap that cannot be represented as independent JSON rows:
/// its dimensions, compact palette, and the brush that writes its cell array.
pub(super) fn tilemap_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    project_root: Option<&Path>,
    tool: &mut TilemapTool,
) {
    let Ok(mut map) = tilemap::component(payload) else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("This tilemap cannot be read; repair its stored fields first")
                    .size(10.0)
                    .color(PROBLEM),
            );
        });
        return;
    };

    section_header(ui, ICON_GRID_VIEW, "Map");
    let mut columns = f64::from(map.columns);
    let mut rows = f64::from(map.rows);
    let mut resized = number_row(ui, "Columns", &mut columns, 10.0, true);
    resized |= number_row(ui, "Rows", &mut rows, 10.0, true);
    if resized && let Err(error) = resize_tilemap(payload, grid_side(columns), grid_side(rows)) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(error).size(10.0).color(PROBLEM));
        });
    }
    // The resize above changes the payload this frame. Read it again so the
    // palette and the cell count below describe what the command will write.
    if let Ok(resized) = tilemap::component(payload) {
        map = resized;
    }

    let world_space = map.space == SpriteSpace::World;
    if !world_space {
        tool.enabled = false;
    }
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let label = if tool.enabled {
            "Painting in Scene view"
        } else {
            "Paint in Scene view"
        };
        if ui
            .add_enabled_ui(world_space, |ui| ui.selectable_label(tool.enabled, label))
            .inner
            .clicked()
        {
            tool.enabled = !tool.enabled;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(if !world_space {
                "Scene painting supports world-space tilemaps; switch Space to world first."
            } else if tool.enabled {
                "Primary drag paints. Middle or Shift-drag pans; secondary drag orbits."
            } else {
                "Enable painting, then choose a sprite or the eraser."
            })
            .size(9.0)
            .color(if world_space { TEXT_MUTED } else { PROBLEM }),
        );
    });

    tool.palette.ensure(project_root, &map.texture);
    let texture = tool.palette.texture_id(ui.ctx());
    let mut sprites = tool.palette.sprites().to_vec();
    // A broken or changed sheet must not make a sprite already used by the map
    // impossible to select and replace. It stays visible as a named fallback,
    // without a thumbnail that would pretend it still resolves.
    for name in &map.palette {
        if !sprites.iter().any(|sprite| sprite.name == *name) {
            sprites.push(PaletteSprite {
                name: name.clone(),
                rect: None,
            });
        }
    }
    if !tool.erase
        && tool
            .sprite
            .as_ref()
            .is_none_or(|chosen| !sprites.iter().any(|sprite| sprite.name == *chosen))
    {
        tool.sprite = sprites.first().map(|sprite| sprite.name.clone());
    }

    section_header(ui, ICON_IMAGE, "Palette");
    tile_palette(ui, texture, &sprites, tool);
    if let Some(problem) = tool.palette.problem() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(problem).size(9.0).color(PROBLEM));
        });
    }
}

pub(super) fn tile_palette(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    sprites: &[PaletteSprite],
    tool: &mut TilemapTool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0);
        if palette_cell(ui, None, None, tool.erase) {
            tool.erase = true;
        }
        for sprite in sprites {
            let selected = !tool.erase && tool.sprite.as_deref() == Some(sprite.name.as_str());
            if palette_cell(ui, Some(sprite), texture, selected) {
                tool.erase = false;
                tool.sprite = Some(sprite.name.clone());
            }
        }
    });
    if sprites.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Slice and name the map's texture to populate this palette.")
                    .size(9.0)
                    .color(TEXT_MUTED),
            );
        });
    }
}

/// One compact palette swatch. Drawn directly so a named slice can preview a
/// UV rectangle without creating one egui texture per sprite.
pub(super) fn palette_cell(
    ui: &mut egui::Ui,
    sprite: Option<&PaletteSprite>,
    texture: Option<egui::TextureId>,
    selected: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(72.0, 72.0), Sense::click());
    let painter = ui.painter_at(rect);
    let border = if selected {
        ACCENT_BRIGHT
    } else if response.hovered() {
        TEXT_MUTED
    } else {
        BORDER
    };
    painter.rect_filled(rect, 4.0, if selected { ACCENT_SOFT } else { FIELD_BG });
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(if selected { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let preview = Rect::from_min_max(
        rect.min + Vec2::new(7.0, 6.0),
        Pos2::new(rect.max.x - 7.0, rect.max.y - 20.0),
    );
    match (sprite, texture) {
        (Some(sprite), Some(texture)) if sprite.rect.is_some() => {
            let [x, y, width, height] = sprite.rect.expect("checked above");
            painter.image(
                texture,
                preview,
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
                Color32::WHITE,
            );
        }
        (Some(_), _) => {
            painter.rect_stroke(
                preview,
                2.0,
                Stroke::new(1.0, BORDER_SUBTLE),
                StrokeKind::Inside,
            );
        }
        (None, _) => {
            painter.line_segment(
                [preview.left_top(), preview.right_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
            painter.line_segment(
                [preview.right_top(), preview.left_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
        }
    }
    let label = sprite.map_or("Erase", |sprite| sprite.name.as_str());
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 12.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        if selected { TEXT } else { TEXT_MUTED },
    );
    response.on_hover_text(label).clicked()
}

pub(super) fn grid_choices(world: &World) -> Vec<(String, String)> {
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

/// The rows of one payload, indented under its heading.
///
/// `skip_properties` keeps a script's authored values from appearing twice:
/// they are drawn above as typed fields, from what the script declared.
pub(super) fn object_rows(
    ui: &mut egui::Ui,
    type_name: &str,
    payload: &mut Value,
    skip_properties: bool,
) {
    let Value::Object(fields) = payload else {
        return;
    };
    // Which fields apply can depend on the others, so the decision is made
    // against the payload as it was before this frame's edits.
    let whole = Value::Object(fields.clone());
    for (key, value) in fields.iter_mut() {
        if skip_properties && key == "properties" {
            continue;
        }
        if !inspector::applies(type_name, key, &whole) {
            continue;
        }
        value_row(ui, key, value, 10.0);
    }
}

/// One field, drawn as whatever its stored shape deserves.
pub(super) fn value_row(ui: &mut egui::Ui, key: &str, value: &mut Value, indent: f32) {
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
pub(super) fn opaque_summary(value: &Value) -> String {
    match value {
        Value::Null => "not set".to_owned(),
        Value::Array(items) => format!("{} items", items.len()),
        other => other.to_string(),
    }
}

/// A script's `@export` fields, drawn from what the script declared.
///
/// This is the capability that justified a statically typed language: the panel
/// knows a field exists, what it is called, what type it is, and what it starts
/// as, without running anything. A field the scene has not set shows its
/// default and says so.
pub(super) fn script_exports_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    scripts: &SceneScripts,
) {
    let source = payload.get("source").and_then(Value::as_str).unwrap_or("");
    let script = payload.get("script").and_then(Value::as_str).unwrap_or("");
    let Some(exports) = scripts.exports(source, script) else {
        // Not the same as having no properties, and saying so matters: a panel
        // that showed nothing would look like a script with nothing to author.
        property_label(ui, "Properties", "waiting for the script");
        return;
    };
    if exports.is_empty() {
        property_label(ui, "Properties", "none declared");
        return;
    }

    for export in exports {
        let stored = payload
            .get("properties")
            .and_then(|properties| properties.get(&export.name))
            .cloned();
        let authored = stored.is_some();
        let mut value = stored.unwrap_or_else(|| script_value_json(&export.default));
        let label = inspector::humanize(&export.name);

        let before = value.clone();
        value_row(ui, &export.name, &mut value, 10.0);
        if value != before {
            // Setting a property is what puts it in the scene: a field left
            // alone stays absent, so a scene records the author's choices
            // rather than a copy of every default.
            let properties = payload
                .as_object_mut()
                .expect("a script component is an object")
                .entry("properties")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(properties) = properties.as_object_mut() {
                properties.insert(export.name.clone(), value);
            }
        } else if !authored {
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.label(
                    RichText::new(format!(
                        "default{}",
                        export
                            .type_name
                            .as_ref()
                            .map_or_else(String::new, |name| format!(" · {name}"))
                    ))
                    .size(9.0)
                    .color(TEXT_MUTED),
                );
            });
        }
        let _ = label;
    }
}

/// A Decay value as the JSON a scene stores.
///
/// A reference stores as null, because it names a runtime handle and runtime
/// handles are never serialized: writing one to a scene would produce a file
/// that means something different the next time it is opened. An `@export` of
/// an entity is not authorable for that reason, and the inspector shows it as
/// empty rather than as a number nobody can act on.
pub(super) fn script_value_json(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::Number(number) => Value::from(*number),
        ScriptValue::Bool(flag) => Value::Bool(*flag),
        ScriptValue::String(text) => Value::String(text.clone()),
        ScriptValue::Reference(_) | ScriptValue::Null | ScriptValue::Unit => Value::Null,
    }
}

/// A labelled drag, reporting whether it moved.
pub(super) fn number_row(
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

pub(super) fn bool_row(ui: &mut egui::Ui, label: &str, value: &mut bool, indent: f32) -> bool {
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

pub(super) fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, indent: f32) -> bool {
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
pub(super) fn numbers_row(
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
pub(super) fn add_component_button(
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
pub(super) fn vector_row(
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

/// The inspector's editable copy of an entity.
///
/// Widgets write here rather than into the world, so every change can be
/// turned into a command instead of a silent mutation.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EntityDraft {
    pub(super) name: String,
    pub(super) transform_3d: Option<Transform3D>,
}

impl From<&EntityData> for EntityDraft {
    fn from(data: &EntityData) -> Self {
        Self {
            name: entity_name(data),
            transform_3d: data.transform_3d,
        }
    }
}

/// Turns the difference between an entity's stored state and the drawn draft
/// into the commands that close the gap.
/// Turns every changed component payload into a command, and says what it
/// refused.
///
/// Kept apart from the drawing of it so the claims — that an edit becomes a
/// command, and that one which breaks a schema becomes nothing — are things a
/// test can check without a window or a GPU.
///
/// A payload is written back exactly as stored, so an edit that stopped it
/// decoding would produce a scene the engine refuses to open. Checking here
/// means the author hears about it at the field they were editing rather than
/// at the next launch.
pub(super) fn component_commands(
    entity: EntityId,
    original: &BTreeMap<String, Value>,
    draft: &BTreeMap<String, Value>,
    components: &ComponentSchemaRegistry,
) -> (CommandBuffer, Vec<String>) {
    let mut buffer = CommandBuffer::new();
    let mut refused = Vec::new();
    for (type_name, payload) in draft {
        if original.get(type_name) == Some(payload) {
            continue;
        }
        if let Err(error) = components.validate_payload(type_name, payload) {
            refused.push(error.to_string());
            continue;
        }
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.clone(),
            payload: payload.clone(),
        });
    }
    (buffer, refused)
}

/// The components an entity does not have and the registry can create.
///
/// A type with no default payload is missing from the list rather than offered
/// and refused: a button that adds a component the engine will then reject is
/// worse than no button, which is why the old Add Component was removed rather
/// than left drawn.
pub(super) fn addable_components(
    components: &ComponentSchemaRegistry,
    present: &BTreeMap<String, Value>,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Vec<ComponentMetadata> {
    components
        .registered_components()
        .filter(|metadata| !present.contains_key(&metadata.type_name))
        .filter(|metadata| {
            metadata.type_name != GRID_NAVIGATION_COMPONENT
                || present.contains_key(tilemap::TYPE_NAME)
        })
        .filter(|metadata| {
            component_default(
                components,
                &metadata.type_name,
                first_font,
                first_sprite,
                first_grid,
            )
            .is_some()
        })
        .cloned()
        .collect()
}

/// What Add Component writes for a fresh component.
///
/// Built-ins normally own a fixed default in the registry. Text and sprite
/// animation cannot: their reproducible asset references must come from the
/// project, so their defaults are completed at the editor boundary.
pub(super) fn component_default(
    components: &ComponentSchemaRegistry,
    type_name: &str,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Option<Value> {
    if type_name == GRID_OCCUPANT_COMPONENT {
        return first_grid.map(|grid| {
            serde_json::json!({
                "grid": grid,
                "footprint": [[0, 0]]
            })
        });
    }
    if type_name == TEXT_COMPONENT {
        return first_font.map(|font| {
            serde_json::json!({
                "text": "Text",
                "font": font
            })
        });
    }
    if type_name == animation::TYPE_NAME {
        return first_sprite.map(|sprite| {
            serde_json::json!({
                "clips": {
                    "clip": {
                        "frames": [sprite],
                        "seconds_per_frame": 0.1,
                        "looping": true
                    }
                },
                "playing": "clip",
                "speed": 1.0
            })
        });
    }
    components.default_payload(type_name).cloned()
}

pub(super) fn draft_commands(
    entity: EntityId,
    original: &EntityDraft,
    draft: &EntityDraft,
) -> CommandBuffer {
    let mut buffer = CommandBuffer::new();
    if original.name != draft.name {
        buffer.push(WorldCommand::SetName {
            entity,
            name: Some(draft.name.clone()),
        });
    }
    if original.transform_3d != draft.transform_3d {
        buffer.push(WorldCommand::SetTransform3D {
            entity,
            transform: draft.transform_3d,
        });
    }
    buffer
}

impl EditorApp {
    /// Turns the difference between the drawn draft and the world into one
    /// transaction, so inspector edits are undoable and reach the viewport.
    pub(super) fn commit_draft(
        &mut self,
        entity: EntityId,
        original: &EntityDraft,
        draft: &EntityDraft,
    ) {
        let buffer = draft_commands(entity, original, draft);
        if buffer.is_empty() {
            return;
        }

        // One merge key per entity: a continuous drag stays a single undo step
        // until the pointer is released or the selection changes.
        let transaction = buffer
            .into_transaction("Edit entity")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// The components this entity does not have and the registry can create.
    ///
    /// A type with no default payload is missing from the list rather than
    /// offered and refused: a button that adds a component the engine will
    /// then reject is worse than no button, which is why the old Add Component
    /// was removed instead of left drawn.
    pub(super) fn addable_components(
        &self,
        present: &BTreeMap<String, Value>,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) -> Vec<ComponentMetadata> {
        addable_components(
            self.scene.components(),
            present,
            first_font,
            first_sprite,
            first_grid,
        )
    }

    /// Turns every changed component payload into a command.
    ///
    /// Each is checked against its own schema first. A payload is written back
    /// exactly as stored, so an edit that stopped it decoding would produce a
    /// scene the engine refuses to open — and the author would find out on the
    /// next launch rather than at the field they were editing.
    pub(super) fn commit_components(
        &mut self,
        entity: EntityId,
        original: &BTreeMap<String, Value>,
        draft: &BTreeMap<String, Value>,
    ) {
        let (buffer, refused) =
            component_commands(entity, original, draft, self.scene.components());
        for message in refused {
            self.console.warning(message);
        }
        if buffer.is_empty() {
            return;
        }
        // The same merge key the rest of the inspector uses, so dragging a tint
        // is one undo step rather than one per frame of the drag.
        let transaction = buffer
            .into_transaction("Edit components")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Adds a component with the payload its schema says a fresh one starts as.
    pub(super) fn add_component(
        &mut self,
        entity: EntityId,
        type_name: &str,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) {
        let Some(payload) = component_default(
            self.scene.components(),
            type_name,
            first_font,
            first_sprite,
            first_grid,
        ) else {
            return;
        };
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.to_owned(),
            payload,
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Add component"), &mut self.world)
        {
            self.report(error.to_string());
        }
        self.refresh_textures();
    }

    pub(super) fn remove_component(&mut self, entity: EntityId, type_name: &str) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::RemoveComponent {
            entity,
            type_name: type_name.to_owned(),
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Remove component"), &mut self.world)
        {
            self.report(error.to_string());
        }
        self.refresh_textures();
    }

    pub(super) fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(340.0)
            .min_size(300.0)
            .max_size(440.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Inspector");
                if self.slicer.is_some() {
                    self.slicer_panel(ui);
                    return;
                }
                let Some(entity) = self.selection else {
                    return;
                };
                let Some(data) = self.world.get(entity) else {
                    return;
                };
                // Widgets edit a draft copy; every difference becomes a command,
                // so the world is only ever written through the command layer.
                let mut draft = EntityDraft::from(data);
                let original = draft.clone();
                let icon = entity_icon(data);
                let original_components = data.components.clone();
                let mut components = original_components.clone();
                let parent = data.parent;
                let choices = reparent_choices(&self.world, entity);
                let mut reparented = ParentChoice::Unchanged;
                let mut removed = None;
                let mut added = None;
                let fonts = self.project.fonts();
                let first_font = fonts.first().map(String::as_str);
                let animation_texture = components
                    .get("sindri.sprite")
                    .and_then(|sprite| sprite.get("texture"))
                    .and_then(Value::as_str)
                    .and_then(|reference| SpriteRef::parse(reference).ok())
                    .map(|reference| reference.texture().to_owned());
                let animation_sprites = animation_texture
                    .as_deref()
                    .map(|texture| self.project.sprites_for_texture(texture))
                    .unwrap_or_default();
                let first_sprite = animation_sprites.first().map(String::as_str);
                let grids = grid_choices(&self.world);
                let first_grid = grids.first().map(|(_, id)| id.as_str());
                let addable =
                    self.addable_components(&components, first_font, first_sprite, first_grid);
                let project_root = self.project.root().map(Path::to_path_buf);
                {
                    let scripts = &self.scripts;
                    let mut tools = InspectorTools {
                        animation: &mut self.animation_tool,
                        tilemap: &mut self.tilemap_tool,
                    };
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, icon, &mut draft);
                        reparented = inspector_parent(ui, entity, parent, &choices);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        removed = components_sections(
                            ui,
                            &mut components,
                            &InspectorProject {
                                scripts,
                                root: project_root.as_deref(),
                                fonts: &fonts,
                                animation_texture: animation_texture.as_deref(),
                                grids: &grids,
                            },
                            &mut tools,
                        );
                        added = add_component_button(ui, &addable);
                    });
                }
                self.commit_draft(entity, &original, &draft);
                self.commit_components(entity, &original_components, &components);
                if let Some(type_name) = removed {
                    self.remove_component(entity, &type_name);
                }
                if let Some(type_name) = added {
                    self.add_component(entity, &type_name, first_font, first_sprite, first_grid);
                }
                match reparented {
                    ParentChoice::Unchanged => {}
                    ParentChoice::Root => self.reparent(entity, None),
                    ParentChoice::Under(parent) => self.reparent(entity, Some(parent)),
                }
            });
    }
}
