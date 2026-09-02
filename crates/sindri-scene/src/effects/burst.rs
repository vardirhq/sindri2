//! `sindri.effect.burst`: what one throw of flecks looks like.

use serde::Deserialize;
use sindri_core::{SceneComponent, SpriteRef, SpriteRefError};

/// An authored burst of flecks.
///
/// On an entity rather than passed to a call, because these are a designer's
/// numbers — how many, how fast, how big, what colour — and a script that had to
/// name all of them would be a script nobody could read. A bullet fires
/// `Effects.burst(this.entity)` and the scene decides what that looks like.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct EffectBurstComponent {
    /// The image every fleck in this burst draws.
    ///
    /// One reference for the whole burst, so a burst is one batch. A fleck is
    /// cheap because nothing about it varies per fleck except its motion and
    /// its colour, and a texture that varied would cost a draw call.
    pub texture: String,
    /// How many flecks one burst throws.
    #[serde(default = "default_count")]
    pub count: u32,
    /// How fast they leave, in world units per second.
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// How much that speed varies, as a fraction of it.
    ///
    /// A burst where every fleck moves at exactly one speed reads as a ring
    /// rather than a spray, which is sometimes what is wanted and usually not.
    #[serde(default = "default_spread")]
    pub spread: f32,
    /// How long a fleck lives, in seconds.
    #[serde(default = "default_lifetime")]
    pub lifetime: f32,
    /// How big a fleck is, in world units.
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default = "opaque_white")]
    pub tint: [f32; 4],
    /// Whether a fleck fades out as it dies.
    ///
    /// On by default: a fleck that vanishes at full brightness pops, and a
    /// burst of them pops together.
    #[serde(default = "yes")]
    pub fade: bool,
    /// How much of its speed a fleck keeps each second.
    ///
    /// Below one it slows, which is what makes a spray settle rather than fly
    /// off the screen at a constant rate.
    #[serde(default = "default_drag")]
    pub drag: f32,
    #[serde(default)]
    pub layer: i32,
}

const fn default_count() -> u32 {
    12
}

const fn default_speed() -> f32 {
    4.0
}

const fn default_spread() -> f32 {
    0.5
}

const fn default_lifetime() -> f32 {
    0.5
}

const fn default_size() -> f32 {
    0.1
}

const fn default_drag() -> f32 {
    0.25
}

const fn yes() -> bool {
    true
}

const fn opaque_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

impl EffectBurstComponent {
    /// The image these flecks draw, checked the way every other reference is.
    pub fn reference(&self) -> Result<SpriteRef, SpriteRefError> {
        SpriteRef::parse(&self.texture)
    }
}

impl SceneComponent for EffectBurstComponent {
    const TYPE_NAME: &'static str = "sindri.effect.burst";
}
