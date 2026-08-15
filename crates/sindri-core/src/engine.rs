use std::time::Duration;

use thiserror::Error;

use crate::{
    EngineLifecycle, EngineState, FixedStepClock, FixedStepConfig, FrameSteps, LifecycleError,
    TimeError, TimeScale, World,
};

/// Platform-independent state owned by every Sindri runtime host.
///
/// Desktop and browser hosts are responsible for their own event loops. They
/// advance this shared core with measured frame deltas.
#[derive(Clone, Debug)]
pub struct EngineCore {
    lifecycle: EngineLifecycle,
    clock: FixedStepClock,
    world: World,
}

impl EngineCore {
    pub fn new(time: FixedStepConfig) -> Result<Self, EngineError> {
        Ok(Self {
            lifecycle: EngineLifecycle::new(),
            clock: FixedStepClock::new(time)?,
            world: World::default(),
        })
    }

    pub const fn state(&self) -> EngineState {
        self.lifecycle.state()
    }

    pub const fn world(&self) -> &World {
        &self.world
    }

    pub const fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Total simulated time since the engine started, after time scaling.
    pub const fn elapsed(&self) -> Duration {
        self.clock.elapsed()
    }

    pub const fn time_scale(&self) -> TimeScale {
        self.clock.time_scale()
    }

    pub const fn set_time_scale(&mut self, scale: TimeScale) {
        self.clock.set_time_scale(scale);
    }

    pub fn initialize(&mut self) -> Result<(), EngineError> {
        self.lifecycle.initialize()?;
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), EngineError> {
        self.lifecycle.start()?;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), EngineError> {
        self.lifecycle.pause()?;
        self.clock.set_paused(true);
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), EngineError> {
        self.lifecycle.resume()?;
        self.clock.set_paused(false);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), EngineError> {
        self.lifecycle.stop()?;
        Ok(())
    }

    pub fn destroy(&mut self) -> Result<(), EngineError> {
        self.lifecycle.destroy()?;
        Ok(())
    }

    pub fn advance(&mut self, real_delta: Duration) -> Result<EngineFrame, EngineError> {
        if !matches!(self.state(), EngineState::Running | EngineState::Paused) {
            return Err(EngineError::NotActive(self.state()));
        }
        Ok(EngineFrame {
            time: self.clock.advance(real_delta),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineFrame {
    pub time: FrameSteps,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum EngineError {
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    #[error(transparent)]
    Time(#[from] TimeError),
    #[error("engine must be running or paused to advance; current state is {0:?}")]
    NotActive(EngineState),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_advances_only_an_active_engine() {
        let mut engine = EngineCore::new(FixedStepConfig::default()).unwrap();
        assert!(matches!(
            engine.advance(Duration::from_millis(16)),
            Err(EngineError::NotActive(EngineState::Created))
        ));
        engine.initialize().unwrap();
        engine.start().unwrap();
        assert!(engine.advance(Duration::from_millis(16)).is_ok());
    }
}
