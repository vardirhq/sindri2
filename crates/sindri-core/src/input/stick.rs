//! A joystick made out of a finger.
//!
//! A phone has no stick, so a game that wants one has to build it out of the
//! only thing a phone reports: where a finger is. The rule is short -- the
//! press anchors where it lands, and how far it has been pulled from there is
//! how far the stick is pushed -- but the details are what separate a control
//! that feels like a stick from one that feels like a bug, and they are the
//! same details in every game that has ever needed one.
//!
//! # Why not just steer towards the finger
//!
//! Because the finger is somewhere on the screen and the ship is somewhere
//! else, so "go to my finger" makes the ship lunge when a thumb lands, gives
//! the player no way to ask for *gently left*, and needs the thumb over the
//! part of the screen they are trying to look at. Anchoring makes the gesture
//! relative: the thumb can start anywhere, the ship does not move until it is
//! dragged, and full speed is a known distance rather than a screen away.

use crate::input::{Press, PressId, Presses};

/// How a stick answers to a finger.
///
/// Authorable for the reason [`GestureLimits`] is: a thumb reaching across a
/// phone and a thumb resting on a tablet want different distances, and there is
/// no one number that is right for both.
///
/// [`GestureLimits`]: super::GestureLimits
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickSettings {
    /// How far, in the units positions are reported in, the finger travels
    /// from where it landed for the stick to read fully pushed.
    ///
    /// In *physical* pixels, like every other position: a radius in logical
    /// ones would be a stick that needs a third of the travel on a dense
    /// display, which is the same class of fault that made a tap miss.
    pub radius: f32,
    /// How far the finger may drift, as a fraction of the radius, and still
    /// read as centred.
    ///
    /// A thumb resting on glass is never still. Without this the ship drifts
    /// whenever a finger is down, which reads as the game ignoring the player
    /// rather than as a control with slack in it.
    pub dead_zone: f32,
}

impl Default for StickSettings {
    fn default() -> Self {
        Self {
            // About a thumb's comfortable travel on a phone, and small enough
            // that a stick anchored near an edge still has room to be pushed.
            radius: 120.0,
            dead_zone: 0.15,
        }
    }
}

/// A stick driven by one press.
///
/// Holds only which press is driving it. Everything else is read from that
/// press, because a [`Press`] already remembers where it began -- and a second
/// copy of the anchor is a second thing that can disagree with where the finger
/// actually landed.
#[derive(Clone, Debug, Default)]
pub struct VirtualStick {
    settings: StickSettings,
    owner: Option<PressId>,
    value: [f32; 2],
}

impl VirtualStick {
    #[must_use]
    pub fn new(settings: StickSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn settings(&self) -> StickSettings {
        self.settings
    }

    /// Reads this frame's presses.
    ///
    /// A stick keeps the press it started with until that press ends, rather
    /// than following whichever finger is newest: a second thumb landing to
    /// fire should not snatch the steering away mid-turn, which is what makes
    /// a two-thumb layout possible at all.
    pub fn update(&mut self, presses: &Presses) {
        let driving = self
            .owner
            .and_then(|id| presses.get(id))
            .filter(|press| press.phase().is_live())
            .or_else(|| {
                // Nothing driving it, so the next press to arrive takes it.
                // `began` rather than any live press, or releasing the stick
                // while another finger happened to be down would hand the
                // stick straight to that finger.
                presses.began().find(|press| press.phase().is_live())
            });

        if let Some(press) = driving {
            self.owner = Some(press.id());
            self.value = deflection(press, self.settings);
        } else {
            self.owner = None;
            // Centred the moment the finger leaves. A stick that kept its last
            // reading would be a ship that keeps flying after the player has
            // let go.
            self.value = [0.0, 0.0];
        }
    }

    /// How far the stick is pushed, from -1 to 1 on each axis.
    ///
    /// Never longer than 1: pulling further than the radius does not go faster,
    /// it only changes direction. Screen axes, so `y` grows downward like every
    /// other position the engine reports -- a game that wants up to be positive
    /// negates it, which is the same thing it already does for a pointer.
    #[must_use]
    pub const fn value(&self) -> [f32; 2] {
        self.value
    }

    /// Whether a finger is on the stick at all.
    ///
    /// Distinct from a zero value, which is also what a thumb sitting inside
    /// the dead zone reads: a game showing the stick wants to draw it while it
    /// is held even when it is centred.
    #[must_use]
    pub const fn is_engaged(&self) -> bool {
        self.owner.is_some()
    }

    /// Where the stick is anchored, for a game that draws it.
    ///
    /// The engine works this out whether or not anything draws it, because the
    /// anchor is where the input decision was made: a drawn ring that did not
    /// come from here would be a picture of a different stick.
    #[must_use]
    pub fn anchor(&self, presses: &Presses) -> Option<[f32; 2]> {
        self.owner.and_then(|id| presses.get(id)).map(Press::origin)
    }

    /// Where to draw the thumb: on the ring at full push, inside it otherwise.
    #[must_use]
    pub fn thumb(&self, presses: &Presses) -> Option<[f32; 2]> {
        let anchor = self.anchor(presses)?;
        Some([
            anchor[0] + self.value[0] * self.settings.radius,
            anchor[1] + self.value[1] * self.settings.radius,
        ])
    }
}

/// How far one press has been pulled from where it landed.
fn deflection(press: &Press, settings: StickSettings) -> [f32; 2] {
    if settings.radius <= 0.0 {
        return [0.0, 0.0];
    }
    let origin = press.origin();
    let at = press.position();
    let offset = [at[0] - origin[0], at[1] - origin[1]];
    let distance = offset[0].hypot(offset[1]);
    if distance <= 0.0 {
        return [0.0, 0.0];
    }

    let pushed = (distance / settings.radius).min(1.0);
    let dead = settings.dead_zone.clamp(0.0, 1.0);
    if pushed <= dead {
        return [0.0, 0.0];
    }
    // Rescaled past the dead zone rather than stepped over it, so the stick
    // starts from nothing at the edge of the slack instead of jumping to a
    // fraction of full speed the instant the thumb clears it.
    let scaled = (pushed - dead) / (1.0 - dead);
    let unit = [offset[0] / distance, offset[1] / distance];
    [unit[0] * scaled, unit[1] * scaled]
}

#[cfg(test)]
mod tests {
    use super::{StickSettings, VirtualStick};
    use crate::input::{PointerDevice, PressId, PressPhase, Presses};

    const RADIUS: f32 = 100.0;

    fn settings() -> StickSettings {
        StickSettings {
            radius: RADIUS,
            dead_zone: 0.1,
        }
    }

    fn finger(raw: u64) -> PressId {
        PressId::new(PointerDevice::Touch, raw)
    }

    /// Centred, component by component.
    ///
    /// The values really are exactly zero -- the code returns the literal --
    /// but comparing float arrays outright is the habit that hides a genuine
    /// rounding difference somewhere else, so it is spelled the careful way.
    #[track_caller]
    fn assert_centred(value: [f32; 2]) {
        assert_near(value[0], 0.0);
        assert_near(value[1], 0.0);
    }

    #[track_caller]
    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-4,
            "expected {expected}, got {actual}"
        );
    }

    /// A stick anchored where the thumb landed, not where the screen's middle
    /// happens to be: the whole point of the gesture is that it is relative.
    #[test]
    fn a_thumb_that_lands_and_does_not_move_asks_for_nothing() {
        let mut presses = Presses::default();
        presses.begin(finger(0), [700.0, 1800.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);

        assert_centred(stick.value());
        assert!(stick.is_engaged(), "but the stick is being held");
    }

    #[test]
    fn pulling_a_full_radius_is_a_full_push() {
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [500.0 + RADIUS, 500.0]);
        stick.update(&presses);

        assert_near(stick.value()[0], 1.0);
        assert_near(stick.value()[1], 0.0);
    }

    #[test]
    fn pulling_further_than_the_radius_does_not_go_faster() {
        // It only changes direction. A stick that kept scaling would make the
        // top speed depend on how much screen is left beyond the thumb.
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [500.0 + RADIUS * 8.0, 500.0]);
        stick.update(&presses);

        assert_near(stick.value()[0], 1.0);
    }

    #[test]
    fn a_diagonal_is_not_faster_than_a_straight_line() {
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [500.0 + RADIUS, 500.0 + RADIUS]);
        stick.update(&presses);

        let value = stick.value();
        assert_near(value[0].hypot(value[1]), 1.0);
    }

    #[test]
    fn a_resting_thumb_does_not_drift_the_ship() {
        // A finger on glass is never still. Inside the dead zone the stick
        // reads centred, or a game looks like it is ignoring the player.
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [505.0, 503.0]);
        stick.update(&presses);

        assert_centred(stick.value());
    }

    #[test]
    fn the_stick_starts_from_nothing_at_the_edge_of_the_dead_zone() {
        // Rescaled rather than stepped over: clearing the slack must not jump
        // the ship straight to a tenth of full speed.
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        // A hair past the dead zone.
        presses.move_to(finger(0), [500.0 + RADIUS * 0.1001, 500.0]);
        stick.update(&presses);

        let pushed = stick.value()[0];
        assert!(pushed > 0.0 && pushed < 0.01, "jumped straight to {pushed}");
    }

    #[test]
    fn letting_go_centres_the_stick() {
        // A ship that keeps flying after the player lets go is the fault this
        // is here to prevent.
        let mut presses = Presses::default();
        presses.begin(finger(0), [500.0, 500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [500.0 + RADIUS, 500.0]);
        stick.update(&presses);
        assert_near(stick.value()[0], 1.0);

        presses.finish(finger(0), PressPhase::Ended);
        stick.update(&presses);
        assert_centred(stick.value());
        assert!(!stick.is_engaged());
    }

    /// What makes a two-thumb layout possible: the hand that is steering keeps
    /// steering when the other hand does something else.
    #[test]
    fn a_second_finger_does_not_snatch_the_stick_mid_turn() {
        let mut presses = Presses::default();
        presses.begin(finger(0), [300.0, 1500.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        presses.move_to(finger(0), [300.0 - RADIUS, 1500.0]);
        stick.update(&presses);
        assert_near(stick.value()[0], -1.0);

        // The other thumb arrives somewhere else entirely.
        presses.begin(finger(1), [900.0, 1500.0]);
        stick.update(&presses);
        assert_near(stick.value()[0], -1.0);
    }

    #[test]
    fn the_next_finger_takes_a_stick_nobody_is_holding() {
        let mut presses = Presses::default();
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        assert!(!stick.is_engaged());

        presses.begin(finger(3), [400.0, 400.0]);
        stick.update(&presses);
        assert!(stick.is_engaged());
    }

    /// The geometry a game draws the control from, so a drawn ring cannot end
    /// up describing a different stick than the one being read.
    #[test]
    fn a_game_can_ask_where_to_draw_it() {
        let mut presses = Presses::default();
        presses.begin(finger(0), [640.0, 1600.0]);
        let mut stick = VirtualStick::new(settings());
        stick.update(&presses);
        let anchor = stick.anchor(&presses).expect("held");
        assert_near(anchor[0], 640.0);
        assert_near(anchor[1], 1600.0);

        presses.move_to(finger(0), [640.0 + RADIUS, 1600.0]);
        stick.update(&presses);
        let thumb = stick.thumb(&presses).expect("held");
        assert_near(thumb[0], 640.0 + RADIUS);
        assert_near(thumb[1], 1600.0);

        presses.finish(finger(0), PressPhase::Ended);
        stick.update(&presses);
        assert_eq!(stick.anchor(&presses), None, "nothing to draw when let go");
    }
}
