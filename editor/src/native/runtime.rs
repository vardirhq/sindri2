//! Play, pause, and stop, and what each lets move.

use eframe::egui::{self, Color32, Response, RichText, Stroke, Vec2};
use egui_material_icons::MaterialIcon;
use sindri_core::{ComponentSchemaRegistry, EngineLifecycle, EngineState, FixedStepConfig};
use sindri_decay::ScriptFailure;
use sindri_scene::SpriteAnimations;

use crate::console::Level;
use crate::ui::theme::{color, metric, radius, text};

use super::EditorApp;

pub(super) fn transport_icon(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    selected: bool,
    enabled: bool,
    tip: &str,
) -> Response {
    let tint = if selected {
        color::FORGE_BRIGHT
    } else {
        color::TEXT_MUTED
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(icon.outlined().rich_text().size(16.0).color(tint))
            .fill(if selected {
                color::EMBER
            } else {
                Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE)
            .corner_radius(radius())
            .min_size(Vec2::splat(metric::TOOL_SIZE - 2.0)),
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
        [70.0, metric::CONTROL_HEIGHT + 5.0],
        egui::Button::new(
            RichText::new(transport.play_label())
                .strong()
                .size(text::BODY)
                .color(Color32::from_rgb(26, 20, 9)),
        )
        .fill(if transport.is_playing() {
            color::FORGE
        } else {
            color::FORGE_BRIGHT
        })
        .stroke(Stroke::new(1.0, color::FORGE))
        .corner_radius(radius()),
    )
    .on_hover_text(transport.play_tip())
}

/// What the editor says when it refuses to author a running scene.
///
/// One sentence, in one place, because it is on every disabled control and in
/// the console line a refused save leaves behind.
pub(super) const PLAYING_TIP: &str = "Stop the scene first: a running scene is not the document";

/// Whether the editor may write to the world and to the file in this state.
///
/// A free function so the rule can be tested without a window and a GPU, which
/// is what an `EditorApp` needs to exist. Every guard in the editor asks this
/// one question, so there is one answer to get right.
pub(super) const fn authoring_allowed(state: EngineState) -> bool {
    !Transport::of(state).is_playing()
}

impl EditorApp {
    /// Whether the editor may write to the world and to the file right now.
    ///
    /// False while the scene is playing, and that is the whole of play mode's
    /// safety. Stop restores the world as it was when Play was pressed
    /// ([`Self::stop_playback`]), so an edit made in between is thrown away —
    /// and the history keeps its transaction, leaving undo describing changes
    /// the world no longer contains. Saving was worse still: `save` writes the
    /// live world, so Ctrl+S mid-run replaced the authored scene on disk with
    /// wherever the scripts had pushed everything, and Stop then restored a
    /// world the file no longer matched.
    ///
    /// Editing a running scene and keeping the changes is a real feature, and
    /// it is not this one: it needs history that can be rebased onto a world
    /// that moved underneath it, or a play mode that runs against a copy.
    /// Until then the honest answer is that a running scene is not the
    /// document.
    pub(super) const fn authoring_enabled(&self) -> bool {
        authoring_allowed(self.lifecycle.state())
    }

    /// Takes delivery of any script that arrived, then moves every script on by
    /// whatever this frame is worth.
    ///
    /// Called every frame, like the animations, and for the same reason: a
    /// script that will not compile should say so when the scene opens rather
    /// than waiting for someone to press Play. What the transport changes is
    /// how much time a frame is worth, so a scene at rest runs nothing.
    pub(super) fn advance_play(&mut self, context: &egui::Context) {
        if self.lifecycle.state() == EngineState::Running {
            // Nothing else asks for a frame while the pointer is still, so
            // without this a played scene runs only as fast as the mouse moves.
            context.request_repaint();
        }
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
        self.input.update(context, listening, self.game_view_rect);
        // Forgotten now that this frame's input has been read, and filled in
        // again by whichever view draws. A workspace that stops showing the
        // Game view then reports no rectangle rather than the last one it had,
        // and a script sees the pointer leave rather than sticking where the
        // view used to be.
        // Kept before the rectangle is forgotten: the screen UI is laid out in
        // the Game view's own pixels, which is the same viewport a script's
        // pointer coordinates are already in.
        let view_size = self
            .game_view_rect
            .map(|rect| (rect.width(), rect.height()));
        self.game_view_rect = None;

        // Compiled whatever the transport says, so a broken script reports at
        // the scene it was opened with and the inspector can read what a script
        // wants authored without anyone pressing Play.
        let components = self.scene.components().clone();
        for failure in self.scripts.compile(&self.world, &components) {
            self.record_script_failure(&failure);
        }

        // The same loop a shipped game runs, for the reason Play exists: a
        // scene that behaves differently here than in the build is a scene
        // nobody can trust a play-test of. `EngineCore` steps a fixed clock and
        // runs gameplay a whole number of times per frame; so does this.
        let steps = self
            .clock
            .advance(std::time::Duration::from_secs_f32(delta));
        for step in 0..steps.fixed_steps {
            self.fixed_step(&components, steps.fixed_delta, view_size);
            if step == 0 {
                // An edge belongs to one step. Spending it here rather than per
                // rendered frame is what keeps a 30 Hz display from firing a
                // button twice and a 144 Hz one from losing the click entirely.
                self.input.spend();
            }
        }
        if steps.fixed_steps == 0 && self.lifecycle.state() != EngineState::Running {
            // Nothing is going to consume them, and a scene at rest should not
            // accumulate a frame's worth of releases for ever.
            self.input.spend();
        }
    }

    /// One fixed step: everything gameplay does, in the order the engine fixes.
    fn fixed_step(
        &mut self,
        components: &ComponentSchemaRegistry,
        fixed_delta: std::time::Duration,
        view_size: Option<(f32, f32)>,
    ) {
        let delta = fixed_delta.as_secs_f32();
        // Flecks move before scripts, so one thrown this step is drawn where it
        // was thrown rather than one step along.
        self.effects.advance(fixed_delta);
        // Physics next, so a script observes the events of the step that just
        // happened and its writes take effect on the next one. `docs/physics.md`
        // fixes that order: consumers run after the step publishes.
        if let Err(error) = self.physics.step(&mut self.world, components, fixed_delta) {
            self.console.error(format!("Physics: {error}"));
        }
        // No safe area: a desktop window has no notch. A host that has one — a
        // browser on a phone — reports it, and the same scene moves its
        // anchored elements in without being edited.
        let (view_width, view_height) = view_size.unwrap_or((0.0, 0.0));
        let input_state = self.input.state();
        if let Err(error) = self.screen_ui.update(
            &self.world,
            components,
            sindri_scene::ScreenExtent::new(view_width, view_height),
            sindri_scene::PointerFrame {
                position: input_state.pointer_position(),
                pressed: input_state.pointer_pressed(sindri_platform::MouseButton::Left),
                released: input_state.pointer_released(sindri_platform::MouseButton::Left),
                down: input_state.pointer_down(sindri_platform::MouseButton::Left),
            },
        ) {
            self.console.error(format!("Screen UI: {error}"));
        }
        let (physics, events) = self.physics.for_scripts();
        let report = self.scripts.advance(
            &mut self.world,
            components,
            crate::scripts::EditorFrame {
                input: self.input.state(),
                physics: Some(sindri_decay::Physics2d {
                    world: physics,
                    events,
                }),
                screen_ui: &self.screen_ui,
                random: &mut self.random,
                saves: &mut self.saves,
                effects: &mut self.effects,
                delta_seconds: delta,
            },
        );
        // Animations move with gameplay rather than with the display, because a
        // clip that advanced per rendered frame would play at a different speed
        // in the editor than in the build.
        if let Err(error) = self.animations.advance(&self.world, components, delta) {
            self.console.error(format!("Sprite animation: {error}"));
        }

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
            self.record_script_failure(&failure);
        }
    }

    /// Says what a script did wrong, naming the entity it happened on.
    ///
    /// The runtime has only a handle, and `EntityId { index: 4, generation: 0 }`
    /// is not something anyone can look for in a hierarchy. The editor holds
    /// the world, so it says "Wisp" — and records which entity the line is
    /// about, so the console row can be the way to it.
    fn record_script_failure(&mut self, failure: &ScriptFailure) {
        match failure.entity() {
            None => self.console.error(failure.to_string()),
            Some(entity) => {
                let message = format!("{}: {}", self.entity_label(entity), failure.detail());
                self.console
                    .record_about(Level::Error, message, Some(entity));
            }
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
        // The same seed for every fresh start, so pressing Play twice gives the
        // same run twice and a bug found once can be found again. Resuming from
        // a pause deliberately does not touch it: that would replay numbers the
        // scene has already acted on.
        self.random = sindri_core::Rng::default();
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

    /// Runs exactly one fixed step of a held scene.
    ///
    /// Only while paused, because that is the only time it means anything: a
    /// running scene is already stepping, and a stopped one has nothing to
    /// step. What it is for is the bug that happens in one frame and is gone
    /// before anyone can look at it — the whole reason a debugger has a step
    /// button.
    ///
    /// It runs the same body a played frame runs, so a scene single-stepped
    /// sixty times is a scene that played for a second.
    pub(super) fn single_step(&mut self, context: &egui::Context) {
        if self.lifecycle.state() != EngineState::Paused {
            return;
        }
        let components = self.scene.components().clone();
        let view_size = self
            .game_view_rect
            .map(|rect| (rect.width(), rect.height()));
        // Read now, because a step taken from a keyboard shortcut has input
        // that a paused frame never delivered.
        let _ = context;
        self.fixed_step(&components, self.clock.fixed_delta(), view_size);
        self.input.spend();
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
        // A fleck outliving the run that threw it would be a scene at rest that
        // is still moving.
        self.effects.clear();
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
}
