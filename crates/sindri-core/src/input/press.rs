//! One pointing interaction, and the vocabulary describing it.
//!
//! The reasoning for the shape of this type is in the module header next door.

use std::time::Duration;

/// The kind of thing doing the pointing.
///
/// Kept apart from the identity because behaviour differs by kind and not by
/// instance: a mouse hovers and a finger cannot, a pen may report pressure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum PointerDevice {
    Mouse,
    Touch,
    Pen,
}

impl PointerDevice {
    /// Whether this kind of device can be somewhere without pressing.
    ///
    /// A mouse rests over things; a finger that is not touching the screen is
    /// not anywhere at all. Interface code that lights an element under the
    /// pointer needs the difference, and a game that treats a finger as a
    /// hovering mouse gets a cursor stuck wherever the last tap was.
    #[must_use]
    pub const fn hovers(self) -> bool {
        matches!(self, Self::Mouse | Self::Pen)
    }
}

/// One interaction, told apart from every other live one.
///
/// The raw number is the host's: a finger's identifier, or the button for a
/// mouse. It is only ever compared, never interpreted, so a host may use
/// whatever its platform gives it as long as it is stable for the life of the
/// press and not reused while that press lives.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PressId {
    device: PointerDevice,
    raw: u64,
}

impl PressId {
    #[must_use]
    pub const fn new(device: PointerDevice, raw: u64) -> Self {
        Self { device, raw }
    }

    #[must_use]
    pub const fn device(self) -> PointerDevice {
        self.device
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.raw
    }
}

/// Where a press is in its life.
///
/// A press is `Began` for exactly the frame it arrives and `Ended` for exactly
/// the frame it leaves, so an edge is read once however many events a host sent
/// in between. `Cancelled` is the system taking the interaction away -- a
/// gesture claimed by the browser, a window losing focus mid-drag -- and is
/// kept apart from `Ended` because a cancelled press should not complete a
/// click, while an ended one is precisely how a click completes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PressPhase {
    Began,
    Held,
    Ended,
    Cancelled,
}

impl PressPhase {
    /// Whether the press is still down.
    #[must_use]
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Began | Self::Held)
    }

    /// Whether this is the last frame of the press, however it finished.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Ended | Self::Cancelled)
    }
}

/// One pointing interaction, from the frame it starts to the frame it stops.
///
/// `position` is meaningful in every phase. On the final frame it is where the
/// press finished, which is the whole point of the type -- see the module
/// header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    id: PressId,
    phase: PressPhase,
    origin: [f32; 2],
    position: [f32; 2],
    previous: [f32; 2],
    held_for: Duration,
}

impl Press {
    /// Starts a press where it arrived.
    ///
    /// The origin and the position are the same point, and stay the same until
    /// something moves.
    #[must_use]
    pub const fn began(id: PressId, at: [f32; 2]) -> Self {
        Self {
            id,
            phase: PressPhase::Began,
            origin: at,
            position: at,
            previous: at,
            held_for: Duration::ZERO,
        }
    }

    #[must_use]
    pub const fn id(&self) -> PressId {
        self.id
    }

    #[must_use]
    pub const fn phase(&self) -> PressPhase {
        self.phase
    }

    /// Where the press began.
    #[must_use]
    pub const fn origin(&self) -> [f32; 2] {
        self.origin
    }

    /// Where the press is, or -- once it has finished -- where it finished.
    #[must_use]
    pub const fn position(&self) -> [f32; 2] {
        self.position
    }

    /// How long the press has been down.
    ///
    /// What separates a tap from a long press, and it is measured rather than
    /// counted in frames so the answer does not change with the frame rate.
    #[must_use]
    pub const fn held_for(&self) -> Duration {
        self.held_for
    }

    /// How far the press moved since the previous frame.
    #[must_use]
    pub fn delta(&self) -> [f32; 2] {
        [
            self.position[0] - self.previous[0],
            self.position[1] - self.previous[1],
        ]
    }

    /// How far the press is from where it began.
    ///
    /// What separates a tap from a drag: a finger never lands perfectly still,
    /// so the question is not whether it moved but whether it moved enough.
    #[must_use]
    pub fn travel(&self) -> f32 {
        let dx = self.position[0] - self.origin[0];
        let dy = self.position[1] - self.origin[1];
        dx.hypot(dy)
    }

    /// Moves the press, without changing what it has been through.
    pub const fn move_to(&mut self, position: [f32; 2]) {
        self.position = position;
    }

    /// Ends the press, leaving it where it was.
    ///
    /// Takes no position: a host reporting a finger lifting rarely says where,
    /// and the answer is the last place it was seen. A press that ends
    /// somewhere new is a move followed by an end.
    pub const fn finish(&mut self, phase: PressPhase) {
        self.phase = phase;
    }

    /// Carries the press into the next frame.
    ///
    /// The edge phases are spent here -- `Began` becomes `Held` -- and the
    /// current position becomes the one deltas are measured from.
    pub const fn advance(&mut self, delta: Duration) {
        if let PressPhase::Began = self.phase {
            self.phase = PressPhase::Held;
        }
        self.previous = self.position;
        self.held_for = self.held_for.saturating_add(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::{PointerDevice, Press, PressId, PressPhase};
    use std::time::Duration;

    fn id() -> PressId {
        PressId::new(PointerDevice::Touch, 1)
    }

    /// Positions are compared by nearness, not equality: they are arrived at
    /// by arithmetic, and the repository lints exact float comparison for the
    /// reason that arithmetic does not promise exact answers.
    #[track_caller]
    fn assert_at(actual: [f32; 2], expected: [f32; 2]) {
        assert!(
            (actual[0] - expected[0]).abs() < f32::EPSILON
                && (actual[1] - expected[1]).abs() < f32::EPSILON,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn a_press_that_has_ended_still_says_where_it_ended() {
        // The property the whole type exists for. A press completes on the
        // frame it ends, and completing one asks where it is; a finger is out
        // of the host's live set by then, so anything that reads the device
        // rather than the press finds nothing.
        let mut press = Press::began(id(), [10.0, 20.0]);
        press.move_to([12.0, 26.0]);
        press.finish(PressPhase::Ended);

        assert!(press.phase().is_final());
        assert_at(press.position(), [12.0, 26.0]);
        assert_at(press.origin(), [10.0, 20.0]);
    }

    #[test]
    fn a_press_begins_once_however_long_it_is_held() {
        let mut press = Press::began(id(), [0.0, 0.0]);
        assert_eq!(press.phase(), PressPhase::Began);

        press.advance(Duration::from_millis(16));
        assert_eq!(press.phase(), PressPhase::Held);
        press.advance(Duration::from_millis(16));
        assert_eq!(press.phase(), PressPhase::Held);
    }

    #[test]
    fn holding_is_measured_in_time_rather_than_frames() {
        // Counted in frames, a long press would need a different number on a
        // 60 Hz phone and a 144 Hz monitor.
        let mut press = Press::began(id(), [0.0, 0.0]);
        press.advance(Duration::from_millis(10));
        press.advance(Duration::from_millis(30));
        assert_eq!(press.held_for(), Duration::from_millis(40));
    }

    #[test]
    fn travel_is_from_the_origin_and_delta_is_from_last_frame() {
        let mut press = Press::began(id(), [0.0, 0.0]);
        press.move_to([3.0, 4.0]);
        assert!((press.travel() - 5.0).abs() < f32::EPSILON);
        assert_at(press.delta(), [3.0, 4.0]);

        press.advance(Duration::from_millis(16));
        press.move_to([3.0, 8.0]);
        assert!(
            (press.travel() - 73.0_f32.sqrt()).abs() < 1e-5,
            "travel was {}",
            press.travel()
        );
        assert_at(press.delta(), [0.0, 4.0]);
    }

    #[test]
    fn a_cancelled_press_is_finished_but_is_not_an_ending() {
        // A click completes on an ending. A press the system took away should
        // not complete one, which is why the two are different phases.
        let mut press = Press::began(id(), [0.0, 0.0]);
        press.finish(PressPhase::Cancelled);
        assert!(press.phase().is_final());
        assert!(!press.phase().is_live());
        assert_ne!(press.phase(), PressPhase::Ended);
    }

    #[test]
    fn only_a_device_that_rests_somewhere_can_hover() {
        assert!(PointerDevice::Mouse.hovers());
        assert!(PointerDevice::Pen.hovers());
        assert!(!PointerDevice::Touch.hovers());
    }
}
