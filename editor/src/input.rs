//! The keyboard and the pointer, as a running script sees them.
//!
//! The editor draws through egui, which has its own idea of a key, a button,
//! and where the mouse is. The engine has another — physical, layout-
//! independent, and the one every other Sindri host reports. A script must see
//! the engine's, or a game would behave one way in the editor and another
//! everywhere else.
//!
//! So this translates, and does nothing else. What it deliberately does not do
//! is invent a second input model: `sindri_platform::InputState` already holds
//! held state, per-frame edges, and the rule that opposed keys cancel, and all
//! of that is wanted here unchanged.
//!
//! The pointer needs one thing the keyboard does not: **a place**. egui reports
//! a position in the window, and a script must read it in the Game view's own
//! rectangle, or a game would aim somewhere else in the editor than in its real
//! build. So the view's rectangle comes in, positions are made relative to it,
//! and a pointer outside it is reported as having left — because it has, as far
//! as the game is concerned.

use eframe::egui;
use sindri_platform::{InputEvent, InputState, Key, MouseButton};

/// egui's pointer button for each engine button.
///
/// Exhaustive over what egui reports that a game can act on. `Extra1` and
/// `Extra2` have no engine button, so a script never sees them in the editor —
/// stated here rather than discovered.
const BUTTONS: &[(egui::PointerButton, MouseButton)] = &[
    (egui::PointerButton::Primary, MouseButton::Left),
    (egui::PointerButton::Middle, MouseButton::Middle),
    (egui::PointerButton::Secondary, MouseButton::Right),
];

/// egui's key for each engine key it can express.
///
/// Modifiers are absent on purpose: egui reports them as a `Modifiers` set
/// rather than as keys, and they are handled separately below. Anything not
/// listed is a key the engine knows and egui does not report, which a script
/// simply never sees in the editor — stated here rather than discovered.
const KEYS: &[(egui::Key, Key)] = &[
    (egui::Key::A, Key::A),
    (egui::Key::B, Key::B),
    (egui::Key::C, Key::C),
    (egui::Key::D, Key::D),
    (egui::Key::E, Key::E),
    (egui::Key::F, Key::F),
    (egui::Key::G, Key::G),
    (egui::Key::H, Key::H),
    (egui::Key::I, Key::I),
    (egui::Key::J, Key::J),
    (egui::Key::K, Key::K),
    (egui::Key::L, Key::L),
    (egui::Key::M, Key::M),
    (egui::Key::N, Key::N),
    (egui::Key::O, Key::O),
    (egui::Key::P, Key::P),
    (egui::Key::Q, Key::Q),
    (egui::Key::R, Key::R),
    (egui::Key::S, Key::S),
    (egui::Key::T, Key::T),
    (egui::Key::U, Key::U),
    (egui::Key::V, Key::V),
    (egui::Key::W, Key::W),
    (egui::Key::X, Key::X),
    (egui::Key::Y, Key::Y),
    (egui::Key::Z, Key::Z),
    (egui::Key::Num0, Key::Digit0),
    (egui::Key::Num1, Key::Digit1),
    (egui::Key::Num2, Key::Digit2),
    (egui::Key::Num3, Key::Digit3),
    (egui::Key::Num4, Key::Digit4),
    (egui::Key::Num5, Key::Digit5),
    (egui::Key::Num6, Key::Digit6),
    (egui::Key::Num7, Key::Digit7),
    (egui::Key::Num8, Key::Digit8),
    (egui::Key::Num9, Key::Digit9),
    (egui::Key::ArrowLeft, Key::ArrowLeft),
    (egui::Key::ArrowRight, Key::ArrowRight),
    (egui::Key::ArrowUp, Key::ArrowUp),
    (egui::Key::ArrowDown, Key::ArrowDown),
    (egui::Key::Space, Key::Space),
    (egui::Key::Enter, Key::Enter),
    (egui::Key::Escape, Key::Escape),
    (egui::Key::Tab, Key::Tab),
    (egui::Key::Backspace, Key::Backspace),
];

/// The keyboard a running script reads.
#[derive(Debug, Default)]
pub struct EditorInput {
    state: InputState,
}

impl EditorInput {
    pub const fn state(&self) -> &InputState {
        &self.state
    }

    /// Spends this frame's edges and accumulated motion.
    ///
    /// Not done per rendered frame, because a rendered frame does not always
    /// run a fixed step — at 144 Hz most do not — and an edge cleared before a
    /// step saw it is a click that never reached the game. What is held stays
    /// held; it is the *going down* that is spent, once, by the step that acted
    /// on it. Motion follows the same rule: two frames of dragging between
    /// steps should sum rather than lose the first.
    pub fn spend(&mut self, delta: std::time::Duration) {
        self.state.begin_frame(delta);
    }

    /// Reads this frame's keyboard and pointer.
    ///
    /// `listening` is false when nothing should be hearing the input — the
    /// scene is not playing, or a text field has focus. Then everything held is
    /// released, because a key left down across a pause would be down when play
    /// resumed, and a script would act on a key nobody is pressing.
    ///
    /// `view` is the Game view's rectangle, which positions are made relative
    /// to. `None` means the view is not on screen this frame, and then the
    /// pointer is reported as gone rather than as being at the window's origin.
    ///
    /// The rectangle is the one the *last* frame drew, because the editor
    /// advances scripts before it lays out. That is one frame stale only while
    /// someone is dragging the splitter, and a pointer position that lags a
    /// resize by a frame is not something a game can notice.
    pub fn update(&mut self, context: &egui::Context, listening: bool, view: Option<egui::Rect>) {
        if !listening {
            // The same path a host takes when its window loses focus, which is
            // exactly this situation: input is going somewhere else.
            self.state.apply(InputEvent::FocusChanged(false));
            return;
        }
        self.state.apply(InputEvent::FocusChanged(true));
        self.update_pointer(context, view);

        context.input(|input| {
            for event in &input.events {
                let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    ..
                } = event
                else {
                    continue;
                };
                // The operating system's key repeat is not a second press. The
                // engine's input model says so too, and a script counting
                // presses would otherwise count a held key many times.
                if *repeat {
                    continue;
                }
                let Some((_, key)) = KEYS.iter().find(|(known, _)| known == key) else {
                    continue;
                };
                self.state.apply(if *pressed {
                    InputEvent::KeyPressed(*key)
                } else {
                    InputEvent::KeyReleased(*key)
                });
            }

            // Modifiers arrive as a set rather than as key events, so they are
            // levelled to the engine's held state rather than edged. Left and
            // right cannot be told apart here; the left one is reported,
            // because a binding has to name one and left is the common reach.
            for (held, key) in [
                (input.modifiers.shift, Key::ShiftLeft),
                (input.modifiers.ctrl, Key::ControlLeft),
                (input.modifiers.alt, Key::AltLeft),
            ] {
                let was = self.state.key_down(key);
                if held && !was {
                    self.state.apply(InputEvent::KeyPressed(key));
                } else if !held && was {
                    self.state.apply(InputEvent::KeyReleased(key));
                }
            }
        });
    }

    /// This frame's pointer and fingers, in the Game view's own pixels.
    fn update_pointer(&mut self, context: &egui::Context, view: Option<egui::Rect>) {
        let Some(view) = view else {
            self.state.apply(InputEvent::PointerLeft);
            return;
        };
        // Positions arrive from egui in points, relative to the window. A
        // script reads them relative to the viewport, which is what every
        // other host reports and what makes a position mean the same thing in
        // editor Play as in the real build.
        let local = |position: egui::Pos2| (position.x - view.min.x, position.y - view.min.y);

        context.input(|input| {
            // A pointer over the inspector is not over the game. Reporting it
            // anyway would let a script aim at a panel, and clamping it to the
            // edge would be worse: the game would think the person is pointing
            // at somewhere they are not.
            match input.pointer.latest_pos() {
                Some(position) if view.contains(position) => {
                    let (x, y) = local(position);
                    self.state.apply(InputEvent::PointerMoved { x, y });
                }
                _ => self.state.apply(InputEvent::PointerLeft),
            }

            for event in &input.events {
                match event {
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        ..
                    } => {
                        let Some((_, button)) = BUTTONS.iter().find(|(known, _)| known == button)
                        else {
                            continue;
                        };
                        // A press that began outside the view is not the game's,
                        // but the release that ends it is — otherwise a button
                        // pressed on a panel and released over the game would
                        // leave the game holding a button nobody pressed, and
                        // one pressed on the game and released off it would
                        // stay down for ever.
                        if *pressed && !view.contains(*pos) {
                            continue;
                        }
                        self.state.apply(if *pressed {
                            InputEvent::ButtonPressed(*button)
                        } else {
                            InputEvent::ButtonReleased(*button)
                        });
                    }
                    egui::Event::Touch { id, phase, pos, .. } => {
                        let (x, y) = local(*pos);
                        let id = id.0;
                        self.state.apply(match phase {
                            egui::TouchPhase::Start if view.contains(*pos) => {
                                InputEvent::TouchStarted { id, x, y }
                            }
                            // A finger that started outside the view is not the
                            // game's, and the platform ignores a move for one
                            // that never started — so this needs no second
                            // check to stay consistent.
                            egui::TouchPhase::Start | egui::TouchPhase::Move => {
                                InputEvent::TouchMoved { id, x, y }
                            }
                            egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                                InputEvent::TouchEnded { id }
                            }
                        });
                    }
                    _ => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{BUTTONS, EditorInput, KEYS};
    use eframe::egui;
    use sindri_platform::{Key, MouseButton};

    /// A Game view somewhere other than the window's corner, so a test that
    /// passes with the offset ignored is a test that would have passed anyway.
    fn view() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(100.0, 40.0), egui::vec2(320.0, 200.0))
    }

    /// Runs one editor frame carrying `events`, and answers with what a script
    /// would read.
    fn frame(events: Vec<egui::Event>, view: Option<egui::Rect>) -> EditorInput {
        let context = egui::Context::default();
        let mut input = EditorInput::default();
        let raw = egui::RawInput {
            events,
            ..egui::RawInput::default()
        };
        context.run_ui(raw, |_| {}).drop_without_applying_deltas();
        input.update(&context, true, view);
        input
    }

    fn touch(phase: egui::TouchPhase, id: u64, position: egui::Pos2) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase,
            pos: position,
            force: None,
        }
    }

    /// The whole point of routing the pointer: a script must read the same
    /// position in editor Play that it reads in the real build, which means the
    /// Game view's own pixels rather than the window's.
    #[test]
    fn a_position_is_read_in_the_game_views_own_pixels() {
        let input = frame(
            vec![egui::Event::PointerMoved(egui::pos2(150.0, 90.0))],
            Some(view()),
        );
        assert_eq!(input.state().pointer_position(), Some([50.0, 50.0]));
    }

    /// A pointer over the inspector is not over the game. Clamping it to the
    /// edge would be worse than dropping it: the game would think the person is
    /// pointing somewhere they are not.
    #[test]
    fn a_pointer_outside_the_view_has_left_as_far_as_the_game_is_concerned() {
        let input = frame(
            vec![egui::Event::PointerMoved(egui::pos2(10.0, 10.0))],
            Some(view()),
        );
        assert_eq!(input.state().pointer_position(), None);
    }

    /// A workspace showing only the Scene view has nowhere for a pointer to be.
    #[test]
    fn no_game_view_means_no_pointer() {
        let input = frame(
            vec![egui::Event::PointerMoved(egui::pos2(150.0, 90.0))],
            None,
        );
        assert_eq!(input.state().pointer_position(), None);
    }

    #[test]
    fn a_press_inside_the_view_reaches_the_game() {
        let input = frame(
            vec![egui::Event::PointerButton {
                pos: egui::pos2(150.0, 90.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            Some(view()),
        );
        assert!(input.state().pointer_pressed(MouseButton::Left));
    }

    /// A button pressed on a panel and released over the game would otherwise
    /// leave the game holding a button nobody pressed.
    #[test]
    fn a_press_outside_the_view_does_not() {
        let input = frame(
            vec![egui::Event::PointerButton {
                pos: egui::pos2(10.0, 10.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            Some(view()),
        );
        assert!(!input.state().pointer_down(MouseButton::Left));
    }

    #[test]
    fn a_finger_is_routed_the_same_way_a_pointer_is() {
        let input = frame(
            vec![touch(egui::TouchPhase::Start, 1, egui::pos2(180.0, 140.0))],
            Some(view()),
        );
        assert_eq!(input.state().touch_count(), 1);
        assert_eq!(input.state().touch_at(0), Some([80.0, 100.0]));
        // And a tap is a left press, which is what makes one code path serve
        // the mouse and the finger.
        assert!(input.state().pointer_pressed(MouseButton::Left));
    }

    #[test]
    fn a_finger_that_started_outside_the_view_is_not_the_games() {
        let input = frame(
            vec![touch(egui::TouchPhase::Start, 1, egui::pos2(5.0, 5.0))],
            Some(view()),
        );
        assert_eq!(input.state().touch_count(), 0);
    }

    /// The same one-to-one rule the keys have, for the same reason: two egui
    /// buttons meaning one engine button would behave differently depending on
    /// which row was found first.
    #[test]
    fn the_button_table_is_one_to_one() {
        let mut engine = std::collections::BTreeSet::new();
        let mut egui_buttons = std::collections::BTreeSet::new();
        for (from, to) in BUTTONS {
            assert!(
                egui_buttons.insert(format!("{from:?}")),
                "{from:?} is listed twice"
            );
            assert!(engine.insert(*to), "two egui buttons both mean {to:?}");
        }
    }

    /// Every button the table produces is one the engine can name, which is
    /// what a script refers to it by.
    #[test]
    fn every_translated_button_has_a_name_a_script_can_use() {
        for (_, button) in BUTTONS {
            assert_eq!(MouseButton::from_name(button.name()), Some(*button));
        }
    }

    /// Two egui keys mapping to one engine key, or one egui key listed twice,
    /// would make a binding behave differently depending on which row was found
    /// first.
    #[test]
    fn the_translation_table_is_one_to_one() {
        let mut engine = std::collections::BTreeSet::new();
        let mut egui = std::collections::BTreeSet::new();
        for (from, to) in KEYS {
            assert!(egui.insert(format!("{from:?}")), "{from:?} is listed twice");
            assert!(engine.insert(*to), "two egui keys both mean {to:?}");
        }
        assert_eq!(engine.len(), KEYS.len());
    }

    /// Every key the table produces is one the engine can name, which is what
    /// a script refers to it by.
    #[test]
    fn every_translated_key_has_a_name_a_script_can_use() {
        for (_, key) in KEYS {
            assert_eq!(Key::from_name(key.name()), Some(*key));
        }
    }

    /// The keys gameplay reaches for first, spelled out so a reordering of
    /// either enum is caught rather than silently changing what W does.
    #[test]
    fn the_movement_keys_map_where_they_should() {
        for (from, to) in [
            (egui::Key::W, Key::W),
            (egui::Key::ArrowLeft, Key::ArrowLeft),
            (egui::Key::Space, Key::Space),
            (egui::Key::Num1, Key::Digit1),
        ] {
            assert_eq!(
                KEYS.iter()
                    .find(|(known, _)| *known == from)
                    .map(|(_, k)| *k),
                Some(to)
            );
        }
    }

    /// A pause must not leave a key held, or resuming would act on a key nobody
    /// is pressing.
    #[test]
    fn not_listening_releases_everything() {
        let context = egui::Context::default();
        let mut input = EditorInput::default();

        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        context.run_ui(raw, |_| {}).drop_without_applying_deltas();
        input.update(&context, true, Some(view()));
        assert!(input.state().key_down(Key::Space));

        input.update(&context, false, Some(view()));
        assert!(
            !input.state().key_down(Key::Space),
            "stopping play lets go of the keyboard"
        );
    }
}
