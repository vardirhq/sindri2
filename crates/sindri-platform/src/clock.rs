use std::time::Duration;

/// A monotonic time source.
///
/// Hosts differ in where time comes from — `Instant` natively,
/// `performance.now()` in a browser, a scripted value in a test — so the loop
/// depends on this rather than on any one of them.
pub trait Clock {
    /// Time elapsed since this clock started. Never decreases.
    fn elapsed(&self) -> Duration;
}

/// A clock driven entirely by the caller.
///
/// This is the deterministic test host: a suite can run thousands of frames of
/// an exact length with no sleeping and no dependence on wall-clock scheduling.
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    elapsed: Duration,
}

impl ManualClock {
    pub const fn new() -> Self {
        Self {
            elapsed: Duration::ZERO,
        }
    }

    pub const fn advance(&mut self, by: Duration) {
        self.elapsed = self.elapsed.saturating_add(by);
    }
}

impl Clock for ManualClock {
    fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

/// A clock backed by the operating system's monotonic timer.
///
/// Not available on `wasm32`, where `Instant` has no meaning; browser hosts
/// supply their own [`Clock`] over `performance.now()`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub struct SystemClock {
    start: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// Turns absolute clock readings into per-frame deltas.
///
/// Hosts read a clock rather than measure deltas themselves, so a delta can
/// never be negative and a paused or backgrounded host cannot produce one
/// enormous catch-up frame from a stale timestamp.
#[derive(Clone, Debug, Default)]
pub struct FrameTimer {
    last: Option<Duration>,
}

impl FrameTimer {
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// The time since the previous tick. The first tick reports zero.
    pub fn tick(&mut self, clock: &impl Clock) -> Duration {
        let now = clock.elapsed();
        let delta = self
            .last
            .map_or(Duration::ZERO, |last| now.saturating_sub(last));
        self.last = Some(now);
        delta
    }

    /// Forgets the previous reading so the next tick reports zero.
    ///
    /// Hosts call this when resuming after a pause, so time spent stopped is
    /// not delivered as one huge frame.
    pub const fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manual_clock_only_moves_when_told_to() {
        let mut clock = ManualClock::new();
        assert_eq!(clock.elapsed(), Duration::ZERO);
        clock.advance(Duration::from_millis(16));
        clock.advance(Duration::from_millis(16));
        assert_eq!(clock.elapsed(), Duration::from_millis(32));
    }

    #[test]
    fn the_first_frame_has_no_delta() {
        let mut clock = ManualClock::new();
        let mut timer = FrameTimer::new();
        clock.advance(Duration::from_secs(5));
        assert_eq!(timer.tick(&clock), Duration::ZERO);

        clock.advance(Duration::from_millis(16));
        assert_eq!(timer.tick(&clock), Duration::from_millis(16));
    }

    #[test]
    fn resetting_drops_time_spent_stopped() {
        let mut clock = ManualClock::new();
        let mut timer = FrameTimer::new();
        timer.tick(&clock);
        clock.advance(Duration::from_millis(16));
        assert_eq!(timer.tick(&clock), Duration::from_millis(16));

        clock.advance(Duration::from_secs(30));
        timer.reset();
        assert_eq!(timer.tick(&clock), Duration::ZERO);
    }
}
