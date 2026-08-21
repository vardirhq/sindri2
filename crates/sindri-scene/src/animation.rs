//! Playing a sprite sheet.
//!
//! A sprite draws part of a texture; an animation decides *which* part, frame by
//! frame. The two are deliberately separate components on one entity: the sprite
//! still owns the texture, tint, space, anchor, and layer, and the animation owns
//! nothing but the sheet's grid, the clips cut from it, and which clip is
//! playing. The legacy engine's `AnimatedSprite` duplicated every sprite field,
//! and the copy is how a tint set on one of them stops being the tint that draws.
//!
//! Where playback *is* matters as much as what it is. A clip and its timing are
//! authored, so they live in the scene; the frame a sprite happens to be on
//! halfway through a run is runtime state, so it does not. Keeping the cursor out
//! of the component is what stops watching an animation play from rewriting the
//! file it came from — a scene saved mid-run has to be the scene that was opened.
//! [`SpriteAnimations`] is that cursor, held beside the world exactly as
//! [`crate::TextureBindings`] holds the other half of a texture reference.

use std::collections::BTreeMap;

use serde::Deserialize;
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, EntityId, SceneComponent, World,
};
use sindri_render::UvRectError;
use thiserror::Error;

/// A named run of sprites through the sheet.
///
/// Every frame lasts the same time. A pose that should be held longer is written
/// by repeating its name — `["idle", "idle", "idle", "step"]` holds the first for
/// three frames — which is how sheet tools express it anyway, and is one way of
/// saying it rather than two that can disagree.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AnimationClip {
    /// Sprite names from the sheet, in the order they play.
    ///
    /// Names and not cell numbers: a name survives a re-slice that moves the
    /// cell, and a number does not. It is also the reason a clip no longer
    /// carries a grid — where each sprite is belongs to the sheet beside the
    /// image, said once.
    pub frames: Vec<String>,
    pub seconds_per_frame: f32,
    /// Whether the clip starts again at the end, rather than holding its last
    /// frame. Looping is the default because most clips do.
    #[serde(default = "looping_by_default")]
    pub looping: bool,
}

const fn looping_by_default() -> bool {
    true
}

/// The clips cut from an entity's sprite sheet, and which one is playing.
///
/// Sits alongside `sindri.sprite` on the same entity; the sprite names the sheet
/// texture, this says how to read it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpriteAnimationComponent {
    pub clips: BTreeMap<String, AnimationClip>,
    /// The clip playing, or nothing — in which case the sprite draws whatever
    /// its own reference names, which is what an entity with clips authored but
    /// none selected should look like rather than an arbitrary frame.
    #[serde(default)]
    pub playing: Option<String>,
    /// A multiplier on time, so one clip can run at half speed without a second
    /// copy of it with doubled durations. Zero holds the current frame.
    #[serde(default = "normal_speed")]
    pub speed: f32,
}

const fn normal_speed() -> f32 {
    1.0
}

impl SpriteAnimationComponent {
    /// The clip playing, checked, or `None` when nothing is.
    ///
    /// Checked here rather than at deserialization for the same reason a sprite's
    /// rect is: a scene with a broken clip still has to open, because the editor
    /// is where it gets fixed.
    pub fn playing_clip(&self) -> Result<Option<(&str, &AnimationClip)>, AnimationError> {
        let Some(name) = self.playing.as_deref() else {
            return Ok(None);
        };
        let clip = self
            .clips
            .get(name)
            .ok_or_else(|| AnimationError::UnknownClip(name.to_owned()))?;
        if clip.frames.is_empty() {
            return Err(AnimationError::EmptyClip(name.to_owned()));
        }
        if !clip.seconds_per_frame.is_finite() || clip.seconds_per_frame <= 0.0 {
            return Err(AnimationError::FrameHasNoDuration {
                clip: name.to_owned(),
                seconds: clip.seconds_per_frame,
            });
        }
        if !self.speed.is_finite() || self.speed < 0.0 {
            return Err(AnimationError::BadSpeed(self.speed));
        }
        Ok(Some((name, clip)))
    }

    /// The sprite this shows before anything has advanced it.
    ///
    /// A sheet is one texture, so a sprite carrying an animation that no
    /// cursor has reached yet — a scene just loaded, an entity sitting in the
    /// editor outside play mode, a frame captured before the first tick —
    /// would otherwise draw every cell of the sheet squeezed into one quad.
    /// The playing clip's first frame is the only honest answer, and it is the
    /// one the first tick agrees with.
    pub fn resting_sprite(&self) -> Result<Option<&str>, AnimationError> {
        let Some((_, clip)) = self.playing_clip()? else {
            return Ok(None);
        };
        Self::frame_sprite(clip, 0).map(Some)
    }

    /// The sprite a clip's frame draws, by its position in the clip.
    pub fn frame_sprite(clip: &AnimationClip, frame: usize) -> Result<&str, AnimationError> {
        clip.frames
            .get(frame)
            .map(String::as_str)
            .ok_or(AnimationError::OutsideClip { frame })
    }
}

impl SceneComponent for SpriteAnimationComponent {
    const TYPE_NAME: &'static str = "sindri.sprite_animation";
}

/// Where every animated sprite in a world currently is.
///
/// Runtime state, held beside the world rather than in it: advancing an
/// animation must not dirty the scene it came from.
#[derive(Clone, Debug, Default)]
pub struct SpriteAnimations {
    playback: BTreeMap<EntityId, Playback>,
}

#[derive(Clone, Debug)]
struct Playback {
    clip: String,
    frame: usize,
    /// Time accumulated towards the next frame, always less than one frame's
    /// worth once a step has been taken.
    elapsed: f32,
    finished: bool,
    sprite: String,
}

impl SpriteAnimations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves every animated sprite in `world` on by `delta_seconds`.
    ///
    /// Call it once per frame with the frame's own delta, and not at all while
    /// paused — holding still is not advancing by zero, it is not advancing.
    ///
    /// An entity whose animation is removed, or which is despawned, loses its
    /// cursor here: the cursors that survive a call are exactly the ones the
    /// world still justifies.
    pub fn advance(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
        delta_seconds: f32,
    ) -> Result<(), AnimationError> {
        if !delta_seconds.is_finite() || delta_seconds < 0.0 {
            return Err(AnimationError::BadDelta(delta_seconds));
        }
        let mut live = BTreeMap::new();
        for (entity, animation) in components.query::<SpriteAnimationComponent>(world)? {
            let Some((name, clip)) = animation.playing_clip()? else {
                continue;
            };
            let mut playback = match self.playback.remove(&entity) {
                // A clip change restarts rather than resuming at whatever frame
                // number the last one had reached, which would be a frame of the
                // new clip chosen by the length of the old one.
                Some(playback) if playback.clip == name => playback,
                _ => Playback {
                    clip: name.to_owned(),
                    frame: 0,
                    elapsed: 0.0,
                    finished: false,
                    sprite: SpriteAnimationComponent::frame_sprite(clip, 0)?.to_owned(),
                },
            };
            step(&mut playback, clip, delta_seconds * animation.speed);
            SpriteAnimationComponent::frame_sprite(clip, playback.frame)?
                .clone_into(&mut playback.sprite);
            live.insert(entity, playback);
        }
        self.playback = live;
        Ok(())
    }

    /// The sprite an entity's animation is showing, or `None` when it has none
    /// — in which case whatever the sprite's own reference names stands.
    ///
    /// A name rather than a rect, because playback is where in a clip an entity
    /// has got to and not where that lands on an image. The rect is resolved
    /// where the sheets are already in hand, during extraction.
    pub fn sprite(&self, entity: EntityId) -> Option<&str> {
        self.playback
            .get(&entity)
            .map(|playback| playback.sprite.as_str())
    }

    /// Which frame of its clip an entity is on, counted within the clip rather
    /// than as a sprite of the sheet.
    pub fn frame(&self, entity: EntityId) -> Option<usize> {
        self.playback.get(&entity).map(|playback| playback.frame)
    }

    /// Whether a non-looping clip has reached its end and is holding there.
    pub fn is_finished(&self, entity: EntityId) -> bool {
        self.playback
            .get(&entity)
            .is_some_and(|playback| playback.finished)
    }

    /// Plays an entity's clip again from its first frame.
    ///
    /// Which clip plays is authored, so switching clips is a change to the
    /// world; playing the same one again is not, and a clip that has finished
    /// otherwise has no way back to its start.
    pub fn restart(&mut self, entity: EntityId) {
        if let Some(playback) = self.playback.get_mut(&entity) {
            playback.frame = 0;
            playback.elapsed = 0.0;
            playback.finished = false;
        }
    }
}

/// Advances one cursor.
///
/// Arithmetic rather than a loop per frame: a long stall with a short frame time
/// would otherwise spin for as many iterations as frames elapsed, and a stall is
/// exactly when that is least affordable.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn step(playback: &mut Playback, clip: &AnimationClip, seconds: f32) {
    if playback.finished || seconds <= 0.0 {
        return;
    }
    playback.elapsed += seconds;
    let steps = (playback.elapsed / clip.seconds_per_frame).floor();
    if steps < 1.0 {
        return;
    }
    playback.elapsed -= steps * clip.seconds_per_frame;
    // Saturating, because a clip's length bounds what a step can mean: with
    // looping only the remainder matters, and without it anything past the end
    // is the end.
    let steps = if steps >= f64::from(u32::MAX) as f32 {
        usize::MAX
    } else {
        steps as usize
    };
    let reached = playback.frame.saturating_add(steps);
    if reached < clip.frames.len() {
        playback.frame = reached;
    } else if clip.looping {
        playback.frame = reached % clip.frames.len();
    } else {
        playback.frame = clip.frames.len() - 1;
        playback.elapsed = 0.0;
        playback.finished = true;
    }
}

#[derive(Debug, Error)]
pub enum AnimationError {
    #[error("no clip named {0:?} is authored on this sprite")]
    UnknownClip(String),
    #[error("clip {0:?} has no frames to play")]
    EmptyClip(String),
    #[error("clip {clip:?} gives each frame {seconds} seconds, which is not a length of time")]
    FrameHasNoDuration { clip: String, seconds: f32 },
    #[error("an animation speed must be finite and forwards, and this one is {0}")]
    BadSpeed(f32),
    #[error("an animation step must be finite and forwards, and this one is {0} seconds")]
    BadDelta(f32),
    #[error("frame {frame} is past the end of the clip")]
    OutsideClip { frame: usize },
    #[error(transparent)]
    Rect(#[from] UvRectError),
    #[error(transparent)]
    Component(#[from] ComponentRegistryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk() -> SpriteAnimationComponent {
        SpriteAnimationComponent {
            clips: BTreeMap::from([(
                "walk".to_owned(),
                AnimationClip {
                    frames: vec![
                        "0".to_owned(),
                        "1".to_owned(),
                        "2".to_owned(),
                        "3".to_owned(),
                    ],
                    seconds_per_frame: 0.1,
                    looping: true,
                },
            )]),
            playing: Some("walk".to_owned()),
            speed: 1.0,
        }
    }

    fn cursor() -> Playback {
        Playback {
            clip: "walk".to_owned(),
            frame: 0,
            elapsed: 0.0,
            finished: false,
            sprite: "0".to_owned(),
        }
    }

    #[test]
    fn a_clip_advances_one_frame_per_its_own_duration() {
        let animation = walk();
        let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
        let mut playback = cursor();

        step(&mut playback, clip, 0.05);
        assert_eq!(playback.frame, 0, "half a frame is not a frame");
        step(&mut playback, clip, 0.05);
        assert_eq!(playback.frame, 1, "and the other half is");
        step(&mut playback, clip, 0.25);
        assert_eq!(
            playback.frame, 3,
            "two and a half more frames land on three"
        );
    }

    /// A stall does not spin: the step is arithmetic, so a delta covering a
    /// million frames costs the same as one covering two.
    #[test]
    fn a_looping_clip_wraps_however_far_time_jumped() {
        let animation = walk();
        let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
        let mut playback = cursor();

        step(&mut playback, clip, 0.4);
        assert_eq!(playback.frame, 0, "four frames is a whole loop");
        step(&mut playback, clip, 100_000.0);
        assert_eq!(
            playback.frame, 0,
            "and a million frames is a whole lot of them"
        );
        assert!(!playback.finished, "a looping clip never finishes");
    }

    #[test]
    fn a_clip_that_does_not_loop_holds_its_last_frame() {
        let mut animation = walk();
        animation
            .clips
            .get_mut("walk")
            .expect("walk is authored")
            .looping = false;
        let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
        let mut playback = cursor();

        step(&mut playback, clip, 10.0);
        assert_eq!(playback.frame, 3, "the end of the clip is where it stops");
        assert!(playback.finished);
        step(&mut playback, clip, 10.0);
        assert_eq!(playback.frame, 3, "and it stays stopped");
    }

    #[test]
    fn no_time_passing_moves_nothing() {
        let animation = walk();
        let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
        let mut playback = cursor();

        step(&mut playback, clip, 0.0);
        assert_eq!(playback.frame, 0);
        assert_eq!(
            playback.elapsed.to_bits(),
            0.0_f32.to_bits(),
            "and no time is banked towards the next frame either"
        );
    }

    /// Every way of authoring a clip that cannot be played, named rather than
    /// approximated.
    #[test]
    fn a_clip_that_cannot_be_played_says_why() {
        let mut animation = walk();
        animation.playing = Some("run".to_owned());
        assert!(matches!(
            animation.playing_clip(),
            Err(AnimationError::UnknownClip(_))
        ));

        let mut animation = walk();
        animation
            .clips
            .get_mut("walk")
            .expect("walk is authored")
            .frames
            .clear();
        assert!(matches!(
            animation.playing_clip(),
            Err(AnimationError::EmptyClip(_))
        ));

        let mut animation = walk();
        animation
            .clips
            .get_mut("walk")
            .expect("walk is authored")
            .seconds_per_frame = 0.0;
        assert!(matches!(
            animation.playing_clip(),
            Err(AnimationError::FrameHasNoDuration { .. })
        ));

        let mut animation = walk();
        animation.speed = -1.0;
        assert!(matches!(
            animation.playing_clip(),
            Err(AnimationError::BadSpeed(_))
        ));

        let mut animation = walk();
        animation.playing = None;
        assert!(
            animation.playing_clip().unwrap().is_none(),
            "and nothing playing is not a failure"
        );
    }

    #[test]
    fn a_frame_names_the_sprite_its_clip_names() {
        let animation = SpriteAnimationComponent {
            clips: BTreeMap::from([(
                "walk".to_owned(),
                AnimationClip {
                    frames: vec!["lift".to_owned(), "plant".to_owned()],
                    seconds_per_frame: 0.1,
                    looping: true,
                },
            )]),
            ..walk()
        };
        let (_, clip) = animation.playing_clip().unwrap().expect("walk is playing");
        assert_eq!(
            SpriteAnimationComponent::frame_sprite(clip, 0).unwrap(),
            "lift",
            "a clip's frames are sprites of the sheet, in the order it names them"
        );
        assert_eq!(
            SpriteAnimationComponent::frame_sprite(clip, 1).unwrap(),
            "plant"
        );
        assert!(matches!(
            SpriteAnimationComponent::frame_sprite(clip, 2),
            Err(AnimationError::OutsideClip { .. })
        ));
    }
}
