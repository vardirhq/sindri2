//! Authoring sprite-animation clips without replacing their stored payload.
//!
//! A clip is scene data and the current preview frame is editor state. Keeping
//! those apart mirrors the runtime's `SpriteAnimations`: watching a preview
//! must not dirty the scene on every frame.

use serde_json::{Map, Value, json};
use sindri_scene::{AnimationClip, SpriteAnimationComponent};

use crate::tilemap::SpritePalette;

pub const TYPE_NAME: &str = "sindri.sprite_animation";

pub fn component(payload: &Value) -> Result<SpriteAnimationComponent, String> {
    serde_json::from_value(payload.clone()).map_err(|error| error.to_string())
}

/// Editor state for one selected animation component.
#[derive(Default)]
pub struct AnimationTool {
    pub palette: SpritePalette,
    selected: Option<String>,
    rename: String,
    previewing: bool,
    frame: usize,
    elapsed: f32,
    preview_key: Option<PreviewKey>,
}

#[derive(Clone, Debug, PartialEq)]
struct PreviewKey {
    clip: String,
    frames: Vec<String>,
    seconds_per_frame: f32,
    looping: bool,
}

impl AnimationTool {
    pub fn reset(&mut self) {
        self.palette.invalidate();
        self.selected = None;
        self.rename.clear();
        self.previewing = false;
        self.reset_playback();
    }

    /// Keeps the selected clip valid as clips are added, renamed, or removed.
    pub fn selected<'a>(&'a mut self, animation: &'a SpriteAnimationComponent) -> Option<&'a str> {
        if self
            .selected
            .as_ref()
            .is_none_or(|name| !animation.clips.contains_key(name))
        {
            self.selected = animation
                .playing
                .as_ref()
                .filter(|name| animation.clips.contains_key(*name))
                .cloned()
                .or_else(|| animation.clips.keys().next().cloned());
            self.rename = self.selected.clone().unwrap_or_default();
            self.reset_playback();
        }
        self.selected.as_deref()
    }

    pub fn select(&mut self, clip: String) {
        if self.selected.as_ref() != Some(&clip) {
            self.selected = Some(clip);
            self.rename = self.selected.clone().unwrap_or_default();
            self.reset_playback();
        }
    }

    pub fn renamed(&mut self, clip: String) {
        self.selected = Some(clip.clone());
        self.rename = clip;
        self.reset_playback();
    }

    pub fn rename(&mut self) -> &mut String {
        &mut self.rename
    }

    pub const fn previewing(&self) -> bool {
        self.previewing
    }

    pub fn set_previewing(&mut self, previewing: bool) {
        if self.previewing != previewing {
            self.reset_playback();
        }
        self.previewing = previewing;
    }

    pub const fn frame(&self) -> usize {
        self.frame
    }

    /// Advances the editor-only preview and returns its current frame.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn advance(&mut self, clip_name: &str, clip: &AnimationClip, seconds: f32) -> usize {
        let key = PreviewKey {
            clip: clip_name.to_owned(),
            frames: clip.frames.clone(),
            seconds_per_frame: clip.seconds_per_frame,
            looping: clip.looping,
        };
        if self.preview_key.as_ref() != Some(&key) {
            self.preview_key = Some(key);
            self.frame = 0;
            self.elapsed = 0.0;
        }
        if !self.previewing
            || clip.frames.is_empty()
            || !clip.seconds_per_frame.is_finite()
            || clip.seconds_per_frame <= 0.0
            || !seconds.is_finite()
            || seconds <= 0.0
        {
            return self.frame;
        }
        self.elapsed += seconds;
        let steps = (self.elapsed / clip.seconds_per_frame) as usize;
        self.elapsed %= clip.seconds_per_frame;
        if steps == 0 {
            return self.frame;
        }
        if clip.looping {
            self.frame = (self.frame + steps % clip.frames.len()) % clip.frames.len();
        } else if steps >= clip.frames.len() - self.frame {
            self.frame = clip.frames.len() - 1;
            self.previewing = false;
            self.elapsed = 0.0;
        } else {
            self.frame += steps;
        }
        self.frame
    }

    pub fn reset_playback(&mut self) {
        self.frame = 0;
        self.elapsed = 0.0;
        self.preview_key = None;
    }
}

/// Adds a valid one-frame clip, choosing a name that does not collide.
pub fn add_clip(payload: &mut Value, first_sprite: &str) -> Result<String, String> {
    let clips = clips_mut(payload)?;
    let mut suffix = 1_u32;
    let name = loop {
        let candidate = if suffix == 1 {
            "clip".to_owned()
        } else {
            format!("clip {suffix}")
        };
        if !clips.contains_key(&candidate) {
            break candidate;
        }
        suffix = suffix.saturating_add(1);
    };
    clips.insert(
        name.clone(),
        json!({
            "frames": [first_sprite],
            "seconds_per_frame": 0.1,
            "looping": true
        }),
    );
    Ok(name)
}

pub fn rename_clip(payload: &mut Value, old: &str, new: &str) -> Result<bool, String> {
    let new = new.trim();
    if new.is_empty() {
        return Err("a clip needs a name".to_owned());
    }
    if old == new {
        return Ok(false);
    }
    let clips = clips_mut(payload)?;
    if clips.contains_key(new) {
        return Err(format!("a clip named {new:?} already exists"));
    }
    let clip = clips
        .remove(old)
        .ok_or_else(|| format!("no clip named {old:?}"))?;
    clips.insert(new.to_owned(), clip);
    if payload.get("playing").and_then(Value::as_str) == Some(old) {
        payload["playing"] = Value::String(new.to_owned());
    }
    Ok(true)
}

pub fn remove_clip(payload: &mut Value, name: &str) -> Result<bool, String> {
    let removed = clips_mut(payload)?.remove(name).is_some();
    if removed && payload.get("playing").and_then(Value::as_str) == Some(name) {
        payload["playing"] = Value::Null;
    }
    Ok(removed)
}

pub fn push_frame(payload: &mut Value, clip: &str, sprite: &str) -> Result<(), String> {
    frames_mut(payload, clip)?.push(Value::String(sprite.to_owned()));
    Ok(())
}

pub fn set_frame(
    payload: &mut Value,
    clip: &str,
    index: usize,
    sprite: &str,
) -> Result<(), String> {
    let frames = frames_mut(payload, clip)?;
    let frame = frames
        .get_mut(index)
        .ok_or_else(|| format!("frame {index} is outside clip {clip:?}"))?;
    *frame = Value::String(sprite.to_owned());
    Ok(())
}

pub fn remove_frame(payload: &mut Value, clip: &str, index: usize) -> Result<bool, String> {
    let frames = frames_mut(payload, clip)?;
    if frames.len() <= 1 || index >= frames.len() {
        return Ok(false);
    }
    frames.remove(index);
    Ok(true)
}

pub fn move_frame(
    payload: &mut Value,
    clip: &str,
    index: usize,
    direction: isize,
) -> Result<bool, String> {
    let frames = frames_mut(payload, clip)?;
    let Some(target) = index.checked_add_signed(direction) else {
        return Ok(false);
    };
    if index >= frames.len() || target >= frames.len() {
        return Ok(false);
    }
    frames.swap(index, target);
    Ok(true)
}

fn clips_mut(payload: &mut Value) -> Result<&mut Map<String, Value>, String> {
    payload
        .get_mut("clips")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "animation clips are not an object".to_owned())
}

fn frames_mut<'a>(payload: &'a mut Value, clip: &str) -> Result<&'a mut Vec<Value>, String> {
    clips_mut(payload)?
        .get_mut(clip)
        .and_then(|clip| clip.get_mut("frames"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("clip {clip:?} has no frame list"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation() -> Value {
        json!({
            "clips": {
                "idle": {
                    "frames": ["idle"],
                    "seconds_per_frame": 0.2,
                    "looping": true
                }
            },
            "playing": "idle",
            "speed": 1.0
        })
    }

    #[test]
    fn clips_can_be_added_renamed_and_removed_without_dangling_playing() {
        let mut payload = animation();
        let added = add_clip(&mut payload, "walk-0").unwrap();
        assert_eq!(added, "clip");
        assert_eq!(payload["clips"]["clip"]["frames"], json!(["walk-0"]));

        payload["playing"] = Value::String(added.clone());
        assert!(rename_clip(&mut payload, &added, "walk").unwrap());
        assert_eq!(payload["playing"], "walk");
        assert!(remove_clip(&mut payload, "walk").unwrap());
        assert_eq!(payload["playing"], Value::Null);
    }

    #[test]
    fn frames_are_appended_reordered_replaced_and_never_emptied() {
        let mut payload = animation();
        push_frame(&mut payload, "idle", "blink").unwrap();
        assert!(move_frame(&mut payload, "idle", 1, -1).unwrap());
        set_frame(&mut payload, "idle", 0, "blink-open").unwrap();
        assert!(remove_frame(&mut payload, "idle", 1).unwrap());
        assert!(!remove_frame(&mut payload, "idle", 0).unwrap());
        assert_eq!(payload["clips"]["idle"]["frames"], json!(["blink-open"]));
    }

    #[test]
    fn a_non_looping_preview_stops_on_its_last_frame() {
        let clip = AnimationClip {
            frames: vec!["a".to_owned(), "b".to_owned()],
            seconds_per_frame: 0.1,
            looping: false,
        };
        let mut tool = AnimationTool::default();
        tool.set_previewing(true);
        assert_eq!(tool.advance("once", &clip, 0.1), 1);
        assert_eq!(tool.advance("once", &clip, 0.1), 1);
        assert!(!tool.previewing());

        tool.set_previewing(true);
        assert_eq!(tool.advance("once", &clip, 0.0), 0);
    }

    #[test]
    fn a_large_preview_delta_skips_frames_without_work_per_frame() {
        let clip = AnimationClip {
            frames: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            seconds_per_frame: 1.0,
            looping: true,
        };
        let mut tool = AnimationTool::default();
        tool.set_previewing(true);

        assert_eq!(tool.advance("loop", &clip, 10_000.0), 1);
        assert!(tool.previewing());
    }
}
