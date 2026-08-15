use std::{fmt, time::Duration};

use sindri_core::{EngineCore, EngineError, EngineState, FixedStepConfig, FrameSteps, World};
use thiserror::Error;

use crate::{InputEvent, InputState};

/// The point in a frame at which gameplay code ran.
///
/// Carried on failures so a host can say which hook failed rather than only
/// that something did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramePhase {
    Start,
    FixedUpdate,
    Update,
    Stop,
}

impl fmt::Display for FramePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Start => "start",
            Self::FixedUpdate => "fixed update",
            Self::Update => "update",
            Self::Stop => "stop",
        };
        formatter.write_str(name)
    }
}

/// Timing for a single gameplay call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameTime {
    /// Simulated time this call covers. Inside a fixed update this is always
    /// the configured fixed step.
    pub delta: Duration,
    /// Real time this frame, unaffected by the time scale. Interface animation
    /// uses this so it keeps moving while the simulation is slowed or frozen.
    pub real_delta: Duration,
    /// Total simulated time since the engine started.
    pub elapsed: Duration,
    /// How far the frame ended into the next fixed step, from zero to one.
    pub interpolation_alpha: f64,
}

/// Everything gameplay code may touch during one call.
pub struct FrameContext<'a> {
    pub world: &'a mut World,
    pub input: &'a InputState,
    pub time: FrameTime,
}

/// Gameplay logic driven by a host.
///
/// Every hook may fail, and a failure stops the frame and reaches the host
/// rather than being swallowed. All hooks have a default so a game only writes
/// the ones it needs.
pub trait Game {
    type Error: std::error::Error + 'static;

    /// Runs once when the engine starts, before any update.
    fn start(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let _ = context;
        Ok(())
    }

    /// Runs at the fixed simulation rate, zero or more times per frame.
    ///
    /// Physics and anything that must be frame-rate independent belongs here.
    fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let _ = context;
        Ok(())
    }

    /// Runs exactly once per frame, after any fixed updates.
    fn update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let _ = context;
        Ok(())
    }

    /// Runs once when the engine stops.
    fn stop(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
        let _ = context;
        Ok(())
    }
}

/// Drives an [`EngineCore`], input, and a [`Game`] as one loop.
///
/// The host owns no window, surface, or clock. A platform adapter feeds it
/// input events and frame deltas, which is what lets the same loop run on a
/// desktop, in a browser, and in a test with no windowing at all.
#[derive(Clone, Debug)]
pub struct EngineHost<G: Game> {
    engine: EngineCore,
    input: InputState,
    game: G,
}

impl<G: Game> EngineHost<G> {
    /// Creates a host with an initialized engine, ready to start.
    pub fn new(game: G, time: FixedStepConfig) -> Result<Self, HostError<G::Error>> {
        let mut engine = EngineCore::new(time)?;
        engine.initialize()?;
        Ok(Self {
            engine,
            input: InputState::default(),
            game,
        })
    }

    pub const fn game(&self) -> &G {
        &self.game
    }

    pub const fn game_mut(&mut self) -> &mut G {
        &mut self.game
    }

    pub const fn world(&self) -> &World {
        self.engine.world()
    }

    pub fn world_mut(&mut self) -> &mut World {
        self.engine.world_mut()
    }

    pub const fn engine(&self) -> &EngineCore {
        &self.engine
    }

    pub const fn engine_mut(&mut self) -> &mut EngineCore {
        &mut self.engine
    }

    pub const fn input(&self) -> &InputState {
        &self.input
    }

    pub const fn state(&self) -> EngineState {
        self.engine.state()
    }

    /// Records a host input event for the next frame.
    pub fn queue_input(&mut self, event: InputEvent) {
        self.input.apply(event);
    }

    pub fn start(&mut self) -> Result<(), HostError<G::Error>> {
        self.engine.start()?;
        self.call(FramePhase::Start, Duration::ZERO, Duration::ZERO, 0.0)
    }

    pub fn pause(&mut self) -> Result<(), HostError<G::Error>> {
        self.engine.pause()?;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), HostError<G::Error>> {
        self.engine.resume()?;
        Ok(())
    }

    /// Stops the engine, giving gameplay a final call first.
    pub fn stop(&mut self) -> Result<(), HostError<G::Error>> {
        self.call(FramePhase::Stop, Duration::ZERO, Duration::ZERO, 0.0)?;
        self.engine.stop()?;
        Ok(())
    }

    /// Advances one frame: any pending fixed updates, then a single update.
    ///
    /// Input edges are cleared afterwards even when gameplay fails, so a
    /// recoverable error cannot leave a press to be seen a second time.
    pub fn advance(&mut self, real_delta: Duration) -> Result<FrameSteps, HostError<G::Error>> {
        let steps = self.engine.advance(real_delta)?.time;
        let result = self.run_frame(steps);
        self.input.begin_frame();
        result.map(|()| steps)
    }

    fn run_frame(&mut self, steps: FrameSteps) -> Result<(), HostError<G::Error>> {
        for _ in 0..steps.fixed_steps {
            self.call(
                FramePhase::FixedUpdate,
                steps.fixed_delta,
                steps.real_delta,
                steps.interpolation_alpha,
            )?;
        }
        self.call(
            FramePhase::Update,
            steps.frame_delta,
            steps.real_delta,
            steps.interpolation_alpha,
        )
    }

    fn call(
        &mut self,
        phase: FramePhase,
        delta: Duration,
        real_delta: Duration,
        interpolation_alpha: f64,
    ) -> Result<(), HostError<G::Error>> {
        let time = FrameTime {
            delta,
            real_delta,
            elapsed: self.engine.elapsed(),
            interpolation_alpha,
        };
        let mut context = FrameContext {
            world: self.engine.world_mut(),
            input: &self.input,
            time,
        };
        let outcome = match phase {
            FramePhase::Start => self.game.start(&mut context),
            FramePhase::FixedUpdate => self.game.fixed_update(&mut context),
            FramePhase::Update => self.game.update(&mut context),
            FramePhase::Stop => self.game.stop(&mut context),
        };
        outcome.map_err(|source| HostError::Game { phase, source })
    }
}

#[derive(Debug, Error)]
pub enum HostError<E: std::error::Error + 'static> {
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("game logic failed during {phase}")]
    Game {
        phase: FramePhase,
        #[source]
        source: E,
    },
}
