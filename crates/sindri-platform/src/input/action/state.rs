//! What each action is worth this frame.

use super::binding::{Binding, Source};
use super::map::{Action, ActionId, ActionMap};
use crate::input::InputState;

/// How far from rest a value must be before an action counts as pressed.
///
/// A stick at rest is never exactly zero, and a game that treated any reading
/// as a press would fire continuously on an untouched pad. Well below anything
/// a person means and well above anything a device drifts to.
const PRESS_POINT: f32 = 0.5;

/// One action's answer this frame.
///
/// Carries the value *and* the edges, because reading either alone is a
/// papercut every game hits: `value` for "how far is the stick pushed",
/// `pressed` for "did they just fire", and the same action answers both without
/// the game keeping its own copy of last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActionState {
    value: [f32; 2],
    held: bool,
    was_held: bool,
}

impl ActionState {
    /// The full value, for an action that is a direction.
    #[must_use]
    pub const fn vector(&self) -> [f32; 2] {
        self.value
    }

    /// The first component, for an action that is one number.
    #[must_use]
    pub const fn axis(&self) -> f32 {
        self.value[0]
    }

    /// Whether the action is being made now.
    #[must_use]
    pub const fn held(&self) -> bool {
        self.held
    }

    /// Whether it started being made this frame.
    #[must_use]
    pub const fn pressed(&self) -> bool {
        self.held && !self.was_held
    }

    /// Whether it stopped being made this frame.
    #[must_use]
    pub const fn released(&self) -> bool {
        !self.held && self.was_held
    }
}

/// Every action's state, recomputed each frame from the input.
///
/// Deliberately derived rather than accumulated: given the same input state and
/// the same map, this produces the same answers, so a recorded run replays to
/// the same game. Anything that remembered more than the previous frame's
/// held-ness would be a second place for the truth to live.
#[derive(Clone, Debug, Default)]
pub struct Actions {
    states: Vec<ActionState>,
}

impl Actions {
    /// Reads every action in `map` against this frame's input.
    pub fn update(&mut self, map: &ActionMap, input: &InputState) {
        self.states.resize(map.len(), ActionState::default());
        for (state, action) in self.states.iter_mut().zip(map.iter()) {
            let value = resolve(action, input);
            state.was_held = state.held;
            state.value = value;
            state.held = value[0].abs().max(value[1].abs()) >= PRESS_POINT;
        }
    }

    /// What an action is worth, or a resting state for one this map has never
    /// heard of.
    ///
    /// Resting rather than `None` because every caller would otherwise write
    /// the same unwrap, and an id can only come from a map that declared it.
    #[must_use]
    pub fn get(&self, id: ActionId) -> ActionState {
        self.states.get(id.index()).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn vector(&self, id: ActionId) -> [f32; 2] {
        self.get(id).vector()
    }

    #[must_use]
    pub fn axis(&self, id: ActionId) -> f32 {
        self.get(id).axis()
    }

    #[must_use]
    pub fn held(&self, id: ActionId) -> bool {
        self.get(id).held()
    }

    #[must_use]
    pub fn pressed(&self, id: ActionId) -> bool {
        self.get(id).pressed()
    }

    #[must_use]
    pub fn released(&self, id: ActionId) -> bool {
        self.get(id).released()
    }
}

/// The strongest reading among an action's bindings.
///
/// Strongest rather than first, so a game bound to both a keyboard and a pad
/// answers to whichever is actually being used without anyone selecting a
/// device: an untouched binding reads zero and loses.
fn resolve(action: &Action, input: &InputState) -> [f32; 2] {
    let mut best = [0.0, 0.0];
    let mut strength = 0.0;
    for binding in &action.bindings {
        let value = read(binding, input);
        let magnitude = value[0].abs().max(value[1].abs());
        if magnitude > strength {
            strength = magnitude;
            best = value;
        }
    }
    best
}

fn read(binding: &Binding, input: &InputState) -> [f32; 2] {
    match binding {
        Binding::Simple(source) => [measure(*source, input), 0.0],
        Binding::Axis { negative, positive } => {
            [measure(*positive, input) - measure(*negative, input), 0.0]
        }
        Binding::Vector {
            up,
            down,
            left,
            right,
        } => {
            // A direction is a direction, not a speed. Held diagonally, four
            // keys make a vector of length 1.41, and a game that used it
            // unchanged moves faster on the diagonal -- the oldest bug in
            // keyboard movement, fixed here once rather than in every game
            // that ever reads two axes.
            normalized([
                measure(*right, input) - measure(*left, input),
                measure(*up, input) - measure(*down, input),
            ])
        }
    }
}

fn normalized(value: [f32; 2]) -> [f32; 2] {
    let length = value[0].hypot(value[1]);
    if length > 1.0 {
        [value[0] / length, value[1] / length]
    } else {
        value
    }
}

/// A pressed source, as a full deflection or none.
const fn held_as_number(down: bool) -> f32 {
    if down { 1.0 } else { 0.0 }
}

/// One source's reading, as a number.
///
/// A pressed source is 0 or 1; a measured one is whatever it is sitting at.
fn measure(source: Source, input: &InputState) -> f32 {
    match source {
        Source::Key(key) => held_as_number(input.key_down(key)),
        Source::MouseButton(button) => held_as_number(input.button_down(button)),
        Source::PointerX => input.presses().focus().map_or(0.0, |at| at[0]),
        Source::PointerY => input.presses().focus().map_or(0.0, |at| at[1]),
        Source::PointerDeltaX => input.pointer_delta()[0],
        Source::PointerDeltaY => input.pointer_delta()[1],
        Source::ScrollX => input.scroll_delta()[0],
        Source::ScrollY => input.scroll_delta()[1],
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionMap, Actions};
    use crate::input::action::binding::{Binding, Source};
    use crate::input::action::map::{ActionId, ActionKind};
    use crate::input::{InputEvent, InputState, Key, MouseButton};
    use std::time::Duration;

    const FRAME: Duration = Duration::from_millis(16);

    fn wasd() -> Binding {
        Binding::Vector {
            up: Source::Key(Key::W),
            down: Source::Key(Key::S),
            left: Source::Key(Key::A),
            right: Source::Key(Key::D),
        }
    }

    /// A map with the two actions every game has, and the state to read them.
    fn moving_and_firing() -> (ActionMap, Actions, ActionId, ActionId) {
        let mut map = ActionMap::default();
        let move_id = map
            .declare("move", ActionKind::Vector, vec![wasd()])
            .expect("declared");
        let fire_id = map
            .declare(
                "fire",
                ActionKind::Button,
                vec![
                    Binding::Simple(Source::Key(Key::Space)),
                    Binding::Simple(Source::MouseButton(MouseButton::Left)),
                ],
            )
            .expect("declared");
        (map, Actions::default(), move_id, fire_id)
    }

    #[track_caller]
    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn a_key_a_game_never_names_still_moves_it() {
        // The point of the whole module: gameplay asks for "move", and what
        // that is made of is written somewhere else entirely.
        let (map, mut actions, move_id, _) = moving_and_firing();
        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::D));
        actions.update(&map, &input);

        assert_near(actions.vector(move_id)[0], 1.0);
        assert_near(actions.vector(move_id)[1], 0.0);
    }

    #[test]
    fn a_diagonal_is_not_faster_than_a_straight_line() {
        // Two keys held make a vector of length 1.41 unless something says
        // otherwise, and every game that reads two axes raw moves faster
        // diagonally. Said once, here.
        let (map, mut actions, move_id, _) = moving_and_firing();
        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::W));
        input.apply(InputEvent::KeyPressed(Key::D));
        actions.update(&map, &input);

        let value = actions.vector(move_id);
        assert_near(value[0].hypot(value[1]), 1.0);
    }

    #[test]
    fn opposite_keys_held_together_cancel() {
        let (map, mut actions, move_id, _) = moving_and_firing();
        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::A));
        input.apply(InputEvent::KeyPressed(Key::D));
        actions.update(&map, &input);
        assert_near(actions.vector(move_id)[0], 0.0);
    }

    #[test]
    fn either_binding_makes_the_action() {
        // A game bound to a key and a button answers to whichever is used,
        // without anyone choosing a device first.
        let (map, mut actions, _, fire_id) = moving_and_firing();

        let mut keyboard = InputState::default();
        keyboard.apply(InputEvent::KeyPressed(Key::Space));
        actions.update(&map, &keyboard);
        assert!(actions.held(fire_id));

        let mut mouse = InputState::default();
        mouse.apply(InputEvent::ButtonPressed(MouseButton::Left));
        let mut fresh = Actions::default();
        fresh.update(&map, &mouse);
        assert!(fresh.held(fire_id));
    }

    #[test]
    fn an_action_reports_the_edges_as_well_as_the_value() {
        // Both, because a game needs "is it held" for movement and "did it
        // just happen" for firing, and keeping its own copy of last frame to
        // get the second is the papercut this removes.
        let (map, mut actions, _, fire_id) = moving_and_firing();
        let mut input = InputState::default();

        input.apply(InputEvent::KeyPressed(Key::Space));
        actions.update(&map, &input);
        assert!(actions.pressed(fire_id), "the frame it goes down");
        assert!(actions.held(fire_id));
        assert!(!actions.released(fire_id));

        input.begin_frame(FRAME);
        actions.update(&map, &input);
        assert!(!actions.pressed(fire_id), "and only that frame");
        assert!(actions.held(fire_id), "but it is still held");

        input.apply(InputEvent::KeyReleased(Key::Space));
        actions.update(&map, &input);
        assert!(actions.released(fire_id));
        assert!(!actions.held(fire_id));
    }

    #[test]
    fn rebinding_moves_the_control_without_touching_the_game() {
        // What a game reads does not change: the id is the same, the kind is
        // the same, and only the key is different.
        let (mut map, mut actions, _, fire_id) = moving_and_firing();
        map.rebind(fire_id, vec![Binding::Simple(Source::Key(Key::Enter))])
            .expect("a button can be bound to a key");

        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::Space));
        actions.update(&map, &input);
        assert!(!actions.held(fire_id), "the old key is not the control now");

        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::Enter));
        actions.update(&map, &input);
        assert!(actions.held(fire_id));
    }

    #[test]
    fn an_axis_reads_two_opposed_keys() {
        let mut map = ActionMap::default();
        let turn = map
            .declare(
                "turn",
                ActionKind::Axis,
                vec![Binding::Axis {
                    negative: Source::Key(Key::ArrowLeft),
                    positive: Source::Key(Key::ArrowRight),
                }],
            )
            .expect("declared");
        let mut actions = Actions::default();

        let mut input = InputState::default();
        input.apply(InputEvent::KeyPressed(Key::ArrowLeft));
        actions.update(&map, &input);
        assert_near(actions.axis(turn), -1.0);
    }

    #[test]
    fn nothing_pressed_is_nothing_reported() {
        let (map, mut actions, move_id, fire_id) = moving_and_firing();
        actions.update(&map, &InputState::default());
        assert_near(actions.vector(move_id)[0], 0.0);
        assert!(!actions.held(fire_id));
        assert!(!actions.pressed(fire_id));
        assert!(!actions.released(fire_id));
    }
}
