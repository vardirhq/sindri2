use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    Created,
    Initialized,
    Running,
    Paused,
    Stopped,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EngineLifecycle {
    state: EngineState,
}

impl Default for EngineLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineLifecycle {
    pub const fn new() -> Self {
        Self {
            state: EngineState::Created,
        }
    }

    pub const fn state(self) -> EngineState {
        self.state
    }

    pub fn initialize(&mut self) -> Result<(), LifecycleError> {
        self.transition(&[EngineState::Created], EngineState::Initialized)
    }

    pub fn start(&mut self) -> Result<(), LifecycleError> {
        self.transition(
            &[EngineState::Initialized, EngineState::Stopped],
            EngineState::Running,
        )
    }

    pub fn pause(&mut self) -> Result<(), LifecycleError> {
        self.transition(&[EngineState::Running], EngineState::Paused)
    }

    pub fn resume(&mut self) -> Result<(), LifecycleError> {
        self.transition(&[EngineState::Paused], EngineState::Running)
    }

    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        self.transition(
            &[EngineState::Running, EngineState::Paused],
            EngineState::Stopped,
        )
    }

    pub fn destroy(&mut self) -> Result<(), LifecycleError> {
        if self.state == EngineState::Destroyed {
            return Err(LifecycleError::InvalidTransition {
                from: self.state,
                to: EngineState::Destroyed,
            });
        }
        self.state = EngineState::Destroyed;
        Ok(())
    }

    fn transition(
        &mut self,
        allowed: &[EngineState],
        next: EngineState,
    ) -> Result<(), LifecycleError> {
        if !allowed.contains(&self.state) {
            return Err(LifecycleError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LifecycleError {
    #[error("invalid engine lifecycle transition from {from:?} to {to:?}")]
    InvalidTransition { from: EngineState, to: EngineState },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_enforces_transitions() {
        let mut lifecycle = EngineLifecycle::new();
        assert!(lifecycle.start().is_err());
        lifecycle.initialize().unwrap();
        lifecycle.start().unwrap();
        lifecycle.pause().unwrap();
        lifecycle.resume().unwrap();
        lifecycle.stop().unwrap();
        lifecycle.destroy().unwrap();
        assert_eq!(lifecycle.state(), EngineState::Destroyed);
    }
}
