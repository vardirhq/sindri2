//! What presses turn out to mean.
//!
//! A tap, a long press and a drag are the same events told apart by how far a
//! press wandered and how long it lasted. Every game re-derives them from raw
//! touches otherwise, and each one picks its own thresholds, so the same finger
//! is a tap in one screen and a drag in the next.
//!
//! Recognition is deliberately conservative about what it will claim. A press
//! that has moved is never a tap afterwards, however still it becomes; a press
//! that was taken away is nothing at all. Both follow from the same principle
//! the press model does: say only what actually happened.

use std::collections::BTreeMap;
use std::time::Duration;

use super::press::{Press, PressId, PressPhase};
use super::set::Presses;

/// What a press turned out to be.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Gesture {
    /// A press that arrived, stayed still, and left again.
    Tap { at: [f32; 2] },
    /// A press that stayed still and stayed down. Reported once, while the
    /// finger is still there, because that is when it is useful: waiting for
    /// the release would be a different gesture.
    LongPress { at: [f32; 2] },
    /// A press that is moving, reported every frame it moves.
    Drag {
        from: [f32; 2],
        to: [f32; 2],
        delta: [f32; 2],
    },
    /// Two presses moving together or apart, as a ratio of the distance
    /// between them since the previous frame.
    ///
    /// A ratio rather than a distance because that is what a zoom wants: a
    /// factor is the same gesture whether the fingers started an inch apart or
    /// a hand's width.
    Pinch { scale: f32, centre: [f32; 2] },
}

/// How far and how long a press may go and still be each thing.
///
/// Authorable because the answer is not universal: a stylus is steadier than a
/// thumb, and a game played at arm's length wants more slack than one played
/// on a desk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureLimits {
    /// How far a press may wander, in the units positions are reported in, and
    /// still be a tap rather than a drag.
    ///
    /// Not zero: a finger never lands perfectly still, and a tap that demanded
    /// it would fail for everyone.
    pub slop: f32,
    /// How long a press may last and still be a tap.
    pub tap: Duration,
    /// How long a still press must last to be a long press.
    pub long_press: Duration,
}

impl Default for GestureLimits {
    fn default() -> Self {
        Self {
            slop: 10.0,
            tap: Duration::from_millis(400),
            long_press: Duration::from_millis(500),
        }
    }
}

/// How far along one press is in being recognised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Progress {
    /// Still still, and still short: it could yet be anything.
    Undecided,
    /// It moved past the slop, so it is a drag and can never be a tap.
    Dragging,
    /// It was held still long enough, and has been reported as such.
    LongPressed,
}

/// Recognises gestures from presses, across frames.
///
/// Holds the little that recognition needs to remember: which presses have
/// already become something, so a drag is not also a tap and a long press is
/// reported once rather than every frame after it qualifies.
#[derive(Clone, Debug, Default)]
pub struct Gestures {
    limits: GestureLimits,
    progress: BTreeMap<PressId, Progress>,
    recognised: Vec<Gesture>,
    /// The distance between two presses last frame, for a pinch.
    spread: Option<f32>,
}

impl Gestures {
    #[must_use]
    pub fn new(limits: GestureLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn limits(&self) -> GestureLimits {
        self.limits
    }

    /// Reads this frame's presses, replacing what was recognised last frame.
    pub fn update(&mut self, presses: &Presses) {
        self.recognised.clear();
        // Kept while the press is still in the set at all, not only while it
        // is live: the frame it ends is the frame its history decides what it
        // was, and dropping the history there would make every drag end as a
        // tap.
        self.progress.retain(|id, _| presses.get(*id).is_some());

        for press in presses.iter() {
            self.read(press);
        }
        self.read_pinch(presses);
    }

    /// What was recognised this frame.
    pub fn iter(&self) -> impl Iterator<Item = &Gesture> {
        self.recognised.iter()
    }

    /// Where a tap happened this frame, if one did.
    #[must_use]
    pub fn tap(&self) -> Option<[f32; 2]> {
        self.recognised.iter().find_map(|gesture| match gesture {
            Gesture::Tap { at } => Some(*at),
            _ => None,
        })
    }

    /// Where a long press happened this frame, if one did.
    #[must_use]
    pub fn long_press(&self) -> Option<[f32; 2]> {
        self.recognised.iter().find_map(|gesture| match gesture {
            Gesture::LongPress { at } => Some(*at),
            _ => None,
        })
    }

    /// How far the drag moved this frame, if one is happening.
    #[must_use]
    pub fn drag(&self) -> Option<[f32; 2]> {
        self.recognised.iter().find_map(|gesture| match gesture {
            Gesture::Drag { delta, .. } => Some(*delta),
            _ => None,
        })
    }

    /// How much two fingers spread this frame, if they are.
    #[must_use]
    pub fn pinch(&self) -> Option<f32> {
        self.recognised.iter().find_map(|gesture| match gesture {
            Gesture::Pinch { scale, .. } => Some(*scale),
            _ => None,
        })
    }

    fn read(&mut self, press: &Press) {
        let progress = *self
            .progress
            .entry(press.id())
            .or_insert(Progress::Undecided);

        // Past the slop it is a drag, and stays one: a finger that wandered
        // and then stopped did not turn back into a tap.
        if progress != Progress::Dragging && press.travel() > self.limits.slop {
            self.progress.insert(press.id(), Progress::Dragging);
            self.drag_from(press);
            return;
        }
        if progress == Progress::Dragging {
            if press.phase().is_live() {
                self.drag_from(press);
            }
            return;
        }

        match press.phase() {
            PressPhase::Began | PressPhase::Held => {
                if progress == Progress::Undecided && press.held_for() >= self.limits.long_press {
                    self.progress.insert(press.id(), Progress::LongPressed);
                    self.recognised.push(Gesture::LongPress {
                        at: press.position(),
                    });
                }
            }
            PressPhase::Ended => {
                // A long press already reported is not also a tap: the person
                // was told something happened, and letting go is the end of it
                // rather than a second thing.
                if progress == Progress::Undecided && press.held_for() <= self.limits.tap {
                    self.recognised.push(Gesture::Tap {
                        at: press.position(),
                    });
                }
            }
            // Taken away rather than let go, so it was nothing.
            PressPhase::Cancelled => {}
        }
    }

    fn drag_from(&mut self, press: &Press) {
        self.recognised.push(Gesture::Drag {
            from: press.origin(),
            to: press.position(),
            delta: press.delta(),
        });
    }

    /// Two live presses moving together or apart.
    ///
    /// Exactly two: with one there is nothing to measure against, and with
    /// three a pinch is not what the person is doing.
    fn read_pinch(&mut self, presses: &Presses) {
        let live: Vec<&Press> = presses.iter().filter(|p| p.phase().is_live()).collect();
        let [first, second] = live[..] else {
            self.spread = None;
            return;
        };

        let (a, b) = (first.position(), second.position());
        let spread = (a[0] - b[0]).hypot(a[1] - b[1]);
        let previous = self.spread.replace(spread);

        // Nothing to compare against on the first frame of the pair, and a
        // pinch that started from nothing would report an infinite factor.
        let Some(previous) = previous else { return };
        if previous <= f32::EPSILON {
            return;
        }
        self.recognised.push(Gesture::Pinch {
            scale: spread / previous,
            centre: [f32::midpoint(a[0], b[0]), f32::midpoint(a[1], b[1])],
        });
    }
}
