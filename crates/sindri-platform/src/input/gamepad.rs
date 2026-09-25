//! Gamepads, and which player holds which.
//!
//! A host reports each pad by an id of its own, and pads come and go: plugged
//! in, unplugged, asleep. A game does not want those ids. It wants *players*:
//! the first person who pressed a button, the second, and so on. So a pad
//! claims a player slot the first time a face button or Start is pressed on it,
//! keeps it while it stays connected, and gives it back when it goes, and a
//! game reads slots.
//!
//! Joins and leaves are edges, like a key press: `joined` answers for the frame
//! a slot was claimed, and never for two slots in one frame. A second pad
//! pressing in the same frame waits for the next, so a game that reads one
//! join per frame never misses one.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A pad, by the id its host gave it.
///
/// Meaningful only to the host that reported it. A game reads player slots,
/// which survive a pad being unplugged and another taking its place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PadId(pub u32);

/// How many players a game can have at once.
pub const SLOT_LIMIT: usize = 8;

/// A stick reading smaller than this is a stick at rest.
///
/// Every stick drifts a little from centre; without this a character walks
/// slowly away on its own.
pub const DEAD_ZONE: f32 = 0.2;

/// A button on a pad, named by where it is rather than what it is labelled.
///
/// "South" is A on one make of pad and a cross on another; naming it by place
/// is what lets one binding mean the same button on both.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl GamepadButton {
    pub const ALL: [Self; 16] = [
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::LeftBumper,
        Self::RightBumper,
        Self::LeftTrigger,
        Self::RightTrigger,
        Self::Select,
        Self::Start,
        Self::LeftStick,
        Self::RightStick,
        Self::DPadUp,
        Self::DPadDown,
        Self::DPadLeft,
        Self::DPadRight,
    ];

    /// The name a binding or a script writes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::North => "north",
            Self::LeftBumper => "left_bumper",
            Self::RightBumper => "right_bumper",
            Self::LeftTrigger => "left_trigger",
            Self::RightTrigger => "right_trigger",
            Self::Select => "select",
            Self::Start => "start",
            Self::LeftStick => "left_stick",
            Self::RightStick => "right_stick",
            Self::DPadUp => "dpad_up",
            Self::DPadDown => "dpad_down",
            Self::DPadLeft => "dpad_left",
            Self::DPadRight => "dpad_right",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|button| button.name() == name)
    }

    /// Whether pressing it claims a player slot for an unclaimed pad.
    ///
    /// The four face buttons and Start: what a person presses to say "me".
    /// Not a stick or a trigger, which are pushed by accident while picking a
    /// pad up.
    #[must_use]
    pub const fn joins(self) -> bool {
        matches!(
            self,
            Self::South | Self::East | Self::West | Self::North | Self::Start
        )
    }
}

/// A measured control on a pad.
///
/// Sticks read -1 to 1 in screen axes, as `Stick` does: right and *down* are
/// positive. Triggers read 0 to 1.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

impl GamepadAxis {
    pub const ALL: [Self; 6] = [
        Self::LeftX,
        Self::LeftY,
        Self::RightX,
        Self::RightY,
        Self::LeftTrigger,
        Self::RightTrigger,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LeftX => "left_x",
            Self::LeftY => "left_y",
            Self::RightX => "right_x",
            Self::RightY => "right_y",
            Self::LeftTrigger => "left_trigger",
            Self::RightTrigger => "right_trigger",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|axis| axis.name() == name)
    }

    const fn index(self) -> usize {
        self as usize
    }

    /// The two axes of the stick this one is half of.
    const fn stick(self) -> Option<(Self, Self)> {
        match self {
            Self::LeftX | Self::LeftY => Some((Self::LeftX, Self::LeftY)),
            Self::RightX | Self::RightY => Some((Self::RightX, Self::RightY)),
            Self::LeftTrigger | Self::RightTrigger => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Pad {
    held: BTreeSet<GamepadButton>,
    pressed: BTreeSet<GamepadButton>,
    released: BTreeSet<GamepadButton>,
    axes: [f32; 6],
}

impl Pad {
    /// An axis with the dead zone taken out, rescaled so the edge of the dead
    /// zone reads zero and full deflection still reads one.
    ///
    /// A stick's dead zone is round: it is the stick's distance from centre
    /// that is small, not each half of it, or a diagonal push snaps to an axis.
    fn axis(&self, axis: GamepadAxis) -> f32 {
        let raw = self.axes[axis.index()];
        let length = match axis.stick() {
            Some((x, y)) => self.axes[x.index()].hypot(self.axes[y.index()]),
            None => raw.abs(),
        };
        if length <= DEAD_ZONE {
            return 0.0;
        }
        let scale = ((length - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0) / length;
        raw * scale
    }
}

/// Every pad a host has reported, and the player slots they hold.
#[derive(Clone, Debug, Default)]
pub struct Gamepads {
    pads: BTreeMap<PadId, Pad>,
    /// Slot `n` is `slots[n - 1]`.
    slots: [Option<PadId>; SLOT_LIMIT],
    /// The slot claimed this frame, and the button pressed this frame that
    /// claimed it: none for a pad that pressed last frame and waited.
    joined: Option<(u32, Option<GamepadButton>)>,
    /// The slot given back this frame.
    left: Option<u32>,
    /// Pads that asked to join in a frame that already had a join.
    waiting: VecDeque<PadId>,
    /// Slots given back in a frame that already had a leave.
    leaving: VecDeque<u32>,
}

impl Gamepads {
    pub(super) fn connect(&mut self, pad: PadId) {
        self.pads.entry(pad).or_default();
    }

    pub(super) fn disconnect(&mut self, pad: PadId) {
        self.pads.remove(&pad);
        self.waiting.retain(|waiting| *waiting != pad);
        if let Some(index) = self.slots.iter().position(|held| *held == Some(pad)) {
            self.slots[index] = None;
            self.leave(slot_number(index));
        }
    }

    pub(super) fn press(&mut self, pad: PadId, button: GamepadButton) {
        let state = self.pads.entry(pad).or_default();
        if state.held.insert(button) {
            state.pressed.insert(button);
            if button.joins() && self.slot_of(pad).is_none() && !self.waiting.contains(&pad) {
                self.claim(pad, Some(button));
            }
        }
    }

    pub(super) fn release(&mut self, pad: PadId, button: GamepadButton) {
        if let Some(state) = self.pads.get_mut(&pad)
            && state.held.remove(&button)
        {
            state.released.insert(button);
        }
    }

    pub(super) fn move_axis(&mut self, pad: PadId, axis: GamepadAxis, value: f32) {
        let value = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.pads.entry(pad).or_default().axes[axis.index()] = value;
    }

    /// Clears this frame's edges, and lets one waiting pad join and one
    /// waiting slot leave.
    pub(super) fn begin_frame(&mut self) {
        for pad in self.pads.values_mut() {
            pad.pressed.clear();
            pad.released.clear();
        }
        self.joined = None;
        self.left = None;
        if let Some(pad) = self.waiting.pop_front() {
            // Its press was last frame's, so none of this frame's is the one
            // that claimed the slot.
            self.claim(pad, None);
        }
        if let Some(slot) = self.leaving.pop_front() {
            self.left = Some(slot);
        }
    }

    /// Gives every slot back without a leave, for a game starting again: the
    /// pads stay connected, and each joins anew with its next press.
    pub(super) fn forget_players(&mut self) {
        self.slots = [None; SLOT_LIMIT];
        self.joined = None;
        self.left = None;
        self.waiting.clear();
        self.leaving.clear();
    }

    /// Lets go of every button, as a window losing focus does.
    pub(super) fn release_everything(&mut self) {
        for pad in self.pads.values_mut() {
            for button in std::mem::take(&mut pad.held) {
                pad.released.insert(button);
            }
            pad.axes = [0.0; 6];
        }
    }

    fn claim(&mut self, pad: PadId, button: Option<GamepadButton>) {
        if !self.pads.contains_key(&pad) {
            return;
        }
        if self.joined.is_some() {
            self.waiting.push_back(pad);
            return;
        }
        if let Some(index) = self.slots.iter().position(Option::is_none) {
            self.slots[index] = Some(pad);
            self.joined = Some((slot_number(index), button));
        }
    }

    fn leave(&mut self, slot: u32) {
        if self.left.is_some() {
            self.leaving.push_back(slot);
        } else {
            self.left = Some(slot);
        }
    }

    fn slot_of(&self, pad: PadId) -> Option<u32> {
        self.slots
            .iter()
            .position(|held| *held == Some(pad))
            .map(slot_number)
    }

    /// The pads a slot reads: the one holding it, or every pad for slot 0.
    fn read(&self, slot: u32) -> impl Iterator<Item = (&PadId, &Pad)> {
        let only = match slot {
            0 => None,
            slot => Some(
                usize::try_from(slot - 1)
                    .ok()
                    .and_then(|index| self.slots.get(index).copied().flatten()),
            ),
        };
        self.pads
            .iter()
            .filter(move |(pad, _)| only.is_none_or(|held| held == Some(**pad)))
    }

    /// The slot claimed this frame, if one was.
    #[must_use]
    pub fn joined(&self) -> Option<u32> {
        self.joined.map(|(slot, _)| slot)
    }

    /// The slot given back this frame, because its pad went.
    #[must_use]
    pub const fn left(&self) -> Option<u32> {
        self.left
    }

    /// Whether a pad holds this slot.
    #[must_use]
    pub fn is_claimed(&self, slot: u32) -> bool {
        slot >= 1 && self.read(slot).next().is_some()
    }

    /// How many slots are held.
    #[must_use]
    pub fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// How many pads are connected, held or not.
    #[must_use]
    pub fn connected(&self) -> usize {
        self.pads.len()
    }

    /// Whether this button is held on this slot's pad, or on any pad for 0.
    #[must_use]
    pub fn down(&self, slot: u32, button: GamepadButton) -> bool {
        self.read(slot).any(|(_, pad)| pad.held.contains(&button))
    }

    /// Whether this button went down this frame.
    ///
    /// The press that claimed a slot is not reported through that slot: it
    /// meant "I am playing", and a game reading it again as "jump" would have
    /// every player jump as they join. Slot 0 still sees it.
    #[must_use]
    pub fn pressed(&self, slot: u32, button: GamepadButton) -> bool {
        if slot != 0 && self.joined == Some((slot, Some(button))) {
            return false;
        }
        self.read(slot)
            .any(|(_, pad)| pad.pressed.contains(&button))
    }

    /// Whether this button came up this frame.
    #[must_use]
    pub fn released(&self, slot: u32, button: GamepadButton) -> bool {
        self.read(slot)
            .any(|(_, pad)| pad.released.contains(&button))
    }

    /// An axis on this slot's pad, dead zone taken out.
    ///
    /// For slot 0, the pad pushed furthest, so a game that takes any pad
    /// steers with whichever one someone is actually holding.
    #[must_use]
    pub fn axis(&self, slot: u32, axis: GamepadAxis) -> f32 {
        self.read(slot)
            .map(|(_, pad)| pad.axis(axis))
            .fold(0.0, |best, value| {
                if value.abs() > f32::abs(best) {
                    value
                } else {
                    best
                }
            })
    }
}

fn slot_number(index: usize) -> u32 {
    u32::try_from(index + 1).expect("the slot limit fits a u32")
}

#[cfg(test)]
#[path = "gamepad_tests.rs"]
mod tests;
