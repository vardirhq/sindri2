use std::time::Duration;

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedStepConfig {
    pub step: Duration,
    pub max_frame_delta: Duration,
    pub max_steps_per_frame: u32,
}

impl Default for FixedStepConfig {
    fn default() -> Self {
        Self {
            step: Duration::from_secs_f64(1.0 / 60.0),
            max_frame_delta: Duration::from_millis(250),
            max_steps_per_frame: 8,
        }
    }
}

/// A rational simulation time multiplier.
///
/// The scale is rational rather than floating point so that scaling cannot
/// accumulate drift. [`FixedStepClock`] carries the division remainder between
/// frames, so total simulated time always matches the exact ratio of total real
/// time no matter how many frames elapse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeScale {
    numerator: u32,
    denominator: u32,
}

impl Default for TimeScale {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl TimeScale {
    /// Real time: simulation advances exactly as fast as the host clock.
    pub const NORMAL: Self = Self {
        numerator: 1,
        denominator: 1,
    };
    /// Frozen time: frames still arrive, but simulation does not advance.
    ///
    /// This differs from pausing the clock, which stops delivering frames.
    pub const FROZEN: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    pub fn new(numerator: u32, denominator: u32) -> Result<Self, TimeError> {
        if denominator == 0 {
            return Err(TimeError::ZeroTimeScaleDenominator);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    pub const fn is_normal(self) -> bool {
        self.numerator == self.denominator
    }

    pub const fn is_frozen(self) -> bool {
        self.numerator == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSteps {
    /// Simulated time this frame, after the time scale is applied.
    pub frame_delta: Duration,
    /// Real time this frame, after clamping but before the time scale.
    ///
    /// Interface animation and profiling use this so they keep running at real
    /// speed while the simulation is slowed, frozen, or fast-forwarded.
    pub real_delta: Duration,
    pub fixed_delta: Duration,
    pub fixed_steps: u32,
    pub interpolation_alpha: f64,
}

#[derive(Clone, Debug)]
pub struct FixedStepClock {
    config: FixedStepConfig,
    accumulator: Duration,
    elapsed: Duration,
    paused: bool,
    scale: TimeScale,
    /// Nanoseconds of scaled time not yet handed out, carried between frames so
    /// repeated scaling stays exact.
    scale_remainder: u128,
}

impl FixedStepClock {
    pub fn new(config: FixedStepConfig) -> Result<Self, TimeError> {
        if config.step.is_zero() {
            return Err(TimeError::ZeroStep);
        }
        if config.max_steps_per_frame == 0 {
            return Err(TimeError::ZeroMaxSteps);
        }
        Ok(Self {
            config,
            accumulator: Duration::ZERO,
            elapsed: Duration::ZERO,
            paused: false,
            scale: TimeScale::NORMAL,
            scale_remainder: 0,
        })
    }

    pub fn advance(&mut self, real_delta: Duration) -> FrameSteps {
        if self.paused {
            return FrameSteps {
                frame_delta: Duration::ZERO,
                real_delta: Duration::ZERO,
                fixed_delta: self.config.step,
                fixed_steps: 0,
                interpolation_alpha: self.accumulator.as_secs_f64()
                    / self.config.step.as_secs_f64(),
            };
        }

        let real_delta = real_delta.min(self.config.max_frame_delta);
        let frame_delta = self.scaled(real_delta);
        self.elapsed += frame_delta;
        self.accumulator += frame_delta;

        let available = u32::try_from(self.accumulator.as_nanos() / self.config.step.as_nanos())
            .unwrap_or(u32::MAX);
        let fixed_steps = available.min(self.config.max_steps_per_frame);
        self.accumulator = self
            .accumulator
            .saturating_sub(self.config.step * fixed_steps);

        if available > self.config.max_steps_per_frame {
            self.accumulator = self.accumulator.min(self.config.step);
        }

        FrameSteps {
            frame_delta,
            real_delta,
            fixed_delta: self.config.step,
            fixed_steps,
            interpolation_alpha: self.accumulator.as_secs_f64() / self.config.step.as_secs_f64(),
        }
    }

    /// Applies the time scale using integer arithmetic.
    ///
    /// The remainder of the division is kept rather than discarded, so a long
    /// run of frames simulates exactly `real * numerator / denominator` instead
    /// of losing a fraction of a nanosecond per frame.
    fn scaled(&mut self, real_delta: Duration) -> Duration {
        if self.scale.is_normal() {
            return real_delta;
        }
        let denominator = u128::from(self.scale.denominator());
        let total =
            real_delta.as_nanos() * u128::from(self.scale.numerator()) + self.scale_remainder;
        self.scale_remainder = total % denominator;
        Duration::from_nanos(u64::try_from(total / denominator).unwrap_or(u64::MAX))
    }

    pub const fn time_scale(&self) -> TimeScale {
        self.scale
    }

    pub const fn set_time_scale(&mut self, scale: TimeScale) {
        self.scale = scale;
    }

    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TimeError {
    #[error("fixed simulation step must be greater than zero")]
    ZeroStep,
    #[error("maximum fixed steps per frame must be greater than zero")]
    ZeroMaxSteps,
    #[error("time scale denominator must be greater than zero")]
    ZeroTimeScaleDenominator,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_caps_spiral_of_death() {
        let mut clock = FixedStepClock::new(FixedStepConfig {
            step: Duration::from_millis(10),
            max_frame_delta: Duration::from_secs(1),
            max_steps_per_frame: 3,
        })
        .unwrap();

        let frame = clock.advance(Duration::from_millis(100));
        assert_eq!(frame.fixed_steps, 3);
        assert!(frame.interpolation_alpha <= 1.0);
    }

    #[test]
    fn time_scale_does_not_accumulate_drift() {
        let mut clock = FixedStepClock::new(FixedStepConfig::default()).unwrap();
        clock.set_time_scale(TimeScale::new(1, 3).unwrap());

        // A third of 16ms is not representable in nanoseconds, so a naive
        // multiply loses a fraction every frame.
        let frame = Duration::from_millis(16);
        let frames = 10_000_u128;
        for _ in 0..frames {
            clock.advance(frame);
        }

        let exact = u64::try_from(frames * frame.as_nanos() / 3).unwrap();
        assert_eq!(clock.elapsed(), Duration::from_nanos(exact));
    }

    #[test]
    fn time_scale_slows_and_accelerates_simulated_time() {
        let mut clock = FixedStepClock::new(FixedStepConfig::default()).unwrap();
        clock.set_time_scale(TimeScale::new(1, 2).unwrap());
        let frame = clock.advance(Duration::from_millis(20));
        assert_eq!(frame.real_delta, Duration::from_millis(20));
        assert_eq!(frame.frame_delta, Duration::from_millis(10));

        clock.set_time_scale(TimeScale::new(3, 1).unwrap());
        let frame = clock.advance(Duration::from_millis(20));
        assert_eq!(frame.real_delta, Duration::from_millis(20));
        assert_eq!(frame.frame_delta, Duration::from_millis(60));
    }

    #[test]
    fn frozen_time_still_delivers_frames() {
        let mut clock = FixedStepClock::new(FixedStepConfig::default()).unwrap();
        clock.set_time_scale(TimeScale::FROZEN);
        let frame = clock.advance(Duration::from_millis(16));

        // Unlike pausing, the host still gets a frame with real time on it.
        assert_eq!(frame.real_delta, Duration::from_millis(16));
        assert_eq!(frame.frame_delta, Duration::ZERO);
        assert_eq!(frame.fixed_steps, 0);
        assert_eq!(clock.elapsed(), Duration::ZERO);
    }

    #[test]
    fn a_zero_time_scale_denominator_is_rejected() {
        assert_eq!(
            TimeScale::new(1, 0),
            Err(TimeError::ZeroTimeScaleDenominator)
        );
    }

    #[test]
    fn paused_clock_does_not_advance() {
        let mut clock = FixedStepClock::new(FixedStepConfig::default()).unwrap();
        clock.set_paused(true);
        let frame = clock.advance(Duration::from_secs(2));
        assert_eq!(frame.frame_delta, Duration::ZERO);
        assert_eq!(clock.elapsed(), Duration::ZERO);
    }
}
