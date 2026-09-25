//! The live presentation a running game draws and hit-tests each frame.

use sindri_core::{EntityId, World};
use weave::{States, Stylesheet, Viewport};

use crate::{ApplyError, Declared, MAX_HIERARCHY_DEPTH, Pass, Transitions, UiStates, Undo, apply};

/// The live presentation of a running game.
///
/// Two layers, as a browser has them. The stylesheet's rules are
/// [settled](Self::settle) into the world when the game starts and whenever
/// the screen changes shape: that is the world the scripts read, and a script
/// that then writes a value keeps it, as an inline style beats a stylesheet.
/// Each frame, only what the pointer's states and running transitions change
/// is [laid over](Self::present_over) that, for the draw and the hit-test,
/// and taken off again. A frame with nothing hovered and nothing easing costs
/// nothing.
#[derive(Clone, Debug, Default)]
pub struct Presenter {
    transitions: Transitions,
    /// What each stylesheet settled each element with.
    settled: Vec<Declared>,
    /// Whether the last frame had nothing to lay over, so this one, if it has
    /// nothing either, can be skipped.
    idle: bool,
}

impl Presenter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves transitions on by `seconds`.
    pub fn advance(&mut self, seconds: f32) {
        self.transitions.advance(seconds);
    }

    /// Whether a transition is still running.
    #[must_use]
    pub fn animating(&self) -> bool {
        self.transitions.animating()
    }

    /// Presents `authored` through each stylesheet in order into a copy, for
    /// this viewport and these pointer states, with transitions.
    ///
    /// For a tool that shows a world it does not run. A running game settles
    /// its world and lays states over it instead.
    pub fn present(
        &mut self,
        authored: &World,
        stylesheets: &[Stylesheet],
        viewport: Viewport,
        states: &UiStates,
    ) -> Result<World, ApplyError> {
        let mut world = authored.clone();
        for (sheet, stylesheet) in stylesheets.iter().enumerate() {
            let pass = Pass {
                transitions: Some((&mut self.transitions, sheet)),
                ..Pass::default()
            };
            apply(&mut world, stylesheet, viewport, states, pass)?;
        }
        Ok(world)
    }

    /// Styles `world` with the stylesheets' rules, no pointer states, for
    /// this viewport: the world the game runs on, until the screen changes.
    pub fn settle(
        &mut self,
        world: &mut World,
        stylesheets: &[Stylesheet],
        viewport: Viewport,
    ) -> Result<(), ApplyError> {
        self.settled = vec![Declared::new(); stylesheets.len()];
        for ((sheet, stylesheet), settled) in stylesheets.iter().enumerate().zip(&mut self.settled)
        {
            // Through the transitions, so the first hover eases from here.
            let pass = Pass {
                transitions: Some((&mut self.transitions, sheet)),
                record: Some(settled),
                ..Pass::default()
            };
            apply(world, stylesheet, viewport, &UiStates::new(), pass)?;
        }
        self.idle = true;
        Ok(())
    }

    /// Lays what `states` and running transitions change over a
    /// [settled](Self::settle) `world`, in place, and returns what takes it
    /// off again. The caller draws or hit-tests, then undoes it before
    /// anything else reads the world. On an error the world is put back
    /// before the error is returned.
    pub fn present_over(
        &mut self,
        world: &mut World,
        stylesheets: &[Stylesheet],
        viewport: Viewport,
        states: &UiStates,
    ) -> Result<Undo, ApplyError> {
        let mut undo = Undo::default();
        if states.is_empty() && self.idle && !self.transitions.animating() {
            return Ok(undo);
        }
        for (sheet, stylesheet) in stylesheets.iter().enumerate() {
            let pass = Pass {
                transitions: Some((&mut self.transitions, sheet)),
                undo: Some(&mut undo),
                over: self.settled.get(sheet),
                ..Pass::default()
            };
            if let Err(error) = apply(world, stylesheet, viewport, states, pass) {
                undo.undo(world);
                return Err(error);
            }
        }
        // One pass with no states after the pointer leaves, so what it
        // changed starts easing back; then nothing until it returns.
        self.idle = states.is_empty();
        Ok(undo)
    }
}

/// The states the pointer puts elements in: the element under it hovered,
/// the one held down active, and, as in CSS, every element containing either
/// in the same state, so `.card:hover` holds while the pointer is over the
/// card's button.
#[must_use]
pub fn pointer_states(
    world: &World,
    hovered: Option<EntityId>,
    active: Option<EntityId>,
) -> UiStates {
    let mut states = UiStates::new();
    for (start, state) in [(hovered, States::HOVER), (active, States::ACTIVE)] {
        let mut current = start;
        for _ in 0..MAX_HIERARCHY_DEPTH {
            let Some(entity) = current else {
                break;
            };
            let entry = states.entry(entity).or_insert(States::NONE);
            *entry = entry.with(state);
            current = world.get(entity).and_then(|data| data.parent);
        }
    }
    states
}
