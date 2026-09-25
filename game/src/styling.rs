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

use sindri_scene::{ScreenExtent, UiTextSizes};

use crate::Session;
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

/// The session's side of styling: settling, laying pointer states over a
/// draw, and recording what was drawn for the clicks until the next one.
impl Session {
    /// The game's stylesheets. A game with none is drawn and clicked as
    /// authored.
    #[must_use]
    pub fn with_styles(mut self, stylesheets: Vec<weave::Stylesheet>) -> Self {
        self.styles = Styles::new(stylesheets);
        self
    }

    /// Styles `world` with the game's stylesheets for `viewport`. The host
    /// does this when the game starts and whenever the screen changes shape;
    /// the scripts then run on the styled world. Nothing for a game with no
    /// stylesheets.
    pub fn settle_styles(
        &mut self,
        world: &mut World,
        viewport: weave::Viewport,
    ) -> Result<(), CausewayError> {
        let Some(styles) = &mut self.styles else {
            return Ok(());
        };
        styles.settle(world, viewport)?;
        self.screen_ui.lay_out(
            world,
            &self.components,
            ScreenExtent::new(viewport.width, viewport.height),
            &self.text_sizes,
        )?;
        Ok(())
    }

    /// Tells the session what the host just drew: `world` as drawn, at
    /// `viewport`, with its text measured. Clicks are hit-tested against this
    /// until the next draw, so an element sized by its words is clicked at
    /// the size it is drawn.
    pub fn record_drawn(
        &mut self,
        world: &World,
        viewport: weave::Viewport,
        text_sizes: UiTextSizes,
    ) -> Result<(), CausewayError> {
        self.text_sizes = text_sizes;
        if self.styles.is_some() {
            self.screen_ui.lay_out(
                world,
                &self.components,
                ScreenExtent::new(viewport.width, viewport.height),
                &self.text_sizes,
            )?;
        }
        Ok(())
    }

    /// Lays the pointer's states over the settled `world` for a draw at
    /// `viewport`, which clicks are hit-tested against from then on. The
    /// host must undo it once drawn, before the next step; `None` when the
    /// game has no stylesheets.
    pub fn style(
        &mut self,
        world: &mut World,
        viewport: weave::Viewport,
    ) -> Result<Option<sindri_weave::Undo>, CausewayError> {
        let Some(styles) = &mut self.styles else {
            return Ok(None);
        };
        styles.set_viewport(viewport);
        styles.style(world, &self.screen_ui).map(Some)
    }
}
