//! The game's stylesheets: settled into the world, with the pointer's states
//! laid over it for each draw.
//!
//! Settled when the game starts and when the screen changes shape, so the
//! scripts run on a styled world and what they write stays written. Each draw
//! lays only what `:hover`, `:active` and running transitions change over
//! that, in place, and takes it off straight after: no copy of the world,
//! which is a copy of the level. See `sindri_weave::Presenter`.

use sindri_core::World;
use sindri_scene::ScreenUi;
use sindri_weave::{Presenter, Undo};
use weave::{Stylesheet, Viewport};

use crate::error::CausewayError;

/// A game's stylesheets and the presenter that runs their transitions.
#[derive(Debug)]
pub(crate) struct Styles {
    presenter: Presenter,
    stylesheets: Vec<Stylesheet>,
    /// The screen the host last drew at, which a click is hit-tested against
    /// until the next draw says otherwise.
    viewport: Viewport,
}

impl Styles {
    /// `None` for a game with no stylesheets, which is drawn and clicked as
    /// it is authored.
    pub(crate) fn new(stylesheets: Vec<Stylesheet>) -> Option<Self> {
        (!stylesheets.is_empty()).then(|| Self {
            presenter: Presenter::new(),
            stylesheets,
            viewport: Viewport {
                width: 1.0,
                height: 1.0,
            },
        })
    }

    pub(crate) fn advance(&mut self, seconds: f32) {
        self.presenter.advance(seconds);
    }

    pub(crate) fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    /// Styles `world` with the stylesheets' rules for `viewport`: what the
    /// game runs on until the screen changes shape.
    pub(crate) fn settle(
        &mut self,
        world: &mut World,
        viewport: Viewport,
    ) -> Result<(), CausewayError> {
        self.viewport = viewport;
        self.presenter
            .settle(world, &self.stylesheets, viewport)
            .map_err(|error| CausewayError::Weave(error.to_string()))
    }

    /// Styles `world` for what the pointer is doing to `screen_ui`; the
    /// caller must undo it before anything else reads the world.
    pub(crate) fn style(
        &mut self,
        world: &mut World,
        screen_ui: &ScreenUi,
    ) -> Result<Undo, CausewayError> {
        let states = sindri_weave::pointer_states(world, screen_ui.hovered(), screen_ui.active());
        self.presenter
            .present_over(world, &self.stylesheets, self.viewport, &states)
            .map_err(|error| CausewayError::Weave(error.to_string()))
    }
}
