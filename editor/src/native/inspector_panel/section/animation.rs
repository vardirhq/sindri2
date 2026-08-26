//! `sindri.sprite_animation`: frames, timing, and what they look like.

use std::path::Path;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};
use egui_material_icons::icons::{ICON_IMAGE, ICON_PLAY_ARROW};
use serde_json::Value;

use crate::animation::{self, AnimationTool};

use super::super::super::{
    BORDER_SUBTLE, PROBLEM, TEXT_FAINT, TEXT_MUTED,
    theme::{FIELD_BG, section_header},
};
use super::super::rows::{bool_row, number_row};

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
