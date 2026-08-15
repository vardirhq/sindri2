use std::time::Duration;

use sindri_platform::Clock;
use web_time::Instant;

/// The monotonic clock the windowed host reads.
///
/// `sindri-platform`'s `SystemClock` is native-only, because `std::time::Instant`
/// has no meaning in a browser. This host runs on a desktop and in a browser
/// through the same `winit` backend, so it needs one clock that works on both:
/// `web_time::Instant` is `std::time::Instant` natively and `performance.now()`
/// on `wasm32`, which keeps the host free of a target conditional around
/// something as fundamental as what time it is.
#[derive(Clone, Copy, Debug)]
pub struct WindowClock {
    start: Instant,
}

impl Default for WindowClock {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Clock for WindowClock {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use sindri_platform::FrameTimer;

    use super::*;

    #[test]
    fn a_window_clock_never_reports_a_negative_frame() {
        // The point of reading a clock rather than measuring deltas is that a
        // frame cannot come out negative however the readings land.
        let clock = WindowClock::new();
        let mut timer = FrameTimer::new();
        for _ in 0..64 {
            let delta = timer.tick(&clock);
            assert!(delta < Duration::from_secs(1));
        }
    }
}
