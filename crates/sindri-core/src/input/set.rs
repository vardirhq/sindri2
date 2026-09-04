//! Every press happening at once, and the one question most readers ask.

use std::collections::BTreeMap;
use std::time::Duration;

use super::press::{Press, PressId, PressPhase};

/// How many presses are tracked before further ones are dropped.
///
/// Ten is every finger a person has. Past that a host is misbehaving, and a
/// collection that grew without limit on a host's say-so is a collection a
/// misbehaving host can exhaust memory with.
const PRESS_LIMIT: usize = 10;

/// The presses in flight this frame, and where a resting device is pointing.
///
/// Ordered by identity, so "the first press" is the same interaction from one
/// frame to the next. A set that reordered would make a drag jump between
/// fingers half way through.
///
/// Holds presses that finished this frame as well as live ones: the frame a
/// press ends is the frame anything waiting on it gets to act, so dropping it
/// on arrival would hide every completion. They go at the next
/// [`Presses::advance`].
#[derive(Clone, Debug, Default)]
pub struct Presses {
    presses: BTreeMap<PressId, Press>,
    hover: Option<[f32; 2]>,
}

impl Presses {
    /// Where interface code should look this frame.
    ///
    /// The primary press if there is one, otherwise wherever a hovering device
    /// is resting. This is the question the old pointer accessor was trying to
    /// answer, and the reason it got it wrong: it read the *device*, which has
    /// nothing to report the moment a finger lifts, rather than the press,
    /// which knows where it finished.
    #[must_use]
    pub fn focus(&self) -> Option<[f32; 2]> {
        self.primary().map(Press::position).or(self.hover)
    }

    /// The press everything single-pointer should follow.
    ///
    /// The lowest identity, which is the oldest live interaction: a second
    /// finger arriving does not steal a drag from the first.
    #[must_use]
    pub fn primary(&self) -> Option<&Press> {
        self.presses.values().next()
    }

    /// Where a device that rests somewhere is resting, if one is.
    ///
    /// `None` on a touch screen even while a finger is down, because a finger
    /// is not hovering -- it is pressing, and that is [`Presses::primary`].
    #[must_use]
    pub const fn hover(&self) -> Option<[f32; 2]> {
        self.hover
    }

    #[must_use]
    pub fn get(&self, id: PressId) -> Option<&Press> {
        self.presses.get(&id)
    }

    /// Every press this frame, live or just finished, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Press> {
        self.presses.values()
    }

    /// Presses that arrived this frame.
    pub fn began(&self) -> impl Iterator<Item = &Press> {
        self.iter()
            .filter(|press| press.phase() == PressPhase::Began)
    }

    /// Presses that finished this frame by being let go.
    ///
    /// Cancelled presses are deliberately not here: a click completes on an
    /// ending, and a press the system took away should not complete one.
    pub fn ended(&self) -> impl Iterator<Item = &Press> {
        self.iter()
            .filter(|press| press.phase() == PressPhase::Ended)
    }

    /// How many presses are live.
    #[must_use]
    pub fn live(&self) -> usize {
        self.iter().filter(|press| press.phase().is_live()).count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.presses.is_empty()
    }

    /// Starts a press, unless one with this identity is already running or the
    /// host has reported more than anyone has fingers.
    pub fn begin(&mut self, id: PressId, at: [f32; 2]) {
        if self.presses.contains_key(&id) || self.presses.len() >= PRESS_LIMIT {
            return;
        }
        self.presses.insert(id, Press::began(id, at));
    }

    /// Moves a press that is running.
    ///
    /// A move for a press that never began is ignored rather than inventing
    /// one, which would hide a host bug behind an interaction nobody made.
    pub fn move_to(&mut self, id: PressId, at: [f32; 2]) {
        if let Some(press) = self.presses.get_mut(&id)
            && press.phase().is_live()
        {
            press.move_to(at);
        }
    }

    /// Finishes a press, leaving it in the set until the next frame so whoever
    /// is waiting on it sees the ending.
    pub fn finish(&mut self, id: PressId, phase: PressPhase) {
        if let Some(press) = self.presses.get_mut(&id)
            && press.phase().is_live()
        {
            press.finish(phase);
        }
    }

    /// Finishes every live press, for a window that stopped being told about
    /// them.
    ///
    /// Cancelled rather than ended: focus lost mid-drag is the system taking
    /// the interaction away, and it must not complete a click.
    pub fn cancel_all(&mut self) {
        let live: Vec<PressId> = self
            .presses
            .values()
            .filter(|press| press.phase().is_live())
            .map(Press::id)
            .collect();
        for id in live {
            self.finish(id, PressPhase::Cancelled);
        }
    }

    /// Where a resting device is, or `None` when it has left.
    pub const fn set_hover(&mut self, at: Option<[f32; 2]>) {
        self.hover = at;
    }

    /// Carries the set into the next frame.
    ///
    /// Presses that finished are dropped here rather than when they finished,
    /// which is what gives an ending exactly one frame of visibility.
    pub fn advance(&mut self, delta: Duration) {
        self.presses.retain(|_, press| press.phase().is_live());
        for press in self.presses.values_mut() {
            press.advance(delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PRESS_LIMIT, Presses};
    use crate::input::{PointerDevice, PressId, PressPhase};
    use std::time::Duration;

    const FRAME: Duration = Duration::from_millis(16);

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

    fn finger(raw: u64) -> PressId {
        PressId::new(PointerDevice::Touch, raw)
    }

    fn mouse() -> PressId {
        PressId::new(PointerDevice::Mouse, 0)
    }

    #[test]
    fn an_ending_is_visible_for_exactly_one_frame() {
        // The frame a press ends is the frame a click completes. Dropping it
        // the moment the host says so would mean nothing ever saw the ending.
        let mut presses = Presses::default();
        presses.begin(finger(1), [5.0, 5.0]);
        presses.advance(FRAME);

        presses.finish(finger(1), PressPhase::Ended);
        assert_eq!(presses.ended().count(), 1, "the ending is readable");
        assert_at(presses.focus().expect("and it says where"), [5.0, 5.0]);

        presses.advance(FRAME);
        assert!(presses.is_empty(), "and then it is gone");
        assert_eq!(presses.focus(), None);
    }

    #[test]
    fn a_finished_press_still_answers_where_it_is() {
        // The property the earlier bug violated, stated at the level a reader
        // actually asks it.
        let mut presses = Presses::default();
        presses.begin(finger(1), [10.0, 20.0]);
        presses.advance(FRAME);
        presses.move_to(finger(1), [12.0, 24.0]);
        presses.finish(finger(1), PressPhase::Ended);

        assert_at(presses.focus().expect("a focus"), [12.0, 24.0]);
    }

    #[test]
    fn a_second_finger_does_not_steal_the_first_ones_drag() {
        let mut presses = Presses::default();
        presses.begin(finger(1), [1.0, 1.0]);
        presses.advance(FRAME);
        presses.begin(finger(2), [50.0, 50.0]);

        assert_eq!(presses.primary().map(super::Press::id), Some(finger(1)));
        assert_eq!(presses.live(), 2);
    }

    #[test]
    fn a_finger_does_not_hover_but_a_mouse_does() {
        let mut presses = Presses::default();
        presses.begin(finger(1), [3.0, 4.0]);
        assert_eq!(presses.hover(), None, "a finger presses, it does not rest");
        assert_at(presses.focus().expect("a focus"), [3.0, 4.0]);

        let mut resting = Presses::default();
        resting.set_hover(Some([7.0, 8.0]));
        assert_eq!(
            resting.focus(),
            Some([7.0, 8.0]),
            "nothing pressed, so where it rests"
        );
        resting.begin(mouse(), [9.0, 9.0]);
        assert_eq!(
            resting.focus(),
            Some([9.0, 9.0]),
            "a press outranks resting"
        );
    }

    #[test]
    fn a_cancelled_press_is_not_an_ending() {
        let mut presses = Presses::default();
        presses.begin(finger(1), [0.0, 0.0]);
        presses.advance(FRAME);
        presses.cancel_all();

        assert_eq!(presses.ended().count(), 0, "nothing to complete a click");
        assert_eq!(presses.live(), 0);
    }

    #[test]
    fn a_move_for_a_press_that_never_began_is_ignored() {
        let mut presses = Presses::default();
        presses.move_to(finger(9), [1.0, 1.0]);
        assert!(presses.is_empty());
    }

    #[test]
    fn more_presses_than_fingers_are_dropped_rather_than_stored() {
        let mut presses = Presses::default();
        for raw in 0..(PRESS_LIMIT as u64 + 5) {
            presses.begin(finger(raw), [0.0, 0.0]);
        }
        assert_eq!(presses.live(), PRESS_LIMIT);
    }

    #[test]
    fn beginning_the_same_press_twice_does_not_restart_it() {
        let mut presses = Presses::default();
        presses.begin(finger(1), [1.0, 1.0]);
        presses.advance(FRAME);
        presses.begin(finger(1), [90.0, 90.0]);

        let press = presses.get(finger(1)).expect("still the first press");
        assert_at(press.origin(), [1.0, 1.0]);
        assert_eq!(press.held_for(), FRAME, "and it kept its age");
    }
}
