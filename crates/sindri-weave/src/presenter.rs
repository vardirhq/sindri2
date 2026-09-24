//! The live presentation a running game draws and hit-tests each frame.

use sindri_core::{EntityId, World};
use weave::{States, Stylesheet, Viewport};

use crate::{ApplyError, MAX_HIERARCHY_DEPTH, Transitions, UiStates, apply};

/// The live presentation of a running game: resolved every frame from the
/// authored world, with what the pointer is doing and with transitions easing
/// between one frame's values and the next.
///
/// A host keeps one for as long as the game runs, moves its clock on each
/// frame, and hands it the pointer state it read. What it returns is what to
/// draw and what to hit-test; the authored world is never changed.
#[derive(Clone, Debug, Default)]
pub struct Presenter {
    transitions: Transitions,
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

    /// Presents `authored` through each stylesheet in order, for this
    /// viewport and these pointer states.
    pub fn present(
        &mut self,
        authored: &World,
        stylesheets: &[Stylesheet],
        viewport: Viewport,
        states: &UiStates,
    ) -> Result<World, ApplyError> {
        let mut world = authored.clone();
        for (sheet, stylesheet) in stylesheets.iter().enumerate() {
            apply(
                &mut world,
                stylesheet,
                viewport,
                states,
                Some((&mut self.transitions, sheet)),
            )?;
        }
        Ok(world)
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
