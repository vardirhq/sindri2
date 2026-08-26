//! Play, pause, and stop, and what each lets move.

use eframe::egui::{self, Color32, Response, RichText, Stroke, Vec2};
use egui_material_icons::MaterialIcon;
use sindri_core::{EngineLifecycle, EngineState, FixedStepConfig};
use sindri_scene::SpriteAnimations;

use super::EditorApp;
use super::theme::{ACCENT, ACCENT_BRIGHT, BORDER, TEXT_FAINT};

pub(super) fn transport_icon(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    selected: bool,
    enabled: bool,
    tip: &str,
) -> Response {
    let color = if selected {
        ACCENT
    } else if enabled {
        TEXT_FAINT
    } else {
        BORDER
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(icon.outlined().rich_text().size(16.0).color(color))
            .frame(false)
            .min_size(Vec2::new(26.0, 26.0)),
    )
    .on_hover_text(tip)
}

/// What the transport is doing, in the words the buttons use.
///
/// There used to be four controls for three states: a stop icon, a pause icon,
/// a play icon, and an accent button — and the accent button said "Stop" while
/// running but paused when pressed, while the play icon did the same. Whatever
/// each was meant to be, together they were three ways to guess.
///
/// Two controls cover it, the way Unity's do. Play enters and leaves play mode;
/// Pause holds and releases what is already playing; and this says which of the
/// three states the editor is actually in, so the answer is read rather than
/// inferred from which icon looks lit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Transport {
    Editing,
    Playing,
    Paused,
}

impl Transport {
    pub(super) const fn of(state: EngineState) -> Self {
        match state {
            EngineState::Running => Self::Playing,
            EngineState::Paused => Self::Paused,
            _ => Self::Editing,
        }
    }

    /// Whether the scene is in play mode, running or held.
    pub(super) const fn is_playing(self) -> bool {
        !matches!(self, Self::Editing)
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Editing => "Editing",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }

    /// What pressing Play would do, said as the button's own label.
    pub(super) const fn play_label(self) -> &'static str {
        if self.is_playing() { "Stop" } else { "Play" }
    }

    pub(super) const fn play_tip(self) -> &'static str {
        if self.is_playing() {
            "Stop the scene and put back everything playing changed  (Ctrl+P)"
        } else {
            "Run the scene's scripts and animations  (Ctrl+P)"
        }
    }

    pub(super) const fn pause_tip(self) -> &'static str {
        match self {
            Self::Editing => "Nothing is running to pause",
            Self::Playing => "Hold the scene where it is  (Ctrl+Shift+P)",
            Self::Paused => "Carry on from here  (Ctrl+Shift+P)",
        }
    }
}

/// How much time an animation takes from a frame, given where the transport is.
///
/// Only a running engine moves an animation on; paused holds where it is, which
/// is not the same as advancing by nothing only because stopping resets. The cap
/// is the same quarter second `FixedStepConfig` caps a frame at, shared rather
/// than chosen again here, so a window left behind another one for a minute
/// comes back where it left off rather than wherever a minute of animation
/// lands.
pub(super) fn animation_delta(state: EngineState, frame_seconds: f32) -> f32 {
    if state != EngineState::Running || !frame_seconds.is_finite() || frame_seconds < 0.0 {
        return 0.0;
    }
    frame_seconds.min(FixedStepConfig::default().max_frame_delta.as_secs_f32())
}

pub(super) fn initialized_lifecycle() -> EngineLifecycle {
    let mut lifecycle = EngineLifecycle::new();
    lifecycle
        .initialize()
        .expect("a new lifecycle always accepts initialization");
    lifecycle
}

/// The one button that enters and leaves play mode.
///
/// It is labelled with what pressing it does rather than with what the editor
/// is doing, because a button is a verb. What the editor is doing is said
/// beside it, in words, by [`Transport::label`].
pub(super) fn play_button(ui: &mut egui::Ui, transport: Transport) -> Response {
    ui.add_sized(
        [72.0, 29.0],
        egui::Button::new(
            RichText::new(transport.play_label())
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(28, 23, 12)),
        )
        .fill(ACCENT_BRIGHT)
        .stroke(Stroke::new(1.0, ACCENT)),
    )
    .on_hover_text(transport.play_tip())
}

impl EditorApp {
    /// Takes delivery of any script that arrived, then moves every script on by
    /// whatever this frame is worth.
    ///
    /// Called every frame, like the animations, and for the same reason: a
    /// script that will not compile should say so when the scene opens rather
    /// than waiting for someone to press Play. What the transport changes is
    /// how much time a frame is worth, so a scene at rest runs nothing.
    pub(super) fn advance_scripts(&mut self, context: &egui::Context) {
        let notes = self.scripts.poll();
        self.record_script_notes(notes);

        let delta = animation_delta(
            self.lifecycle.state(),
            context.input(|input| input.stable_dt),
        );
        // The keyboard is read only while the scene is actually running, and
        // never while a text field has it: renaming an entity to "Wall" must
        // not walk the player left. Read every frame regardless, so that
        // stopping releases what was held rather than leaving it down.
        let listening =
            self.lifecycle.state() == EngineState::Running && !context.egui_wants_keyboard_input();
        self.input.update(context, listening);

        // Compiled whatever the transport says, so a broken script reports at
        // the scene it was opened with and the inspector can read what a script
        // wants authored without anyone pressing Play.
        let components = self.scene.components().clone();
        for failure in self.scripts.compile(&self.world, &components) {
            self.console.error(failure.to_string());
        }

        if delta == 0.0 {
            return;
        }
        let report = self
            .scripts
            .advance(&mut self.world, &components, self.input.state(), delta);

        for message in report.printed {
            // Named by entity, because "moving" is not something an author can
            // act on when six entities run the same script.
            self.console.info(format!(
                "{}: {}",
                self.entity_label(message.entity),
                message.message
            ));
        }
        for failure in report.failures {
            // Collapsed by the console the same way a broken clip is: a script
            // that fails does it sixty times a second, and one line with a
            // count says more than sixty that scroll.
            self.console.error(failure.to_string());
        }
    }

    /// Enters play mode, or leaves it.
    ///
    /// One button, two directions, and no third meaning: pressing it while
    /// something is playing stops it rather than pausing it, which is what its
    /// label says and what the equivalent button does everywhere else. Pausing
    /// is [`Self::toggle_pause`].
    ///
    /// Play and stop move the engine lifecycle rather than a display flag, so
    /// the editor exercises the same transitions a runtime host does.
    pub(super) fn toggle_play_mode(&mut self) {
        if Transport::of(self.lifecycle.state()).is_playing() {
            self.stop_playback();
            return;
        }
        // Taken before the first frame runs, and only on a fresh start rather
        // than on resume, so pausing and carrying on does not move the point
        // stop returns to.
        self.play_snapshot = Some(self.world.clone());
        if let Err(error) = self.lifecycle.start() {
            self.report(error.to_string());
        }
    }

    /// Holds a running scene where it is, or lets a held one carry on.
    ///
    /// Does nothing outside play mode, where there is nothing to hold: the
    /// button is disabled there, and this agrees with it rather than starting
    /// something the author did not ask to start.
    pub(super) fn toggle_pause(&mut self) {
        let result = match self.lifecycle.state() {
            EngineState::Running => self.lifecycle.pause(),
            EngineState::Paused => self.lifecycle.resume(),
            _ => return,
        };
        if let Err(error) = result {
            self.report(error.to_string());
        }
    }

    /// Ends a play session, putting back what playing changed.
    ///
    /// Scripts write to the world, so the world is part of what playing
    /// changed — and restoring it is what makes Play safe to press on work in
    /// progress. The snapshot is the world as it was when Play was pressed,
    /// not the authored document: a scene edited and then played must come
    /// back to the edit, or pressing Play would quietly discard it.
    ///
    /// Undo history is deliberately left alone. A script moving something is
    /// not an action the author took, so it was never on the history, and
    /// putting the world back does not change what undo means.
    pub(super) fn stop_playback(&mut self) {
        if let Err(error) = self.lifecycle.stop() {
            self.report(error.to_string());
        }
        self.animations = SpriteAnimations::new();
        // Entity handles survive, because this is the same world restored
        // rather than one reloaded from a document — so the selection and the
        // history keep pointing at the things they named.
        if let Some(snapshot) = self.play_snapshot.take() {
            self.world = snapshot;
        }
        self.scripts.restart();
    }

    /// Moves every animated sprite on by whatever this frame is worth.
    ///
    /// Called every frame rather than only while playing, so a scene at rest
    /// shows its clips' first frames and a clip that cannot be played says so
    /// without anyone pressing Play. What the transport changes is how much time
    /// a frame is worth, which is [`animation_delta`].
    pub(super) fn advance_animations(&mut self, context: &egui::Context) {
        if self.lifecycle.state() == EngineState::Running {
            // Nothing else asks for a frame while the pointer is still, so
            // without this an animation plays only as fast as the mouse moves.
            context.request_repaint();
        }
        let delta = animation_delta(
            self.lifecycle.state(),
            context.input(|input| input.stable_dt),
        );
        if let Err(error) = self
            .animations
            .advance(&self.world, self.scene.components(), delta)
        {
            // Collapsed by the console the same way a render failure is: a
            // broken clip fails every frame, and one entry with a count says
            // more than sixty a second.
            self.console.error(error.to_string());
        }
    }
}
