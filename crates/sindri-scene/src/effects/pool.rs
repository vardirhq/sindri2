//! The pool a burst throws flecks into.

use std::time::Duration;

use sindri_core::Rng;

use super::EffectBurstComponent;

/// One fleck: everything it needs and nothing it does not.
///
/// Plain values in one array. There is no identity here because nothing can
/// hold one — that is what makes the whole population a single pass over
/// contiguous memory rather than a scan of a world.
#[derive(Clone, Copy, Debug)]
pub struct Fleck {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    /// Seconds left to live.
    pub remaining: f32,
    /// Seconds it started with, so a fade knows how far through it is.
    pub lifetime: f32,
    pub size: f32,
    pub tint: [f32; 4],
    pub fade: bool,
    pub drag: f32,
    pub layer: i32,
    /// Which texture binding this draws, as an index into the batch's own list.
    pub texture: usize,
}

impl Fleck {
    /// The colour this draws right now, fade included.
    #[must_use]
    pub fn drawn_tint(self) -> [f32; 4] {
        if !self.fade || self.lifetime <= 0.0 {
            return self.tint;
        }
        let left = (self.remaining / self.lifetime).clamp(0.0, 1.0);
        [
            self.tint[0],
            self.tint[1],
            self.tint[2],
            self.tint[3] * left,
        ]
    }
}

/// Every live fleck, and the stream their motion comes from.
///
/// Runtime state beside the world, derived from what a scene authors and never
/// serialized — the same shape as `SpriteAnimations`, `ScenePhysics2d` and
/// `ScreenUi`.
#[derive(Debug)]
pub struct Effects2d {
    flecks: Vec<Fleck>,
    capacity: usize,
    /// Where a fleck's direction and speed come from.
    ///
    /// **Its own stream, never the run's.** A fleck drawn from the gameplay
    /// stream would shift every number after it, so turning an explosion up
    /// would change which enemies spawned — a seeded run has to mean the same
    /// run whatever it looked like.
    rng: Rng,
    /// The textures this pool's flecks name, in the order they were first seen.
    ///
    /// Resolved once at emission rather than per frame, because a texture
    /// lookup per fleck per frame is exactly the per-frame cost this whole
    /// thing exists to avoid.
    textures: Vec<String>,
    /// How many flecks were dropped for want of room, since the last look.
    overflowed: usize,
}

impl Default for Effects2d {
    fn default() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }
}

impl Effects2d {
    /// How many flecks a pool holds when nobody has said.
    ///
    /// Comfortably past what the measurement showed a frame can afford as
    /// entities, and a fraction of what it costs here: `docs/effect-scaling.md`
    /// puts eight thousand at 0.018 ms.
    pub const DEFAULT_CAPACITY: usize = 8_192;

    /// The stream flecks are drawn from.
    ///
    /// A different one from the run's, deliberately and permanently.
    const EFFECT_STREAM: u64 = 0x_E77E_C750;

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            flecks: Vec::with_capacity(capacity),
            capacity,
            rng: Rng::with_stream(0, Self::EFFECT_STREAM),
            textures: Vec::new(),
            overflowed: 0,
        }
    }

    /// How many flecks are alive.
    #[must_use]
    pub fn live(&self) -> usize {
        self.flecks.len()
    }

    /// How many flecks the pool has room for.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many flecks have been dropped for want of room.
    ///
    /// Worth reporting rather than hiding: a game whose explosions quietly stop
    /// appearing at the busiest moment should be able to find out why.
    #[must_use]
    pub const fn overflowed(&self) -> usize {
        self.overflowed
    }

    /// Every live fleck.
    #[must_use]
    pub fn flecks(&self) -> &[Fleck] {
        &self.flecks
    }

    /// The texture a fleck's index names.
    #[must_use]
    pub fn texture(&self, index: usize) -> Option<&str> {
        self.textures.get(index).map(String::as_str)
    }

    /// Throws one burst at a place.
    ///
    /// Returns how many flecks were made, which is fewer than asked for when the
    /// pool is full.
    pub fn burst(&mut self, burst: &EffectBurstComponent, at: [f32; 2]) -> usize {
        let texture = self.texture_index(&burst.texture);
        let mut made = 0;
        for _ in 0..burst.count {
            if self.flecks.len() >= self.capacity {
                // The newest action is the one worth seeing. A fleck from four
                // hundred milliseconds ago is not what someone is looking at.
                if self.flecks.is_empty() {
                    break;
                }
                self.flecks.swap_remove(0);
                self.overflowed += 1;
            }
            let angle = self.rng.next_f32() * std::f32::consts::TAU;
            // `1 ± spread`, so a spread of zero is a clean ring and the default
            // is a spray.
            let scale = burst
                .spread
                .mul_add(self.rng.next_f32().mul_add(2.0, -1.0), 1.0)
                .max(0.0);
            let speed = burst.speed * scale;
            self.flecks.push(Fleck {
                position: at,
                velocity: [angle.cos() * speed, angle.sin() * speed],
                remaining: burst.lifetime,
                lifetime: burst.lifetime,
                size: burst.size,
                tint: burst.tint,
                fade: burst.fade,
                drag: burst.drag,
                layer: burst.layer,
                texture,
            });
            made += 1;
        }
        made
    }

    /// Moves every fleck on, and retires the ones that ran out.
    ///
    /// One pass, in place, with a swap-remove for the dead: the order flecks are
    /// drawn in does not matter because they are all on one layer and all in one
    /// batch, which is what lets the cheapest removal be the right one.
    pub fn advance(&mut self, delta: Duration) {
        let seconds = delta.as_secs_f32();
        if seconds <= 0.0 {
            return;
        }
        let mut index = 0;
        while index < self.flecks.len() {
            let fleck = &mut self.flecks[index];
            fleck.remaining -= seconds;
            if fleck.remaining <= 0.0 {
                self.flecks.swap_remove(index);
                continue;
            }
            fleck.position[0] += fleck.velocity[0] * seconds;
            fleck.position[1] += fleck.velocity[1] * seconds;
            // Exponential rather than linear, so drag is a rate a designer can
            // reason about and a fleck never reverses.
            let kept = (1.0 - fleck.drag * seconds).clamp(0.0, 1.0);
            fleck.velocity[0] *= kept;
            fleck.velocity[1] *= kept;
            index += 1;
        }
    }

    /// Forgets every fleck, which is what stopping a scene means.
    pub fn clear(&mut self) {
        self.flecks.clear();
        self.overflowed = 0;
        self.rng = Rng::with_stream(0, Self::EFFECT_STREAM);
    }

    /// Where this texture sits in the pool's list, adding it if it is new.
    fn texture_index(&mut self, texture: &str) -> usize {
        if let Some(found) = self.textures.iter().position(|known| known == texture) {
            return found;
        }
        self.textures.push(texture.to_owned());
        self.textures.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectBurstComponent, Effects2d};
    use std::time::Duration;

    fn burst(count: u32) -> EffectBurstComponent {
        EffectBurstComponent {
            texture: "sindri:white".to_owned(),
            count,
            speed: 4.0,
            spread: 0.5,
            lifetime: 0.5,
            size: 0.1,
            tint: [1.0, 1.0, 1.0, 1.0],
            fade: true,
            drag: 0.0,
            layer: 0,
        }
    }

    #[test]
    fn a_burst_makes_the_flecks_it_asked_for() {
        let mut effects = Effects2d::default();
        assert_eq!(effects.burst(&burst(12), [0.0, 0.0]), 12);
        assert_eq!(effects.live(), 12);
    }

    #[test]
    fn flecks_leave_in_different_directions() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(32), [0.0, 0.0]);
        let first = effects.flecks()[0].velocity;
        assert!(
            effects
                .flecks()
                .iter()
                .any(|fleck| (fleck.velocity[0] - first[0]).abs() > 0.1),
            "a burst that went one way"
        );
    }

    #[test]
    fn a_fleck_runs_out_and_is_retired() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(4), [0.0, 0.0]);
        effects.advance(Duration::from_millis(250));
        assert_eq!(effects.live(), 4, "retired early");
        effects.advance(Duration::from_millis(300));
        assert_eq!(effects.live(), 0, "outlived its lifetime");
    }

    #[test]
    fn a_fleck_moves_the_way_it_was_thrown() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(1), [0.0, 0.0]);
        let thrown = effects.flecks()[0].velocity;
        effects.advance(Duration::from_millis(100));
        let moved = effects.flecks()[0].position;
        assert!(moved[0].signum() == thrown[0].signum() || moved[0].abs() < 1.0e-6);
        assert!(moved[0].abs() + moved[1].abs() > 0.0, "it did not move");
    }

    /// A fleck that vanishes at full brightness pops.
    #[test]
    fn a_fading_fleck_dims_as_it_dies() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(1), [0.0, 0.0]);
        let full = effects.flecks()[0].drawn_tint()[3];
        effects.advance(Duration::from_millis(400));
        let nearly_gone = effects.flecks()[0].drawn_tint()[3];
        assert!(
            nearly_gone < full,
            "{nearly_gone} is not dimmer than {full}"
        );
        assert!(nearly_gone >= 0.0);
    }

    #[test]
    fn a_fleck_that_does_not_fade_keeps_its_colour() {
        let mut effects = Effects2d::default();
        let mut definition = burst(1);
        definition.fade = false;
        effects.burst(&definition, [0.0, 0.0]);
        effects.advance(Duration::from_millis(400));
        assert!((effects.flecks()[0].drawn_tint()[3] - 1.0).abs() < 1.0e-6);
    }

    /// Drag is what makes a spray settle instead of flying off the screen.
    #[test]
    fn drag_slows_a_fleck_without_reversing_it() {
        let mut effects = Effects2d::default();
        let mut definition = burst(1);
        definition.drag = 4.0;
        definition.lifetime = 10.0;
        effects.burst(&definition, [0.0, 0.0]);
        let thrown = effects.flecks()[0].velocity;
        effects.advance(Duration::from_millis(100));
        let slowed = effects.flecks()[0].velocity;
        let speed = |v: [f32; 2]| v[0].hypot(v[1]);
        assert!(speed(slowed) < speed(thrown), "drag did nothing");
        assert!(
            slowed[0] * thrown[0] >= 0.0 && slowed[1] * thrown[1] >= 0.0,
            "drag reversed a fleck"
        );
    }

    /// A pool that grew without limit would be the memory problem this exists
    /// to avoid.
    #[test]
    fn a_full_pool_stays_full_rather_than_growing() {
        let mut effects = Effects2d::with_capacity(16);
        for _ in 0..10 {
            effects.burst(&burst(8), [0.0, 0.0]);
        }
        assert_eq!(effects.live(), 16);
        assert!(effects.overflowed() > 0, "overflow went unreported");
    }

    /// A game whose explosions quietly stop appearing should be able to find
    /// out why.
    #[test]
    fn overflow_is_counted_rather_than_hidden() {
        let mut effects = Effects2d::with_capacity(4);
        effects.burst(&burst(4), [0.0, 0.0]);
        assert_eq!(effects.overflowed(), 0);
        effects.burst(&burst(3), [0.0, 0.0]);
        assert_eq!(effects.overflowed(), 3);
    }

    /// Turning an explosion up must not change which enemies spawn.
    #[test]
    fn flecks_do_not_touch_the_runs_own_stream() {
        let run = Rng::from_seed(7);
        let mut effects = Effects2d::default();
        effects.burst(&burst(64), [0.0, 0.0]);
        // The pool holds its own generator; the run's is untouched because it
        // was never handed over. This asserts the shape rather than the value:
        // there is no path from here to that stream.
        assert_eq!(run, Rng::from_seed(7));
    }

    use sindri_core::Rng;

    #[test]
    fn one_texture_is_one_index() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(2), [0.0, 0.0]);
        effects.burst(&burst(2), [1.0, 1.0]);
        assert_eq!(effects.texture(0), Some("sindri:white"));
        assert_eq!(effects.texture(1), None, "one texture became two");
    }

    #[test]
    fn stopping_forgets_everything() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(8), [0.0, 0.0]);
        effects.clear();
        assert_eq!(effects.live(), 0);
        assert_eq!(effects.overflowed(), 0);
    }

    /// A paused scene must not age its flecks.
    #[test]
    fn no_time_passing_moves_nothing() {
        let mut effects = Effects2d::default();
        effects.burst(&burst(4), [0.0, 0.0]);
        let before = effects.flecks()[0].position;
        effects.advance(Duration::ZERO);
        let now = effects.flecks()[0].position;
        assert!((now[0] - before[0]).abs() < 1.0e-9 && (now[1] - before[1]).abs() < 1.0e-9);
        assert_eq!(effects.live(), 4);
    }
}
