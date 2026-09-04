//! What a frame's input adds up to: what is held, what changed, and where the
//! person is pointing.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use sindri_core::{Presses, StickSettings, VirtualStick};

use super::{Key, MouseButton};

/// How many fingers a host reports before the rest are dropped.
///
/// Past two a game is doing something this engine has no abstraction for yet,
/// and an unbounded map of live touches is a thing a misbehaving host could
/// grow without limit. Ten is every finger a person has.
pub(super) const TOUCH_LIMIT: usize = 10;

/// A single change reported by a platform host.
///
/// Hosts translate their native events into these; nothing above this layer
/// knows whether the input came from `winit`, the DOM, or a test.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum InputEvent {
    KeyPressed(Key),
    KeyReleased(Key),
    ButtonPressed(MouseButton),
    ButtonReleased(MouseButton),
    /// Pointer position in logical pixels, with the origin at the top left.
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerLeft,
    /// A finger arrived, moved, or left, in the same logical pixels a pointer
    /// is reported in.
    ///
    /// Separate from the pointer rather than folded into it, because they are
    /// different facts: a mouse has one position and is always somewhere, while
    /// fingers arrive and leave and there may be several. What *unifies* them
    /// is a decision for the surface a game reads, not for the record a host
    /// keeps — see [`InputState::pointer_position`].
    TouchStarted {
        id: u64,
        x: f32,
        y: f32,
    },
    TouchMoved {
        id: u64,
        x: f32,
        y: f32,
    },
    TouchEnded {
        id: u64,
    },
    Scrolled {
        x: f32,
        y: f32,
    },
    /// Window focus gained or lost.
    FocusChanged(bool),
}

/// Accumulated input for the current frame.
///
/// Holds both level state (is this key down?) and edge state (was it pressed
/// this frame?). Edges are cleared once per frame by the host, so gameplay sees
/// each press exactly once regardless of how many events arrived.
#[derive(Clone, Debug)]
pub struct InputState {
    keys_held: BTreeSet<Key>,
    keys_pressed: BTreeSet<Key>,
    keys_released: BTreeSet<Key>,
    buttons_held: BTreeSet<MouseButton>,
    buttons_pressed: BTreeSet<MouseButton>,
    buttons_released: BTreeSet<MouseButton>,
    pointer: Option<[f32; 2]>,
    /// Where each live finger is, by the id its host gave it.
    ///
    /// Ordered by id so "the first touch" is the same finger from one frame to
    /// the next: a map that reordered would make a drag jump between fingers.
    touches: BTreeMap<u64, [f32; 2]>,
    touches_began: BTreeSet<u64>,
    /// Fingers lifted this frame, and where each was when it went.
    ///
    /// The position is kept because a release is asked about in the frame it
    /// happens, and by then the finger is out of `touches`. A mouse is still
    /// wherever it was let go; without this a finger is nowhere, and every
    /// press that ends -- which is every tap -- ends over no element at all.
    touches_ended: BTreeMap<u64, [f32; 2]>,
    pointer_delta: [f32; 2],
    scroll_delta: [f32; 2],
    focused: bool,
    /// The same interactions, as presses rather than as devices.
    ///
    /// Built from the very same events as the fields above and kept beside
    /// them while callers move across. The difference is not the data but the
    /// shape: a press knows where it is for every frame of its life, where the
    /// device fields lose a finger the instant it lifts -- see the header of
    /// `sindri_core::input`.
    presses: Presses,
    /// A stick built out of whichever finger is steering.
    ///
    /// Kept here rather than left to each game because the anchoring, the
    /// clamp and the dead zone are the same decisions every time, and a game
    /// that rebuilt them would get a slightly different feel and one more
    /// place for them to be wrong.
    stick: VirtualStick,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys_held: BTreeSet::new(),
            keys_pressed: BTreeSet::new(),
            keys_released: BTreeSet::new(),
            buttons_held: BTreeSet::new(),
            buttons_pressed: BTreeSet::new(),
            buttons_released: BTreeSet::new(),
            pointer: None,
            touches: BTreeMap::new(),
            touches_began: BTreeSet::new(),
            touches_ended: BTreeMap::new(),
            pointer_delta: [0.0, 0.0],
            scroll_delta: [0.0, 0.0],
            focused: true,
            presses: Presses::default(),
            stick: VirtualStick::default(),
        }
    }
}

impl InputState {
    /// Folds one host event into the current frame.
    ///
    /// A repeated press of an already-held key is ignored, so operating-system
    /// key repeat cannot make `key_pressed` fire more than once per physical
    /// press.
    pub fn apply(&mut self, event: InputEvent) {
        // Before the device bookkeeping, because a button press is placed at
        // where the pointer was *before* this event, and a move in the same
        // event would otherwise have already shifted it.
        super::presses::apply(&mut self.presses, event, self.pointer);
        match event {
            InputEvent::KeyPressed(key) => {
                if self.keys_held.insert(key) {
                    self.keys_pressed.insert(key);
                }
            }
            InputEvent::KeyReleased(key) => {
                if self.keys_held.remove(&key) {
                    self.keys_released.insert(key);
                }
            }
            InputEvent::ButtonPressed(button) => {
                if self.buttons_held.insert(button) {
                    self.buttons_pressed.insert(button);
                }
            }
            InputEvent::ButtonReleased(button) => {
                if self.buttons_held.remove(&button) {
                    self.buttons_released.insert(button);
                }
            }
            InputEvent::PointerMoved { x, y } => {
                if let Some([previous_x, previous_y]) = self.pointer {
                    self.pointer_delta[0] += x - previous_x;
                    self.pointer_delta[1] += y - previous_y;
                }
                self.pointer = Some([x, y]);
            }
            InputEvent::PointerLeft => self.pointer = None,
            InputEvent::TouchStarted { id, x, y } => {
                // A host that reports more fingers than anyone has is dropping
                // the extra ones rather than growing a map without limit.
                if (self.touches.len() < TOUCH_LIMIT || self.touches.contains_key(&id))
                    && self.touches.insert(id, [x, y]).is_none()
                {
                    self.touches_began.insert(id);
                }
            }
            InputEvent::TouchMoved { id, x, y } => {
                // Only for a finger already down. A move for one that never
                // started is a host bug, and inventing the touch here would
                // hide it behind a finger that never arrived.
                if let Some(position) = self.touches.get_mut(&id) {
                    *position = [x, y];
                }
            }
            InputEvent::TouchEnded { id } => {
                if let Some(position) = self.touches.remove(&id) {
                    self.touches_ended.insert(id, position);
                }
            }
            InputEvent::Scrolled { x, y } => {
                self.scroll_delta[0] += x;
                self.scroll_delta[1] += y;
            }
            InputEvent::FocusChanged(focused) => {
                self.focused = focused;
                if !focused {
                    // Key-up events are not delivered while unfocused, so
                    // anything still held would stick down forever.
                    self.release_everything();
                }
            }
        }
        self.settle();
    }

    /// Clears edge state and per-frame deltas, keeping what is still held.
    ///
    /// `delta` is how long the frame being spent was, which is what ages a
    /// press. Measured rather than counted so that a long press is the same
    /// length of time on a 60 Hz phone and a 144 Hz monitor.
    pub fn begin_frame(&mut self, delta: Duration) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.touches_began.clear();
        self.touches_ended.clear();
        self.pointer_delta = [0.0, 0.0];
        self.scroll_delta = [0.0, 0.0];
        self.presses.advance(delta);
        self.settle();
    }

    fn release_everything(&mut self) {
        for key in std::mem::take(&mut self.keys_held) {
            self.keys_released.insert(key);
        }
        for button in std::mem::take(&mut self.buttons_held) {
            self.buttons_released.insert(button);
        }
        // A finger cannot be reported as lifted once the window has stopped
        // hearing about it, so one still down would stay down for ever.
        for (id, position) in std::mem::take(&mut self.touches) {
            self.touches_ended.insert(id, position);
        }
    }

    /// Re-reads whatever is derived from the presses.
    ///
    /// Called wherever the presses change rather than at one frame boundary,
    /// because there is no such boundary here: `begin_frame` runs before the
    /// frame's events and a value read after them has to include them.
    fn settle(&mut self) {
        self.stick.update(&self.presses);
    }

    /// The stick the steering finger is making.
    #[must_use]
    pub const fn stick(&self) -> &VirtualStick {
        &self.stick
    }

    /// Retunes the stick, for a game whose reach is not the default one.
    pub fn set_stick_settings(&mut self, settings: StickSettings) {
        self.stick = VirtualStick::new(settings);
    }

    /// This frame's presses.
    ///
    /// The shape to read for anything that asks where an interaction is or was:
    /// unlike the device accessors, a press here answers on every frame of its
    /// life, the frame it ends included.
    #[must_use]
    pub const fn presses(&self) -> &Presses {
        &self.presses
    }

    pub fn key_down(&self, key: Key) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn key_released(&self, key: Key) -> bool {
        self.keys_released.contains(&key)
    }

    pub fn button_down(&self, button: MouseButton) -> bool {
        self.buttons_held.contains(&button)
    }

    pub fn button_pressed(&self, button: MouseButton) -> bool {
        self.buttons_pressed.contains(&button)
    }

    pub fn button_released(&self, button: MouseButton) -> bool {
        self.buttons_released.contains(&button)
    }

    /// A -1, 0, or 1 axis from two opposing keys.
    ///
    /// Holding both returns zero, which keeps opposed movement keys from
    /// cancelling into a direction that depends on event order.
    pub fn axis(&self, negative: Key, positive: Key) -> f32 {
        f32::from(self.key_down(positive)) - f32::from(self.key_down(negative))
    }

    /// Where the mouse is, or `None` when it is not over the window.
    ///
    /// The mouse and nothing else. [`Self::pointer_position`] is the one a
    /// game reads.
    pub const fn pointer(&self) -> Option<[f32; 2]> {
        self.pointer
    }

    /// Where *the* pointer is: the mouse if there is one, else the first
    /// finger.
    ///
    /// One position for both, because a game that aims at a point should not
    /// have to ask which device the person is using — and a game written for a
    /// mouse then works on a phone without a second code path. The mouse wins
    /// when both are present, because a machine with both is a machine someone
    /// is using the mouse on.
    #[must_use]
    pub fn pointer_position(&self) -> Option<[f32; 2]> {
        self.pointer
            .or_else(|| self.touches.values().next().copied())
            // A finger that lifted this frame is still the answer to "where is
            // the pointer" for the rest of it, so a tap completes where it
            // ended rather than nowhere.
            .or_else(|| self.touches_ended.values().next().copied())
    }

    /// Whether the pointer is being pressed: this mouse button, or any finger.
    ///
    /// A finger counts as [`MouseButton::Left`] and as nothing else, which is
    /// the convention the web settled on and the one that lets a tap and a
    /// click be the same line of gameplay code.
    #[must_use]
    pub fn pointer_down(&self, button: MouseButton) -> bool {
        self.button_down(button) || (button == MouseButton::Left && !self.touches.is_empty())
    }

    /// Whether the pointer went down this frame.
    #[must_use]
    pub fn pointer_pressed(&self, button: MouseButton) -> bool {
        self.button_pressed(button)
            || (button == MouseButton::Left && !self.touches_began.is_empty())
    }

    /// Whether the pointer came up this frame.
    ///
    /// For a finger this means the *last* one left: a second finger lifting
    /// while one is still down is not the pointer coming up, any more than
    /// releasing the right mouse button releases the left.
    #[must_use]
    pub fn pointer_released(&self, button: MouseButton) -> bool {
        self.button_released(button)
            || (button == MouseButton::Left
                && !self.touches_ended.is_empty()
                && self.touches.is_empty())
    }

    /// How many fingers are down.
    #[must_use]
    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }

    /// Where the `index`-th finger is, ordered by the id its host gave it.
    ///
    /// Stable from frame to frame while a finger stays down, which is what a
    /// drag needs: an order that reshuffled would make one finger's drag jump
    /// to another's.
    #[must_use]
    pub fn touch_at(&self, index: usize) -> Option<[f32; 2]> {
        self.touches.values().nth(index).copied()
    }

    pub const fn pointer_delta(&self) -> [f32; 2] {
        self.pointer_delta
    }

    pub const fn scroll_delta(&self) -> [f32; 2] {
        self.scroll_delta
    }

    pub const fn is_focused(&self) -> bool {
        self.focused
    }
}
