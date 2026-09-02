//! What a frame's input adds up to: what is held, what changed, and where the
//! person is pointing.

use std::collections::{BTreeMap, BTreeSet};

use super::{Key, MouseButton};

/// How many fingers a host reports before the rest are dropped.
///
/// Past two a game is doing something this engine has no abstraction for yet,
/// and an unbounded map of live touches is a thing a misbehaving host could
/// grow without limit. Ten is every finger a person has.
const TOUCH_LIMIT: usize = 10;

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
    touches_ended: BTreeSet<u64>,
    pointer_delta: [f32; 2],
    scroll_delta: [f32; 2],
    focused: bool,
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
            touches_ended: BTreeSet::new(),
            pointer_delta: [0.0, 0.0],
            scroll_delta: [0.0, 0.0],
            focused: true,
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
                if self.touches.remove(&id).is_some() {
                    self.touches_ended.insert(id);
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
    }

    /// Clears edge state and per-frame deltas, keeping what is still held.
    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.touches_began.clear();
        self.touches_ended.clear();
        self.pointer_delta = [0.0, 0.0];
        self.scroll_delta = [0.0, 0.0];
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
        for id in std::mem::take(&mut self.touches).into_keys() {
            self.touches_ended.insert(id);
        }
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

#[cfg(test)]
mod tests {
    use super::{InputEvent, InputState, MouseButton};

    fn down(state: &mut InputState, id: u64, x: f32, y: f32) {
        state.apply(InputEvent::TouchStarted { id, x, y });
    }

    #[test]
    fn a_finger_is_where_the_host_put_it() {
        let mut state = InputState::default();
        down(&mut state, 1, 10.0, 20.0);
        assert_eq!(state.touch_count(), 1);
        assert_eq!(state.touch_at(0), Some([10.0, 20.0]));

        state.apply(InputEvent::TouchMoved {
            id: 1,
            x: 30.0,
            y: 40.0,
        });
        assert_eq!(state.touch_at(0), Some([30.0, 40.0]));

        state.apply(InputEvent::TouchEnded { id: 1 });
        assert_eq!(state.touch_count(), 0);
        assert_eq!(state.touch_at(0), None);
    }

    /// A game that aims at a point should not have to ask which device the
    /// person is using.
    #[test]
    fn the_pointer_is_the_mouse_or_the_first_finger() {
        let mut state = InputState::default();
        assert_eq!(state.pointer_position(), None);

        down(&mut state, 7, 5.0, 6.0);
        assert_eq!(state.pointer_position(), Some([5.0, 6.0]));

        // A machine with both is a machine someone is using the mouse on.
        state.apply(InputEvent::PointerMoved { x: 1.0, y: 2.0 });
        assert_eq!(state.pointer_position(), Some([1.0, 2.0]));
    }

    #[test]
    fn a_tap_reads_as_the_left_button() {
        let mut state = InputState::default();
        down(&mut state, 1, 0.0, 0.0);
        assert!(state.pointer_down(MouseButton::Left));
        assert!(state.pointer_pressed(MouseButton::Left));
        // And as nothing else: a finger is not a right-click.
        assert!(!state.pointer_down(MouseButton::Right));

        state.begin_frame();
        assert!(state.pointer_down(MouseButton::Left), "still held");
        assert!(
            !state.pointer_pressed(MouseButton::Left),
            "pressed is an edge"
        );

        state.apply(InputEvent::TouchEnded { id: 1 });
        assert!(state.pointer_released(MouseButton::Left));
        assert!(!state.pointer_down(MouseButton::Left));
    }

    /// A second finger lifting while one is still down is not the pointer
    /// coming up, any more than releasing the right mouse button releases the
    /// left.
    #[test]
    fn the_pointer_comes_up_when_the_last_finger_does() {
        let mut state = InputState::default();
        down(&mut state, 1, 0.0, 0.0);
        down(&mut state, 2, 1.0, 1.0);
        state.begin_frame();

        state.apply(InputEvent::TouchEnded { id: 2 });
        assert!(!state.pointer_released(MouseButton::Left));
        assert!(state.pointer_down(MouseButton::Left));

        state.apply(InputEvent::TouchEnded { id: 1 });
        assert!(state.pointer_released(MouseButton::Left));
    }

    /// The order fingers are reported in has to be the same order next frame,
    /// or a drag would jump from one finger to another.
    #[test]
    fn fingers_keep_their_order_between_frames() {
        let mut state = InputState::default();
        down(&mut state, 9, 90.0, 0.0);
        down(&mut state, 2, 20.0, 0.0);
        assert_eq!(state.touch_at(0), Some([20.0, 0.0]));
        assert_eq!(state.touch_at(1), Some([90.0, 0.0]));

        state.begin_frame();
        assert_eq!(state.touch_at(0), Some([20.0, 0.0]));
    }

    /// A finger cannot be reported as lifted once the window has stopped
    /// hearing about it, so one still down would stay down for ever.
    #[test]
    fn losing_focus_lets_go_of_every_finger() {
        let mut state = InputState::default();
        down(&mut state, 1, 0.0, 0.0);
        state.apply(InputEvent::FocusChanged(false));
        assert_eq!(state.touch_count(), 0);
        assert!(!state.pointer_down(MouseButton::Left));
    }

    /// A move for a finger that never started is a host bug, and inventing the
    /// touch would hide it behind a finger that never arrived.
    #[test]
    fn a_finger_that_never_started_does_not_appear_by_moving() {
        let mut state = InputState::default();
        state.apply(InputEvent::TouchMoved {
            id: 3,
            x: 1.0,
            y: 1.0,
        });
        assert_eq!(state.touch_count(), 0);
    }

    #[test]
    fn a_host_reporting_more_fingers_than_anyone_has_is_bounded() {
        let mut state = InputState::default();
        for id in 0..64 {
            down(&mut state, id, 0.0, 0.0);
        }
        assert_eq!(state.touch_count(), super::TOUCH_LIMIT);
    }
}
