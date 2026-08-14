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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSteps {
    pub frame_delta: Duration,
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
        })
    }

    pub fn advance(&mut self, real_delta: Duration) -> FrameSteps {
        if self.paused {
            return FrameSteps {
                frame_delta: Duration::ZERO,
                fixed_delta: self.config.step,
                fixed_steps: 0,
                interpolation_alpha: self.accumulator.as_secs_f64()
                    / self.config.step.as_secs_f64(),
            };
        }

        let frame_delta = real_delta.min(self.config.max_frame_delta);
        self.elapsed += frame_delta;
        self.accumulator += frame_delta;

        let available = u32::try_from(
            self.accumulator.as_nanos() / self.config.step.as_nanos(),
        )
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
            fixed_delta: self.config.step,
            fixed_steps,
            interpolation_alpha: self.accumulator.as_secs_f64()
                / self.config.step.as_secs_f64(),
        }
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
    fn paused_clock_does_not_advance() {
        let mut clock = FixedStepClock::new(FixedStepConfig::default()).unwrap();
        clock.set_paused(true);
        let frame = clock.advance(Duration::from_secs(2));
        assert_eq!(frame.frame_delta, Duration::ZERO);
        assert_eq!(clock.elapsed(), Duration::ZERO);
    }
}
