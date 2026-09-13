//! The rendered views: their targets, their renderers, and drawing one.

use eframe::{
    egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke},
    wgpu,
};
use sindri_core::EngineState;
use sindri_render::{
    FrameRenderers, FrameTarget, GlyphRenderer, ShapeRenderer, SpriteBatchRenderer, TextRenderer,
    TexturedCubeRenderer, Viewport, ViewportTarget, encode_prepared_frame,
};
use sindri_scene::{CameraView, SceneRuntime, UiCanvas};
use weave::Viewport as WeaveViewport;

use super::block_pointer::TileVolumeHover;
use super::camera::{EditorCamera, camera_for};
use super::frame::physical_viewport_dimension;
use super::hierarchy::row::entity_name;
use super::overlay::{
    ViewportStatus, paint_runtime_overlay, paint_selection_marks, paint_transform_gizmo,
    paint_viewport_border,
};
use super::pointer::TilemapHover;
use super::scene_io::SceneSource;
use super::{EditorApp, INITIAL_VIEWPORT_HEIGHT, INITIAL_VIEWPORT_WIDTH, WorkspaceTab};
use crate::tile_volume::TilePlacement;
use crate::ui::theme::{color, text};

enum PaintHover<'a> {
    Tilemap(&'a TilemapHover),
    TileVolume(&'a TileVolumeHover),
}

struct ViewInteraction {
    editing: bool,
    painting: bool,
    camera: CameraView,
    tilemap_hover: Option<TilemapHover>,
    volume_hover: Option<TileVolumeHover>,
}

impl ViewInteraction {
    fn hover(&self) -> Option<PaintHover<'_>> {
        self.tilemap_hover
            .as_ref()
            .map(PaintHover::Tilemap)
            .or_else(|| self.volume_hover.as_ref().map(PaintHover::TileVolume))
    }
}

/// The GPU pipelines every viewport draws with.
///
/// Held once rather than per viewport: a pipeline does not depend on which
/// camera is looking, and two viewports that each built their own would pay
/// twice for the same thing. The textures used to live here too, handed over by
/// the cube example; they belong to the open scene, which is where they are now.
pub(super) struct SceneRenderers {
    pub(super) cube: TexturedCubeRenderer,
    pub(super) sprites: SpriteBatchRenderer,
    pub(super) text: TextRenderer,
    pub(super) glyphs: GlyphRenderer,
    pub(super) shapes: ShapeRenderer,
}

impl SceneRenderers {
    pub(super) fn new(render_state: &eframe::egui_wgpu::RenderState) -> Self {
        Self {
            cube: TexturedCubeRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            sprites: SpriteBatchRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            text: TextRenderer::new(),
            glyphs: GlyphRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            shapes: ShapeRenderer::new(&render_state.device, ViewportTarget::FORMAT),
        }
    }
}

pub(super) struct RuntimeViewport {
    render_state: eframe::egui_wgpu::RenderState,
    target: ViewportTarget,
    texture_id: egui::TextureId,
}

impl RuntimeViewport {
    pub(super) fn new(render_state: eframe::egui_wgpu::RenderState, label: &str) -> Self {
        let target = ViewportTarget::new(
            &render_state.device,
            label,
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
        );
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            target.sampled(),
            wgpu::FilterMode::Linear,
        );
        Self {
            render_state,
            target,
            texture_id,
        }
    }

    /// The shape of what this viewport draws into.
    ///
    /// Read from the target rather than from whatever rect was last laid out,
    /// so it answers the same thing whether or not this view was drawn in the
    /// current layout — a Scene view alone in the window still knows what the
    /// Game view frames.
    pub(super) fn aspect(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let (width, height) = (self.target.width() as f32, self.target.height() as f32);
        if height <= 0.0 { 1.0 } else { width / height }
    }

    fn render(
        &mut self,
        renderers: &mut SceneRenderers,
        source: SceneSource<'_>,
        size: (u32, u32),
        camera: CameraView,
        canvas: UiCanvas,
    ) -> Result<(), String> {
        self.resize(size.0, size.1);
        let prepared = source
            .scene
            .extract_animated(
                source.world,
                Viewport::new(self.target.width(), self.target.height()),
                camera,
                source.textures.bindings(),
                SceneRuntime::default()
                    .with_animations(source.animations)
                    .with_effects(source.effects)
                    .with_tile_sets(source.textures.tile_sets())
                    .with_canvas(canvas),
            )
            .map_err(|error| error.to_string())?;
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri editor runtime viewport encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut renderers.cube,
                sprites: &mut renderers.sprites,
                text: &mut renderers.text,
                glyphs: &mut renderers.glyphs,
                shapes: &mut renderers.shapes,
                textures: source.textures.registry(),
            },
            &self.render_state.device,
            &self.render_state.queue,
            &mut encoder,
            FrameTarget {
                color: self.target.attachment(),
                depth: self.target.depth(),
            },
            &prepared,
        )
        .map_err(|error| error.to_string())?;
        self.render_state.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Resizes the target and, when it actually changed, points egui at the
    /// new texture. The target answers whether that happened.
    fn resize(&mut self, width: u32, height: u32) {
        if !self.target.resize(&self.render_state.device, width, height) {
            return;
        }
        self.render_state
            .renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                &self.render_state.device,
                self.target.sampled(),
                wgpu::FilterMode::Linear,
                self.texture_id,
            );
    }
}

/// What the viewport answers to.
///
/// Clicks as well as drags, and the click half is not optional: egui sets a
/// response's clicked flag only for a widget whose sense includes clicks, and
/// this used to be `Sense::drag()`. So `clicked_by` was always false and
/// *nothing* in the Scene view could be selected by clicking it, whatever the
/// picking code decided. The tile brush was half-dead the same way — it
/// painted on a drag and ignored a single click.
pub(super) const fn viewport_sense() -> Sense {
    Sense::CLICK.union(Sense::DRAG).union(Sense::FOCUSABLE)
}

impl EditorApp {
    /// Draws the cell a tilemap stroke would edit without changing the scene.
    fn paint_tilemap_hover(&self, ui: &egui::Ui, hover: &TilemapHover) {
        // The brush wears the editor's own two answers: forge for a stroke that
        // writes, danger for one that erases.
        let tint = if self.tilemap_tool.erase {
            color::DANGER
        } else {
            color::FORGE
        };
        let fill = tint.gamma_multiply(0.16);
        let stroke = Stroke::new(2.0, tint);
        ui.painter()
            .add(Shape::convex_polygon(hover.outline.to_vec(), fill, stroke));
        ui.painter().text(
            hover.outline[0],
            Align2::LEFT_BOTTOM,
            format!("{}, {}", hover.column, hover.row),
            FontId::proportional(text::NOTE),
            color::TEXT,
        );
    }

    /// Remembers where a view was drawn, for the two things that need it.
    ///
    /// A script's pointer coordinates are in the Game view's own pixels, and an
    /// overlay anchors to the corners of the world as drawn rather than of the
    /// panel it was drawn in — the panel includes a tab strip and a toolbar,
    /// and an overlay covering those would put the hierarchy on top of the tabs
    /// that switch Scene and Game.
    fn record_view_rect(&mut self, editing: bool, rect: egui::Rect) {
        // Only the Game view records its own. The Scene view must not clear it:
        // an arrangement showing both draws the Game view first, so clearing
        // here would throw away the rectangle that was just recorded.
        // Forgetting a view that stopped being drawn is `advance_scripts`'s
        // job, once per frame.
        if !editing {
            self.game_view_rect = Some(rect);
        }
        // Only the centre's viewport is the canvas: a Scene view someone
        // dragged into a corner is not what the other corners arrange
        // themselves against.
        if self.dock.drawing_main {
            self.dock.canvas = rect;
        }
    }

    /// Draws one view of the world into whatever space `ui` has left.
    ///
    /// The Scene view takes camera input and wears editor chrome; the Game view
    /// takes neither, because chrome painted across what the player would see
    /// makes it something else. Both go through here so the two views cannot
    /// drift into being two renderers.
    pub(super) fn render_view(&mut self, ui: &mut egui::Ui, tab: WorkspaceTab) {
        let context = ui.ctx().clone();
        let (panel, response) = ui.allocate_exact_size(ui.available_size(), viewport_sense());
        // The Game view is drawn at the shape of the screen it is standing in
        // for, which is the panel's own unless someone chose otherwise. The
        // Scene view is always the panel: it is a place to work, not a picture
        // of a device.
        let rect = if tab == WorkspaceTab::Scene {
            panel
        } else {
            self.game_device.fit(panel)
        };
        let interaction = self.interact_view(&context, &response, rect, tab);
        let editing = interaction.editing;
        let camera = interaction.camera;
        let scale = context.pixels_per_point();
        // Worked out before the viewport is borrowed: the canvas and Weave
        // viewport are facts about the project's screen, not about the GPU
        // surface being drawn into.
        let canvas = self.canvas_for(editing);
        let presented = self.resolve_presentation(editing, rect);
        let source_world = presented.as_ref().unwrap_or(&self.world);
        let viewport_size = (
            physical_viewport_dimension(rect.width(), scale),
            physical_viewport_dimension(rect.height(), scale),
        );
        let viewport = if editing {
            &mut self.scene_viewport
        } else {
            &mut self.game_viewport
        };
        let failure = viewport
            .render(
                &mut self.renderers,
                SceneSource {
                    scene: &self.scene,
                    world: source_world,
                    animations: &self.animations,
                    effects: &self.effects,
                    textures: &self.textures,
                },
                viewport_size,
                camera,
                // The Scene view puts the UI in the world, where panning and
                // zooming reach it; the Game view is the screen, so there the
                // overlay is the screen.
                canvas,
            )
            .err();
        // Two views can be live at once, and the first thing to go wrong is the
        // thing worth reading, so a later success does not erase it.
        if let Some(failure) = failure {
            // The console collapses this: a render failure recurs every frame,
            // and one entry with a count says more than sixty a second.
            self.console.error(&failure);
            if self.render_error.is_none() {
                self.render_error = Some(failure);
            }
        }
        ui.painter().image(
            viewport.texture_id,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        if editing {
            // Measured before the chrome is drawn, because measuring a string
            // shapes it and the painter takes only a shared borrow.
            let text_rect = self.selected_text_rect(camera);
            self.paint_scene_chrome(
                ui,
                rect,
                camera,
                interaction.hover(),
                interaction.painting,
                text_rect,
            );
        } else {
            // The unused space is painted out rather than left showing the
            // panel, so the shape being previewed reads as the screen and not
            // as a window that failed to fill.
            if rect != panel {
                ui.painter()
                    .rect_filled(panel, 0.0, crate::ui::theme::color::WELL);
                ui.painter().image(
                    viewport.texture_id,
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            paint_viewport_border(ui.painter(), rect, self.problem());
        }
        context.request_repaint();
    }

    fn interact_view(
        &mut self,
        context: &egui::Context,
        response: &egui::Response,
        rect: Rect,
        tab: WorkspaceTab,
    ) -> ViewInteraction {
        let editing = tab == WorkspaceTab::Scene;
        self.record_view_rect(editing, rect);
        let volume_painting = editing && self.tile_volume_tool.brush().is_some();
        let painting = volume_painting || (editing && self.tilemap_tool.brush().is_some());
        let camera_before_input = self.scene_camera();
        let gizmo_owned = editing
            && !painting
            && self
                .gizmo_visual(rect, camera_before_input)
                .is_some_and(|(camera, anchoring, visual)| {
                    self.interact_gizmo(rect, response, camera, anchoring, &visual)
                });
        if editing {
            self.move_camera(context, response, rect.height(), painting || gizmo_owned);
        }
        let camera = if editing {
            self.scene_camera()
        } else {
            camera_for(tab, EditorCamera::default())
        };
        let volume_hover = editing
            .then(|| self.tile_volume_hover(rect, response.hover_pos(), camera))
            .flatten();
        let tilemap_hover = (!volume_painting && editing)
            .then(|| self.tilemap_hover(rect, response.hover_pos(), camera))
            .flatten();
        self.apply_paint_input(response, volume_hover.as_ref(), tilemap_hover.as_ref());
        if editing {
            self.select_viewport_click(rect, response, camera, painting || gizmo_owned);
        }
        ViewInteraction {
            editing,
            painting,
            camera,
            tilemap_hover,
            volume_hover,
        }
    }

    fn apply_paint_input(
        &mut self,
        response: &egui::Response,
        volume: Option<&TileVolumeHover>,
        tilemap: Option<&TilemapHover>,
    ) {
        if let Some(hover) = volume {
            let requested = response.clicked_by(egui::PointerButton::Primary)
                || (self.tile_volume_tool.placement == TilePlacement::Level
                    && response.dragged_by(egui::PointerButton::Primary));
            if requested {
                self.apply_volume_brush(hover);
            }
        } else if let Some(hover) = tilemap
            && (response.clicked_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Primary))
        {
            self.apply_tile_brush(hover);
        }
    }

    /// Resolves the authored world through the project's Weave presentation.
    ///
    /// Kept outside `render_view` because resolving presentation is one concern
    /// of its own and because failures need the same console/render-error path
    /// whichever viewport asked for them.
    fn resolve_presentation(&mut self, editing: bool, rect: Rect) -> Option<sindri_core::World> {
        if self.styles.is_empty() {
            return None;
        }
        match self
            .styles
            .resolve(&self.world, self.presentation_viewport(editing, rect))
        {
            Ok(world) => Some(world),
            Err(error) => {
                let failure = format!("Weave: {error}");
                self.console.error(&failure);
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

    /// Everything the Scene view wears over the rendered frame.
    ///
    /// Chrome only: nothing here changes the scene or the camera, so it is
    /// drawn after the image and reads what the frame was drawn with rather
    /// than working any of it out a second time.
    fn paint_scene_chrome(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        camera: CameraView,
        hover: Option<PaintHover<'_>>,
        painting: bool,
        text_rect: Option<([f32; 2], [f32; 2])>,
    ) {
        self.paint_canvas_outline(ui, rect, camera);
        if !painting && let Some((centre, size)) = text_rect {
            self.paint_text_rect(ui, rect, camera, centre, size);
        }
        match hover {
            Some(PaintHover::Tilemap(hover)) => self.paint_tilemap_hover(ui, hover),
            Some(PaintHover::TileVolume(hover)) => self.paint_tile_volume_hover(ui, hover),
            None => {}
        }
        if !painting {
            paint_selection_marks(ui.painter(), &self.selection_marks(rect, camera));
            if let Some((_, _, visual)) = self.gizmo_visual(rect, camera) {
                paint_transform_gizmo(
                    ui.painter(),
                    rect,
                    &visual,
                    self.gizmo_drag.map(|drag| drag.axis),
                );
            }
        }
        // The same view the frame under it was drawn through, asked for rather
        // than re-derived, so the axes cannot drift from the picture.
        let axes = self
            .scene
            .world_camera(&self.world, camera)
            .ok()
            .flatten()
            .map(|camera| camera.view);
        // What a drag here would do, said where the pointer already is.
        let selection = match self.selection.len() {
            0 => "No selection".to_owned(),
            1 => self
                .selection
                .primary()
                .and_then(|entity| self.world.get(entity))
                .map_or_else(|| "No selection".to_owned(), entity_name),
            many => format!("{many} entities"),
        };
        paint_runtime_overlay(
            ui.painter(),
            rect,
            &ViewportStatus {
                selection: &selection,
                mode: self.gizmo_mode.label(),
                space: self.gizmo_space.label(),
                snapping: self.preferences.snapping.enabled,
                playing: self.lifecycle.state() == EngineState::Running,
            },
            self.problem(),
            axes,
        );
    }
}

impl EditorApp {
    /// Where this view puts the UI.
    ///
    /// The Scene view puts it in the world, where panning and zooming reach it.
    /// The Game view *is* the screen, so there the overlay is the viewport and
    /// no camera can move it — which is what makes a HUD a HUD.
    fn canvas_for(&self, editing: bool) -> UiCanvas {
        if editing {
            UiCanvas::InScene {
                aspect: self.canvas_aspect(),
            }
        } else {
            UiCanvas::OnViewport
        }
    }

    /// Draws the box the selected text element is laid out in.
    ///
    /// The rect the words actually occupy, which is the authored bounds where
    /// there are any and what the string came out as where there are not. It is
    /// the one way to see what a wrap width does without retyping it and
    /// looking.
    fn paint_text_rect(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        camera: CameraView,
        centre: [f32; 2],
        size: [f32; 2],
    ) {
        let aspect = rect.width() / rect.height().max(1.0);
        let Ok(Some(world)) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
        else {
            return;
        };
        let painter = ui.painter();
        let stroke = egui::Stroke::new(1.0, crate::ui::theme::color::FORGE_DIM);
        for [start, end] in
            super::camera::canvas_rect_outline(rect, world.view_projection, centre, size)
        {
            painter.line_segment([start, end], stroke);
        }
    }

    /// Draws the edge of the screen the UI is laid out on.
    ///
    /// Only in the Scene view, and only because the canvas is in the scene
    /// there: in the Game view the canvas *is* the viewport and its edge is the
    /// viewport's border, which is already drawn.
    fn paint_canvas_outline(&self, ui: &egui::Ui, rect: Rect, camera: CameraView) {
        let aspect = rect.width() / rect.height().max(1.0);
        let Ok(Some(world)) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
        else {
            return;
        };
        let painter = ui.painter();
        let stroke = egui::Stroke::new(1.0, crate::ui::theme::color::LINE);
        for [start, end] in
            super::camera::canvas_outline(rect, world.view_projection, self.canvas_aspect())
        {
            painter.line_segment([start, end], stroke);
        }
    }
}
