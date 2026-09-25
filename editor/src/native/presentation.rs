//! Resolving the scene through the project's Weave styles for a viewport.

use eframe::egui::Rect;
use weave::Viewport as WeaveViewport;

use super::EditorApp;

impl EditorApp {
    /// Resolves the authored world through the project's Weave presentation.
    ///
    /// Kept outside `render_view` because resolving presentation is one concern
    /// of its own and because failures need the same console/render-error path
    /// whichever viewport asked for them.
    pub(super) fn resolve_presentation(
        &mut self,
        editing: bool,
        rect: Rect,
    ) -> Option<sindri_core::World> {
        if self.styles.is_empty() {
            return None;
        }
        let viewport = self.presentation_viewport(editing, rect);
        // The Game view of a running game is live: the pointer's states and
        // transitions. Anything else is the authored presentation.
        let presented = if !editing && !self.authoring_enabled() {
            let states = sindri_weave::pointer_states(
                &self.world,
                self.screen_ui.hovered(),
                self.screen_ui.active(),
            );
            self.styles.present_live(&self.world, viewport, &states)
        } else {
            if self.authoring_enabled() {
                self.styles.stop_live();
            }
            self.styles.resolve(&self.world, viewport)
        };
        match presented {
            Ok(world) => Some(world),
            Err(error) => {
                let failure = format!("Weave: {error}");
                self.console.fail(&failure, None);
                if self.render_error.is_none() {
                    self.render_error = Some(failure);
                }
                None
            }
        }
    }

    /// The logical screen dimensions Weave resolves against.
    ///
    /// A named device uses its real logical size, not the number of editor
    /// points its preview happened to fit into. In Free mode the Game view is
    /// the screen. The Scene view follows that Game rectangle when one has been
    /// drawn, so both views choose the same media queries while shown together.
    fn presentation_viewport(&self, editing: bool, rect: Rect) -> WeaveViewport {
        let (width, height) = self.game_device.size.unwrap_or_else(|| {
            if editing {
                self.game_view_rect
                    .map_or((rect.width(), rect.height()), |game| {
                        (game.width(), game.height())
                    })
            } else {
                (rect.width(), rect.height())
            }
        });
        WeaveViewport {
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }
}
