//! The keyboard, as a running script sees it.
//!
//! The editor draws through egui, which has its own idea of a key. The engine
//! has another — physical, layout-independent, and the one every other Sindri
//! host reports. A script must see the engine's, or a game would behave one way
//! in the editor and another everywhere else.
//!
//! So this translates, and does nothing else. What it deliberately does not do
//! is invent a second input model: `sindri_platform::InputState` already holds
//! held state, per-frame edges, and the rule that opposed keys cancel, and all
//! of that is wanted here unchanged.

use eframe::egui;
use sindri_platform::{InputEvent, InputState, Key};

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

    /// Reads this frame's keyboard.
    ///
    /// `listening` is false when nothing should be hearing the keyboard — the
    /// scene is not playing, or a text field has focus. Then everything held is
    /// released, because a key left down across a pause would be down when play
    /// resumed, and a script would act on a key nobody is pressing.
    pub fn update(&mut self, context: &egui::Context, listening: bool) {
        self.state.begin_frame();
        if !listening {
            // The same path a host takes when its window loses focus, which is
            // exactly this situation: input is going somewhere else.
            self.state.apply(InputEvent::FocusChanged(false));
            return;
        }
        self.state.apply(InputEvent::FocusChanged(true));

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
}

#[cfg(test)]
mod tests {
    use super::{EditorInput, KEYS};
    use eframe::egui;
    use sindri_platform::Key;

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
        input.update(&context, true);
        assert!(input.state().key_down(Key::Space));

        input.update(&context, false);
        assert!(
            !input.state().key_down(Key::Space),
            "stopping play lets go of the keyboard"
        );
    }
}
