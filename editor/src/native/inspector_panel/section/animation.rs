//! `sindri.sprite_animation`: frames, timing, and what they look like.

use std::path::Path;

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2,
};
use serde_json::Value;

use crate::animation::{self, AnimationTool};
use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, text};
use crate::ui::widgets::{
    button::{self, Intent},
    panel, property, section,
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
    assets_root: Option<&Path>,
    texture: Option<&str>,
    tool: &mut AnimationTool,
) {
    let Some(texture) = texture else {
        panel::problem(
            ui,
            "Add a Sprite component before authoring animation clips.",
        );
        return;
    };
    tool.palette.ensure(assets_root, texture);
    let sprite_names: Vec<String> = tool
        .palette
        .sprites()
        .iter()
        .map(|sprite| sprite.name.clone())
        .collect();

    let Ok(mut authored) = animation::component(payload) else {
        panel::problem(
            ui,
            "This animation cannot be read; repair its stored fields first.",
        );
        return;
    };

    section::group(ui, icons::ANIMATION, "Clips");
    let mut selected = tool.selected(&authored).map(str::to_owned);
    let clip_names: Vec<String> = authored.clips.keys().cloned().collect();
    let mut chosen = selected.clone().unwrap_or_default();
    property::Property::new("Clip").show(ui, |ui| {
        egui::ComboBox::from_id_salt("animation-clip")
            .selected_text(if chosen.is_empty() {
                "No clips"
            } else {
                chosen.as_str()
            })
            .width(property::picker_width(ui))
            .show_ui(ui, |ui| {
                for name in &clip_names {
                    ui.selectable_value(&mut chosen, name.clone(), name);
                }
            });
    });
    if selected.as_deref() != Some(chosen.as_str()) && !chosen.is_empty() {
        tool.select(chosen.clone());
        selected = Some(chosen);
    }

    let mut add = false;
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        add = ui
            .add_enabled_ui(!sprite_names.is_empty(), |ui| {
                button::labelled(
                    ui,
                    "Add clip",
                    Intent::Normal,
                    "Start a new clip on this sheet",
                )
            })
            .inner
            .clicked();
        remove = ui
            .add_enabled_ui(selected.is_some(), |ui| {
                button::labelled(ui, "Remove clip", Intent::Danger, "Delete the chosen clip")
            })
            .inner
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
        tool.palette.ensure(assets_root, texture);
        authored = animation::component(payload).unwrap_or(authored);
        selected = tool.selected(&authored).map(str::to_owned);
    }

    let Some(selected) = selected else {
        panel::note(
            ui,
            if sprite_names.is_empty() {
                "Slice and name the sprite texture before adding a clip."
            } else {
                "Add a clip to arrange the sheet's sprites into playback."
            },
        );
        if let Some(problem) = tool.palette.problem() {
            animation_problem(ui, problem);
        }
        return;
    };

    let mut rename_to = tool.rename().clone();
    let mut rename = false;
    property::Property::new("Name").show(ui, |ui| {
        let button_width = 62.0;
        let field = (property::value_width(ui) - button_width - 4.0).max(60.0);
        ui.add_sized(
            [field, metric::CONTROL_HEIGHT],
            egui::TextEdit::singleline(&mut rename_to),
        );
        rename = ui
            .add_enabled_ui(rename_to.trim() != selected, |ui| {
                button::labelled(ui, "Rename", Intent::Normal, "Rename this clip")
            })
            .inner
            .clicked();
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
    property::Property::new("Playing")
        .tip("Which clip this entity runs when the scene plays")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("animation-playing")
                .selected_text(if playing.is_empty() {
                    "None"
                } else {
                    playing.as_str()
                })
                .width(property::picker_width(ui))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut playing, String::new(), "None");
                    for name in authored.clips.keys() {
                        ui.selectable_value(&mut playing, name.clone(), name);
                    }
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

    section::group(ui, icons::SPRITE, "Frames");
    let mut replace = None;
    let mut frame_action = None;
    for (index, frame) in clip.frames.iter().enumerate() {
        let mut sprite = frame.clone();
        // The frame number is the row's label, so the frames read as an ordered
        // list rather than as a stack of identical pickers.
        property::Property::new(&format!("{}", index + 1)).show(ui, |ui| {
            let controls = 3.0 * 20.0 + 8.0;
            let picker = (property::value_width(ui) - controls).max(70.0);
            egui::ComboBox::from_id_salt(("animation-frame", index))
                .selected_text(&sprite)
                .width(picker)
                .show_ui(ui, |ui| {
                    for name in &sprite_names {
                        ui.selectable_value(&mut sprite, name.clone(), name);
                    }
                });
            if button::row_icon(ui, icons::EXPANDED, Intent::Quiet, "Move this frame later")
                .clicked()
            {
                frame_action = Some((index, 1));
            }
            if button::row_icon(
                ui,
                icons::COLLAPSED,
                Intent::Quiet,
                "Move this frame earlier",
            )
            .clicked()
            {
                frame_action = Some((index, -1));
            }
            if ui
                .add_enabled_ui(clip.frames.len() > 1, |ui| {
                    button::row_icon(ui, icons::REMOVE, Intent::Danger, "Remove this frame")
                })
                .inner
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
        ui.add_space(metric::GUTTER);
        ui.menu_button(
            RichText::new("Add frame")
                .size(text::LABEL)
                .color(color::TEXT_MUTED),
            |ui| {
                for sprite in &sprite_names {
                    if ui.button(sprite).clicked() {
                        appended = Some(sprite.clone());
                        ui.close();
                    }
                }
                if sprite_names.is_empty() {
                    ui.label("No named sprites");
                }
            },
        );
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
    section::group(ui, icons::ANIMATION, "Preview");
    let mut previewing = tool.previewing();
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        if button::labelled(
            ui,
            if previewing { "Stop" } else { "Play" },
            if previewing {
                Intent::Normal
            } else {
                Intent::Primary
            },
            "Run this clip in the panel without running the scene",
        )
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
            .size(text::NOTE)
            .color(color::TEXT_FAINT),
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
    ui.add_space(4.0);
    let width = (ui.available_width() - 2.0 * metric::GUTTER).clamp(120.0, 220.0);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        preview_plate(ui, width, texture, sprite_rect, sprite_name, texture_name);
    });
}

/// The plate the previewed frame is drawn on.
///
/// A flat well behind the sprite, because a transparent frame drawn straight
/// onto the panel reads as a hole in the panel rather than as transparency.
fn preview_plate(
    ui: &mut egui::Ui,
    width: f32,
    texture: Option<egui::TextureId>,
    sprite_rect: Option<[f32; 4]>,
    sprite_name: Option<String>,
    texture_name: &str,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, radius(), color::WELL);
    painter.rect_stroke(
        rect,
        radius(),
        Stroke::new(1.0, color::LINE_SOFT),
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
            Stroke::new(1.5, color::DANGER),
        );
        painter.line_segment(
            [image.right_top(), image.left_bottom()],
            Stroke::new(1.5, color::DANGER),
        );
    }
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 13.0),
        Align2::CENTER_CENTER,
        sprite_name.unwrap_or_else(|| format!("{texture_name}: no frame")),
        FontId::proportional(text::NOTE),
        color::TEXT_MUTED,
    );
    let _ = response.on_hover_text("Animation preview uses the project texture and sheet");
}

pub(super) fn animation_problem(ui: &mut egui::Ui, problem: &str) {
    panel::problem(ui, problem);
}
