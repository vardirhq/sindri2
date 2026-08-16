use std::{collections::BTreeMap, f32::consts::TAU, sync::Arc};

use eframe::{
    egui::{
        self, Align, Color32, FontData, FontFamily, FontId, Layout, Pos2, Rect, Response, RichText,
        Sense, Stroke, StrokeKind, TextStyle, Vec2,
    },
    wgpu,
};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_3D_ROTATION, ICON_ACCOUNT_TREE, ICON_ADD, ICON_ARROW_SELECTOR_TOOL, ICON_CAMERA_ALT,
        ICON_CODE, ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_EXPAND_MORE, ICON_FILTER_LIST,
        ICON_FOLDER, ICON_GRID_VIEW, ICON_IMAGE, ICON_LIGHT_MODE, ICON_MORE_VERT, ICON_OPEN_WITH,
        ICON_PAUSE, ICON_PLAY_ARROW, ICON_REDO, ICON_SEARCH, ICON_SETTINGS, ICON_STOP, ICON_TUNE,
        ICON_UNDO, ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};
use glam::Vec2 as GlamVec2;
use serde_json::Value;
use sindri_core::{
    CommandBuffer, CommandHistory, EngineLifecycle, EngineState, EntityData, EntityId,
    SceneDocument, Transform2D, Transform3D, World, WorldCommand,
};
use sindri_cube::{
    CameraView, DemoScene, FrameRenderers, FrameTarget, TextureBindings, WorldProjection,
    demo_textures, encode_prepared_frame,
};
use sindri_render::{
    COLOR_TARGET_FORMAT, DepthTarget, SpriteBatchRenderer, TextureRegistry, TexturedCubeRenderer,
    Viewport,
};

const INTER_FONT: &[u8] = include_bytes!("../assets/Inter.ttf");
const ACCENT: Color32 = Color32::from_rgb(246, 169, 35);
const ACCENT_BRIGHT: Color32 = Color32::from_rgb(255, 187, 54);
const ACCENT_SOFT: Color32 = Color32::from_rgb(59, 45, 20);
const APP_BG: Color32 = Color32::from_rgb(9, 12, 16);
const TOP_BG: Color32 = Color32::from_rgb(12, 15, 19);
const PANEL_BG: Color32 = Color32::from_rgb(15, 19, 23);
const PANEL_RAISED: Color32 = Color32::from_rgb(19, 24, 29);
const FIELD_BG: Color32 = Color32::from_rgb(12, 16, 20);
const BORDER: Color32 = Color32::from_rgb(39, 46, 53);
const BORDER_SUBTLE: Color32 = Color32::from_rgb(29, 35, 41);
const TEXT: Color32 = Color32::from_rgb(224, 228, 231);
const TEXT_MUTED: Color32 = Color32::from_rgb(143, 151, 159);
const TEXT_FAINT: Color32 = Color32::from_rgb(92, 101, 110);
const SUCCESS: Color32 = Color32::from_rgb(98, 202, 122);
const INITIAL_VIEWPORT_WIDTH: u32 = 960;
const INITIAL_VIEWPORT_HEIGHT: u32 = 540;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sindri Editor")
            .with_inner_size([1_440.0, 1_024.0])
            .with_min_inner_size([1_100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sindri Editor",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)))),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorMode {
    Select,
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTab {
    Scene,
    Game,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BottomTab {
    Project,
    Console,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraProjection {
    Perspective,
    Orthographic,
}

/// How the editor is looking at the scene, as opposed to what the scene says.
///
/// The authored camera lives in the world; this moves around it without
/// touching a single entity.
#[derive(Clone, Copy)]
struct EditorCamera {
    orbit: GlamVec2,
    zoom: f32,
    projection: CameraProjection,
}

struct RuntimeViewport {
    render_state: eframe::egui_wgpu::RenderState,
    texture: wgpu::Texture,
    views: ViewportViews,
    depth: DepthTarget,
    texture_id: egui::TextureId,
    cube_renderer: TexturedCubeRenderer,
    sprite_renderer: SpriteBatchRenderer,
    textures: TextureRegistry,
    bindings: TextureBindings,
    width: u32,
    height: u32,
}

impl RuntimeViewport {
    /// The shared colour target format. Defining one here is how the editor
    /// previously drifted into a linear target and rendered every colour too
    /// dark, so it defers to `sindri-render` instead.
    const FORMAT: wgpu::TextureFormat = COLOR_TARGET_FORMAT;

    fn new(context: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let render_state = context
            .wgpu_render_state
            .clone()
            .ok_or_else(|| "the editor requires eframe's WGPU renderer".to_owned())?;
        let (texture, views) = create_viewport_texture(
            &render_state.device,
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
        );
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &views.sampled,
            wgpu::FilterMode::Linear,
        );
        let depth = DepthTarget::new(
            &render_state.device,
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
        );
        let cube_renderer = TexturedCubeRenderer::new(&render_state.device, Self::FORMAT);
        let sprite_renderer = SpriteBatchRenderer::new(&render_state.device, Self::FORMAT);
        let (textures, bindings) = demo_textures(&render_state.device, &render_state.queue);
        Ok(Self {
            render_state,
            texture,
            views,
            depth,
            texture_id,
            cube_renderer,
            sprite_renderer,
            textures,
            bindings,
            width: INITIAL_VIEWPORT_WIDTH,
            height: INITIAL_VIEWPORT_HEIGHT,
        })
    }

    fn render(
        &mut self,
        scene: &DemoScene,
        world: &World,
        size: (u32, u32),
        camera: EditorCamera,
    ) -> Result<(), String> {
        self.resize(size.0, size.1);
        let prepared = scene
            .extract(
                world,
                Viewport::new(self.width, self.height),
                CameraView {
                    orbit: camera.orbit,
                    distance_scale: 1.0 / camera.zoom,
                    projection: match camera.projection {
                        CameraProjection::Perspective => WorldProjection::Perspective,
                        CameraProjection::Orthographic => WorldProjection::Orthographic,
                    },
                },
                &self.bindings,
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
                cube: &mut self.cube_renderer,
                sprites: &mut self.sprite_renderer,
                textures: &self.textures,
            },
            &self.render_state.device,
            &self.render_state.queue,
            &mut encoder,
            FrameTarget {
                color: &self.views.target,
                depth: &self.depth,
            },
            &prepared,
        )
        .map_err(|error| error.to_string())?;
        self.render_state.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        let (texture, views) = create_viewport_texture(&self.render_state.device, width, height);
        self.render_state
            .renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                &self.render_state.device,
                &views.sampled,
                wgpu::FilterMode::Linear,
                self.texture_id,
            );
        self.texture = texture;
        self.views = views;
        self.depth.resize(&self.render_state.device, width, height);
        self.width = width;
        self.height = height;
    }
}

/// The two views of the viewport texture, which must not be the same view.
///
/// Rendering and sampling disagree about what the stored bytes mean, so each
/// gets the view that makes its own half right. See [`create_viewport_texture`].
struct ViewportViews {
    /// The sRGB view the scene renders into, so the hardware encodes on write.
    target: wgpu::TextureView,
    /// The linear view egui samples, so the hardware does not decode on read.
    sampled: wgpu::TextureView,
}

/// Builds the viewport texture and both views of it.
///
/// The texture is sRGB because [`sindri_render::COLOR_TARGET_FORMAT`] is what
/// every Sindri colour target uses: shaders work in linear and the target
/// encodes on write.
///
/// egui then samples that texture, and its shader says what it expects:
///
/// ```wgsl
/// // We expect "normal" textures that are NOT sRGB-aware.
/// let tex_gamma = sample_texture(in);
/// ```
///
/// It treats whatever it samples as already gamma-encoded and decodes it before
/// writing to an sRGB surface. Handing it an sRGB view means the hardware
/// decodes on read as well, and two decodes against one encode is a frame that
/// renders perfectly while being the wrong colour — authored orange arrives as
/// `(221, 43, 6)` instead of `(240, 114, 43)`.
///
/// So the sampled view is the linear view of the same bytes. Nothing is
/// converted twice, and neither half has to know what the other assumed.
fn create_viewport_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, ViewportViews) {
    let sampled_format = RuntimeViewport::FORMAT.remove_srgb_suffix();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Sindri editor runtime viewport"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RuntimeViewport::FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[RuntimeViewport::FORMAT, sampled_format],
    });
    let target = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Sindri editor viewport target"),
        format: Some(RuntimeViewport::FORMAT),
        ..Default::default()
    });
    let sampled = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Sindri editor viewport sampled by egui"),
        format: Some(sampled_format),
        ..Default::default()
    });
    (texture, ViewportViews { target, sampled })
}

struct EditorApp {
    scene: DemoScene,
    world: World,
    authored: SceneDocument,
    selection: Option<EntityId>,
    history: CommandHistory,
    search: String,
    asset_search: String,
    mode: EditorMode,
    workspace_tab: WorkspaceTab,
    bottom_tab: BottomTab,
    projection: CameraProjection,
    lifecycle: EngineLifecycle,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
    runtime_viewport: RuntimeViewport,
    runtime_error: Option<String>,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&context.egui_ctx);
        let scene = DemoScene::new().expect("the built-in component schemas register");
        let authored = DemoScene::authored_document()
            .expect("the embedded editor fixture must remain a valid scene");
        let world = scene
            .load_world(&authored)
            .expect("the embedded editor fixture must satisfy the demo component schema");
        let selection = find_by_source_id(&world, "checker-cube");
        let runtime_viewport = RuntimeViewport::new(context)
            .expect("the native editor must initialize its shared WGPU runtime viewport");
        Self {
            scene,
            world,
            authored,
            selection,
            history: CommandHistory::default(),
            search: String::new(),
            asset_search: String::new(),
            mode: EditorMode::Select,
            workspace_tab: WorkspaceTab::Scene,
            bottom_tab: BottomTab::Project,
            projection: CameraProjection::Perspective,
            lifecycle: initialized_lifecycle(),
            viewport_yaw: 0.0,
            viewport_pitch: 0.0,
            viewport_zoom: 1.0,
            runtime_viewport,
            runtime_error: None,
        }
    }

    /// Changes the selection, ending any in-progress merge run so the next
    /// edit starts its own undo step.
    fn select(&mut self, entity: Option<EntityId>) {
        if self.selection != entity {
            self.history.break_merge_run();
            self.selection = entity;
        }
    }

    /// Turns the difference between the drawn draft and the world into one
    /// transaction, so inspector edits are undoable and reach the viewport.
    fn commit_draft(&mut self, entity: EntityId, original: &EntityDraft, draft: &EntityDraft) {
        let buffer = draft_commands(entity, original, draft);
        if buffer.is_empty() {
            return;
        }

        // One merge key per entity: a continuous drag stays a single undo step
        // until the pointer is released or the selection changes.
        let transaction = buffer
            .into_transaction("Edit entity")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.runtime_error = Some(error.to_string());
        }
    }

    fn undo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.undo(&mut self.world) {
            self.runtime_error = Some(error.to_string());
        }
    }

    fn redo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.redo(&mut self.world) {
            self.runtime_error = Some(error.to_string());
        }
    }

    /// Rebuilds the runtime scene from the authored document.
    ///
    /// Every runtime handle is replaced, so recorded history is discarded
    /// rather than left pointing at entities that no longer exist.
    fn reset_to_authored(&mut self) {
        match self.scene.load_world(&self.authored) {
            Ok(world) => {
                self.world = world;
                self.history.clear();
                self.selection = find_by_source_id(&self.world, "checker-cube");
                self.lifecycle = initialized_lifecycle();
                self.runtime_error = None;
            }
            Err(error) => self.runtime_error = Some(error.to_string()),
        }
    }

    /// Play and pause move the engine lifecycle rather than a display flag, so
    /// the editor exercises the same transitions a runtime host does.
    fn toggle_playback(&mut self) {
        let result = match self.lifecycle.state() {
            EngineState::Running => self.lifecycle.pause(),
            EngineState::Paused => self.lifecycle.resume(),
            _ => self.lifecycle.start(),
        };
        if let Err(error) = result {
            self.runtime_error = Some(error.to_string());
        }
    }

    fn pause(&mut self) {
        if self.lifecycle.state() == EngineState::Running
            && let Err(error) = self.lifecycle.pause()
        {
            self.runtime_error = Some(error.to_string());
        }
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        let (undo, redo) = context.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                input.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y),
            )
        });
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 11.0;
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("SINDRI")
                            .strong()
                            .size(15.0)
                            .color(ACCENT_BRIGHT),
                    );
                    ui.add_space(8.0);
                    for menu in ["File", "Edit", "Scene", "View", "Build", "Tools", "Help"] {
                        ui.add(
                            egui::Button::new(RichText::new(menu).size(12.0).color(TEXT_MUTED))
                                .frame(false),
                        );
                    }
                    ui.add_space((ui.available_width() * 0.22).max(16.0));
                    let undo_tip = self.history.undo_label().map_or_else(
                        || "Nothing to undo".to_owned(),
                        |label| format!("Undo {label}"),
                    );
                    if transport_icon(ui, ICON_UNDO, false, self.history.can_undo(), &undo_tip)
                        .clicked()
                    {
                        self.undo();
                    }
                    let redo_tip = self.history.redo_label().map_or_else(
                        || "Nothing to redo".to_owned(),
                        |label| format!("Redo {label}"),
                    );
                    if transport_icon(ui, ICON_REDO, false, self.history.can_redo(), &redo_tip)
                        .clicked()
                    {
                        self.redo();
                    }
                    let running = self.lifecycle.state() == EngineState::Running;
                    if transport_icon(
                        ui,
                        ICON_STOP,
                        false,
                        true,
                        "Stop and reset to the authored scene",
                    )
                    .clicked()
                    {
                        self.reset_to_authored();
                    }
                    if transport_icon(ui, ICON_PAUSE, !running, running, "Pause").clicked() {
                        self.pause();
                    }
                    if transport_icon(ui, ICON_PLAY_ARROW, running, true, "Play").clicked()
                        || play_button(ui, running).clicked()
                    {
                        self.toggle_playback();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            ICON_EXPAND_MORE
                                .outlined()
                                .rich_text()
                                .size(16.0)
                                .color(TEXT_FAINT),
                        );
                        ui.label(RichText::new("isogame").size(12.0).color(TEXT_MUTED));
                    });
                });
            });
    }

    fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("scene-hierarchy")
            .default_size(248.0)
            .min_size(210.0)
            .max_size(340.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Hierarchy", Some(ICON_ADD));
                search_field(ui, &mut self.search, "Search");
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        hierarchy_group(ui, "World", ICON_ACCOUNT_TREE);
                        let needle = self.search.trim().to_lowercase();
                        let mut clicked = None;
                        for (entity, depth) in hierarchy_rows(&self.world) {
                            let Some(data) = self.world.get(entity) else {
                                continue;
                            };
                            let name = entity_name(data);
                            if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                                continue;
                            }
                            if hierarchy_row(
                                ui,
                                entity_icon(data),
                                &name,
                                self.selection == Some(entity),
                                depth + 1,
                            )
                            .clicked()
                            {
                                clicked = Some(entity);
                            }
                        }
                        if let Some(entity) = clicked {
                            self.select(Some(entity));
                        }
                    });
            });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(340.0)
            .min_size(300.0)
            .max_size(440.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Inspector", None);
                let Some(entity) = self.selection else {
                    return;
                };
                let Some(data) = self.world.get(entity) else {
                    return;
                };
                // Widgets edit a draft copy; every difference becomes a command,
                // so the world is only ever written through the command layer.
                let mut draft = EntityDraft::from(data);
                let original = draft.clone();
                let icon = entity_icon(data);
                let components = data.components.clone();
                {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, icon, &mut draft);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        if let Some(transform) = &mut draft.transform_2d {
                            transform_2d_section(ui, transform);
                        }
                        components_sections(ui, &components);
                        ui.add_space(10.0);
                        ui.add_sized(
                            [ui.available_width(), 31.0],
                            egui::Button::new(
                                RichText::new("Add Component").size(12.0).color(TEXT),
                            )
                            .fill(PANEL_RAISED)
                            .stroke(Stroke::new(1.0, BORDER)),
                        );
                    });
                }
                self.commit_draft(entity, &original, &draft);
            });
    }

    fn asset_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("asset-browser")
            .default_size(226.0)
            .min_size(140.0)
            .max_size(330.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    bottom_tab(ui, &mut self.bottom_tab, BottomTab::Project, "Project");
                    bottom_tab(ui, &mut self.bottom_tab, BottomTab::Console, "Console");
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Project => project_browser(ui, &mut self.asset_search),
                    BottomTab::Console => {
                        console_view(ui, self.world.len(), self.lifecycle.state());
                    }
                }
            });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("editor-status")
            .exact_size(26.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    let healthy = self.runtime_error.is_none();
                    status_dot(ui, if healthy { SUCCESS } else { ACCENT_BRIGHT });
                    ui.label(
                        RichText::new(if healthy {
                            "Renderer ready"
                        } else {
                            "Renderer reported an error"
                        })
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            ICON_SETTINGS
                                .outlined()
                                .rich_text()
                                .size(15.0)
                                .color(TEXT_FAINT),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(if self.runtime_error.is_some() {
                                "1 Error, 0 Warnings"
                            } else {
                                "0 Errors, 0 Warnings"
                            })
                            .size(11.0)
                            .color(TEXT_FAINT),
                        );
                    });
                });
            });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Scene, "Scene");
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Game, "Game");
                });
                ui.separator();
                if self.workspace_tab == WorkspaceTab::Game {
                    empty_game_view(ui);
                    return;
                }
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    mode_icon(
                        ui,
                        &mut self.mode,
                        EditorMode::Select,
                        ICON_ARROW_SELECTOR_TOOL,
                        "Select",
                    );
                    mode_icon(ui, &mut self.mode, EditorMode::Move, ICON_OPEN_WITH, "Move");
                    mode_icon(
                        ui,
                        &mut self.mode,
                        EditorMode::Rotate,
                        ICON_3D_ROTATION,
                        "Rotate",
                    );
                    mode_icon(ui, &mut self.mode, EditorMode::Scale, ICON_TUNE, "Scale");
                    ui.separator();
                    icon_button(ui, ICON_VIEW_IN_AR, false, "Local coordinates");
                    icon_button(ui, ICON_LIGHT_MODE, false, "Lit shading");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(8.0);
                        projection_button(
                            ui,
                            &mut self.projection,
                            CameraProjection::Orthographic,
                            "Ortho",
                        );
                        projection_button(
                            ui,
                            &mut self.projection,
                            CameraProjection::Perspective,
                            "Perspective",
                        );
                    });
                });
                ui.separator();
                let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
                if response.dragged() {
                    let delta = response.drag_motion();
                    self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                    self.viewport_pitch = (self.viewport_pitch + delta.y * 0.008).clamp(-1.1, 1.1);
                }
                if response.hovered() {
                    let delta = context.input(|input| input.smooth_scroll_delta.y);
                    self.viewport_zoom = (self.viewport_zoom + delta * 0.002).clamp(0.65, 1.8);
                }
                let scale = context.pixels_per_point();
                self.runtime_error = self
                    .runtime_viewport
                    .render(
                        &self.scene,
                        &self.world,
                        (
                            physical_viewport_dimension(rect.width(), scale),
                            physical_viewport_dimension(rect.height(), scale),
                        ),
                        EditorCamera {
                            orbit: GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                            zoom: self.viewport_zoom,
                            projection: self.projection,
                        },
                    )
                    .err();
                ui.painter().image(
                    self.runtime_viewport.texture_id,
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                paint_runtime_overlay(
                    ui.painter(),
                    rect,
                    &self
                        .selection
                        .and_then(|entity| self.world.get(entity))
                        .map_or_else(|| "No selection".to_owned(), entity_name),
                    self.runtime_error.as_deref(),
                );
                context.request_repaint();
            });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.top_bar(ui);
        self.status_bar(ui);
        self.hierarchy_panel(ui);
        self.inspector_panel(ui);
        self.asset_panel(ui);
        self.viewport(ui);
        // Releasing the pointer ends a drag, so the next one is its own step.
        if ui.ctx().input(|input| input.pointer.any_released()) {
            self.history.break_merge_run();
        }
    }
}

fn configure_theme(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(INTER_FONT)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    context.set_fonts(fonts);
    egui_material_icons::initialize(context);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(7.0, 6.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.interact_size.y = 26.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.visuals.panel_fill = PANEL_BG;
        style.visuals.window_fill = PANEL_RAISED;
        style.visuals.extreme_bg_color = FIELD_BG;
        style.visuals.faint_bg_color = PANEL_RAISED;
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(25, 31, 37);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(55, 65, 74));
        style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

fn panel_title(ui: &mut egui::Ui, title: &str, action: Option<MaterialIcon>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
        if let Some(action) = action {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(6.0);
                icon_button(ui, action, false, "Add entity");
            });
        }
    });
    ui.add_space(3.0);
    ui.separator();
}

fn search_field(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            ICON_SEARCH
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.add_sized(
            [ui.available_width() - 10.0, 28.0],
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .frame(egui::Frame::NONE),
        );
    });
}

fn hierarchy_group(ui: &mut egui::Ui, label: &str, icon: MaterialIcon) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            ICON_EXPAND_MORE
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.label(icon.outlined().rich_text().size(15.0).color(TEXT_MUTED));
        ui.label(RichText::new(label).size(12.0).color(TEXT));
    });
}

fn hierarchy_row(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    selected: bool,
    depth: usize,
) -> Response {
    ui.horizontal(|ui| {
        ui.add_space(9.0 + hierarchy_indent(depth, 14.0));
        ui.label(icon.outlined().rich_text().size(15.0).color(if selected {
            ACCENT_BRIGHT
        } else {
            TEXT_MUTED
        }));
        ui.add(
            egui::Button::new(RichText::new(name).size(12.0).color(if selected {
                TEXT
            } else {
                TEXT_MUTED
            }))
            .selected(selected)
            .frame(false),
        )
    })
    .response
}

fn inspector_identity(ui: &mut egui::Ui, icon: MaterialIcon, draft: &mut EntityDraft) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(icon.outlined().rich_text().size(19.0).color(TEXT_MUTED));
        ui.add_sized(
            [ui.available_width() - 18.0, 29.0],
            egui::TextEdit::singleline(&mut draft.name).font(FontId::proportional(13.0)),
        );
    });
    ui.horizontal(|ui| {
        ui.add_space(27.0);
        ui.label(RichText::new("Tag  Untagged").size(11.0).color(TEXT_FAINT));
        ui.separator();
        ui.label(RichText::new("Layer  Default").size(11.0).color(TEXT_FAINT));
    });
}

fn section_header(ui: &mut egui::Ui, icon: MaterialIcon, title: &str) {
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            ICON_EXPAND_MORE
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(5.0);
            ui.label(
                ICON_MORE_VERT
                    .outlined()
                    .rich_text()
                    .size(15.0)
                    .color(TEXT_FAINT),
            );
        });
    });
}

fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    section_header(ui, ICON_OPEN_WITH, "Transform");
    vector_row(ui, "Position", &mut transform.position);
    vector_row(ui, "Scale", &mut transform.scale);
    property_label(ui, "Rotation", "Quaternion");
}

fn transform_2d_section(ui: &mut egui::Ui, transform: &mut Transform2D) {
    section_header(ui, ICON_OPEN_WITH, "Transform 2D");
    vector_row_2d(ui, "Position", &mut transform.position);
    vector_row_2d(ui, "Scale", &mut transform.scale);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Rotation").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::DragValue::new(&mut transform.rotation_radians).speed(0.01));
        });
    });
}

fn components_sections(ui: &mut egui::Ui, components: &BTreeMap<String, Value>) {
    for name in components.keys() {
        let icon = match name.as_str() {
            "sindri.camera" => ICON_CAMERA_ALT,
            "sindri.sprite" => ICON_IMAGE,
            "sindri.mesh" => ICON_VIEW_IN_AR,
            _ => ICON_DEPLOYED_CODE,
        };
        section_header(ui, icon, &component_label(name));
        match name.as_str() {
            "sindri.camera" => {
                property_label(ui, "Projection", "Perspective");
                property_label(ui, "Field of view", "45°");
                property_label(ui, "Clipping", "0.1 - 100");
            }
            "sindri.sprite" => {
                property_label(ui, "Texture", "procedural:badge");
                property_label(ui, "Layer", "Overlay");
            }
            "sindri.mesh" => {
                property_label(ui, "Mesh", "Cube");
                property_label(ui, "Material", "Checkerboard");
                property_label(ui, "Layer", "World");
            }
            _ => property_label(ui, "Schema", "v1"),
        }
    }
}

fn property_label(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            ui.label(RichText::new(value).size(11.0).color(TEXT));
        });
    });
}

fn vector_row(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3]) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            ui.label(
                RichText::new(["X", "Y", "Z"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_sized(
                [48.0, 23.0],
                egui::DragValue::new(value).speed(0.05).max_decimals(3),
            );
        }
    });
}

fn vector_row_2d(ui: &mut egui::Ui, label: &str, values: &mut [f32; 2]) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            ui.label(
                RichText::new(["X", "Y"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_sized(
                [67.0, 23.0],
                egui::DragValue::new(value).speed(0.05).max_decimals(3),
            );
        }
    });
}

fn project_browser(ui: &mut egui::Ui, search: &mut String) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(174.0);
            for (label, selected, depth) in [
                ("Assets", true, 0),
                ("Materials", false, 1),
                ("Models", false, 1),
                ("Scenes", false, 1),
                ("Scripts", false, 1),
                ("Textures", false, 1),
            ] {
                folder_row(ui, label, selected, depth);
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Assets").size(12.0).color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    icon_button(ui, ICON_VIEW_LIST, false, "List view");
                    icon_button(ui, ICON_GRID_VIEW, true, "Grid view");
                    icon_button(ui, ICON_FILTER_LIST, false, "Filter assets");
                    ui.add_sized(
                        [210.0, 27.0],
                        egui::TextEdit::singleline(search).hint_text("Search Assets"),
                    );
                });
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (icon, label) in [
                    (ICON_FOLDER, "Materials"),
                    (ICON_FOLDER, "Models"),
                    (ICON_FOLDER, "Scenes"),
                    (ICON_FOLDER, "Scripts"),
                    (ICON_DESCRIPTION, "demo.scene"),
                    (ICON_VIEW_IN_AR, "checker_cube"),
                    (ICON_IMAGE, "badge"),
                    (ICON_CODE, "scene.rs"),
                ] {
                    asset_tile(ui, icon, label);
                }
            });
        });
    });
}

fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + hierarchy_indent(depth, 12.0));
        ui.label(
            ICON_FOLDER
                .outlined()
                .rich_text()
                .size(15.0)
                .color(if selected { ACCENT } else { TEXT_FAINT }),
        );
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(if selected { TEXT } else { TEXT_MUTED }),
        );
    });
}

fn asset_tile(ui: &mut egui::Ui, icon: MaterialIcon, label: &str) {
    ui.vertical(|ui| {
        ui.add_sized(
            [62.0, 54.0],
            egui::Button::new(icon.outlined().rich_text().size(27.0).color(
                if label == "demo.scene" {
                    ACCENT
                } else {
                    TEXT_MUTED
                },
            ))
            .fill(PANEL_RAISED)
            .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
        );
        ui.add_sized(
            [62.0, 17.0],
            egui::Label::new(RichText::new(label).size(10.0).color(TEXT_MUTED)).truncate(),
        );
    });
}

fn console_view(ui: &mut egui::Ui, entity_count: usize, state: EngineState) {
    ui.add_space(8.0);
    for (color, text) in [
        (SUCCESS, "Renderer initialized".to_owned()),
        (ACCENT, format!("Scene loaded - {entity_count} entities")),
        (TEXT_MUTED, format!("Engine {}", lifecycle_label(state))),
    ] {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            status_dot(ui, color);
            ui.label(RichText::new(&text).size(11.0).color(TEXT_MUTED));
        });
    }
}

fn lifecycle_label(state: EngineState) -> &'static str {
    match state {
        EngineState::Created => "created",
        EngineState::Initialized => "ready",
        EngineState::Running => "running",
        EngineState::Paused => "paused",
        EngineState::Stopped => "stopped",
        EngineState::Destroyed => "destroyed",
    }
}

fn workspace_tab(ui: &mut egui::Ui, current: &mut WorkspaceTab, value: WorkspaceTab, label: &str) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

fn bottom_tab(ui: &mut egui::Ui, current: &mut BottomTab, value: BottomTab, label: &str) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

fn projection_button(
    ui: &mut egui::Ui,
    current: &mut CameraProjection,
    value: CameraProjection,
    label: &str,
) {
    let selected = *current == value;
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(11.0).color(if selected {
                ACCENT_BRIGHT
            } else {
                TEXT_FAINT
            }))
            .selected(selected)
            .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
            .stroke(Stroke::new(1.0, if selected { ACCENT } else { BORDER })),
        )
        .clicked()
    {
        *current = value;
    }
}

fn mode_icon(
    ui: &mut egui::Ui,
    mode: &mut EditorMode,
    value: EditorMode,
    icon: MaterialIcon,
    tip: &str,
) {
    if icon_button(ui, icon, *mode == value, tip).clicked() {
        *mode = value;
    }
}

fn icon_button(ui: &mut egui::Ui, icon: MaterialIcon, selected: bool, tip: &str) -> Response {
    ui.add_sized(
        [28.0, 28.0],
        egui::Button::new(icon.outlined().rich_text().size(17.0).color(if selected {
            ACCENT_BRIGHT
        } else {
            TEXT_MUTED
        }))
        .selected(selected)
        .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
        .stroke(Stroke::new(
            1.0,
            if selected { ACCENT } else { BORDER_SUBTLE },
        )),
    )
    .on_hover_text(tip)
}

/// Draws a small status dot.
///
/// The bundled Inter subset carries 192 glyphs and has no `U+25CF`, so a text
/// bullet renders as a missing-glyph box rather than a dot. Painting it keeps
/// the indicator independent of font coverage.
fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (response, painter) = ui.allocate_painter(Vec2::splat(9.0), Sense::hover());
    painter.circle_filled(response.rect.center(), 3.0, color);
}

fn transport_icon(
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

fn initialized_lifecycle() -> EngineLifecycle {
    let mut lifecycle = EngineLifecycle::new();
    lifecycle
        .initialize()
        .expect("a new lifecycle always accepts initialization");
    lifecycle
}

fn play_button(ui: &mut egui::Ui, playing: bool) -> Response {
    let text = if playing { "Stop" } else { "Play" };
    ui.add_sized(
        [68.0, 29.0],
        egui::Button::new(
            RichText::new(text)
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(28, 23, 12)),
        )
        .fill(ACCENT_BRIGHT)
        .stroke(Stroke::new(1.0, ACCENT)),
    )
}

fn empty_game_view(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                ICON_PLAY_ARROW
                    .outlined()
                    .rich_text()
                    .size(34.0)
                    .color(TEXT_FAINT),
            );
            ui.label(
                RichText::new("Game preview")
                    .strong()
                    .size(15.0)
                    .color(TEXT_MUTED),
            );
            ui.label(
                RichText::new("Enter Play mode to preview the active camera")
                    .size(12.0)
                    .color(TEXT_FAINT),
            );
        });
    });
}

fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    selected_name: &str,
    error: Option<&str>,
) {
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let label_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(218.0, 42.0));
    painter.rect_filled(label_rect, 3.0, Color32::from_black_alpha(165));
    painter.text(
        label_rect.min + Vec2::new(9.0, 7.0),
        egui::Align2::LEFT_TOP,
        selected_name,
        FontId::proportional(12.0),
        TEXT,
    );
    painter.text(
        label_rect.min + Vec2::new(9.0, 24.0),
        egui::Align2::LEFT_TOP,
        "Live WGPU viewport  ·  Drag to orbit",
        FontId::proportional(10.0),
        TEXT_FAINT,
    );
    if let Some(error) = error {
        let error_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 42.0),
            Vec2::new((rect.width() - 24.0).max(1.0), 30.0),
        );
        painter.rect_filled(error_rect, 3.0, Color32::from_rgb(72, 28, 32));
        painter.text(
            error_rect.left_center() + Vec2::new(9.0, 0.0),
            egui::Align2::LEFT_CENTER,
            error,
            FontId::proportional(10.0),
            Color32::from_rgb(255, 184, 191),
        );
    }
    paint_axis_gizmo(painter, Pos2::new(rect.right() - 42.0, rect.top() + 48.0));
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2) {
    for (offset, color, label) in [
        (Vec2::new(19.0, 7.0), Color32::from_rgb(239, 92, 101), "X"),
        (Vec2::new(0.0, -22.0), Color32::from_rgb(89, 201, 135), "Y"),
        (Vec2::new(-14.0, 11.0), Color32::from_rgb(91, 151, 239), "Z"),
    ] {
        let end = origin + offset;
        painter.line_segment([origin, end], Stroke::new(2.0, color));
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.0),
            color,
        );
    }
}

/// The inspector's editable copy of an entity.
///
/// Widgets write here rather than into the world, so every change can be
/// turned into a command instead of a silent mutation.
#[derive(Clone, Debug, PartialEq)]
struct EntityDraft {
    name: String,
    transform_2d: Option<Transform2D>,
    transform_3d: Option<Transform3D>,
}

impl From<&EntityData> for EntityDraft {
    fn from(data: &EntityData) -> Self {
        Self {
            name: entity_name(data),
            transform_2d: data.transform_2d,
            transform_3d: data.transform_3d,
        }
    }
}

/// Turns the difference between an entity's stored state and the drawn draft
/// into the commands that close the gap.
fn draft_commands(entity: EntityId, original: &EntityDraft, draft: &EntityDraft) -> CommandBuffer {
    let mut buffer = CommandBuffer::new();
    if original.name != draft.name {
        buffer.push(WorldCommand::SetName {
            entity,
            name: Some(draft.name.clone()),
        });
    }
    if original.transform_2d != draft.transform_2d {
        buffer.push(WorldCommand::SetTransform2D {
            entity,
            transform: draft.transform_2d,
        });
    }
    if original.transform_3d != draft.transform_3d {
        buffer.push(WorldCommand::SetTransform3D {
            entity,
            transform: draft.transform_3d,
        });
    }
    buffer
}

/// Flattens the world into display rows, parents before their children.
///
/// Siblings are ordered by stable ID so the panel matches the order the scene
/// is saved in, rather than the order slots happen to be allocated.
fn hierarchy_rows(world: &World) -> Vec<(EntityId, usize)> {
    let mut roots: Vec<EntityId> = world
        .entities()
        .filter(|(_, data)| data.parent.is_none())
        .map(|(entity, _)| entity)
        .collect();
    roots.sort_by_key(|entity| hierarchy_sort_key(world, *entity));

    let mut rows = Vec::new();
    for root in roots {
        push_hierarchy_row(world, root, 0, &mut rows);
    }
    rows
}

fn push_hierarchy_row(
    world: &World,
    entity: EntityId,
    depth: usize,
    rows: &mut Vec<(EntityId, usize)>,
) {
    rows.push((entity, depth));
    let Some(data) = world.get(entity) else {
        return;
    };
    let mut children = data.children.clone();
    children.sort_by_key(|child| hierarchy_sort_key(world, *child));
    for child in children {
        push_hierarchy_row(world, child, depth + 1, rows);
    }
}

fn hierarchy_sort_key(world: &World, entity: EntityId) -> String {
    world
        .get(entity)
        .and_then(|data| data.source_id.as_ref())
        .map_or_else(
            || format!("~{:010}", entity.index()),
            |id| id.as_str().to_owned(),
        )
}

fn find_by_source_id(world: &World, source_id: &str) -> Option<EntityId> {
    world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == source_id)
        })
        .map(|(entity, _)| entity)
}

fn entity_name(entity: &EntityData) -> String {
    entity.name.clone().unwrap_or_else(|| {
        entity
            .source_id
            .as_ref()
            .map_or_else(|| "Entity".to_owned(), |id| humanize(id.as_str()))
    })
}

fn entity_icon(entity: &EntityData) -> MaterialIcon {
    if entity.components.contains_key("sindri.camera") {
        ICON_CAMERA_ALT
    } else if entity.components.contains_key("sindri.mesh") {
        ICON_VIEW_IN_AR
    } else if entity.components.contains_key("sindri.sprite") {
        ICON_IMAGE
    } else {
        ICON_DEPLOYED_CODE
    }
}

fn component_label(name: &str) -> String {
    humanize(name.strip_prefix("sindri.").unwrap_or(name))
}

fn humanize(value: &str) -> String {
    value
        .split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn hierarchy_indent(depth: usize, step: f32) -> f32 {
    f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * step
}

#[cfg(test)]
mod tests {
    use sindri_core::{CommandHistory, SceneEntity, SceneEntityId};

    use super::*;

    fn demo_scene() -> DemoScene {
        DemoScene::new().unwrap()
    }

    fn demo_world() -> World {
        demo_scene()
            .load_world(&DemoScene::authored_document().unwrap())
            .unwrap()
    }

    fn nested_scene() -> SceneDocument {
        let mut torso = SceneEntity::new(SceneEntityId::new("torso").unwrap());
        torso.parent = Some(SceneEntityId::new("root").unwrap());
        let mut arm = SceneEntity::new(SceneEntityId::new("arm").unwrap());
        arm.parent = Some(SceneEntityId::new("torso").unwrap());
        let mut leg = SceneEntity::new(SceneEntityId::new("leg").unwrap());
        leg.parent = Some(SceneEntityId::new("root").unwrap());

        SceneDocument {
            entities: vec![
                SceneEntity::new(SceneEntityId::new("root").unwrap()),
                torso,
                arm,
                leg,
            ],
            ..SceneDocument::default()
        }
    }

    #[test]
    fn the_embedded_scene_loads_into_a_runtime_world() {
        let world = demo_world();
        assert_eq!(world.len(), 8);
        assert!(find_by_source_id(&world, "checker-cube").is_some());
        assert!(find_by_source_id(&world, "not-an-entity").is_none());
    }

    #[test]
    fn hierarchy_rows_nest_children_under_their_parents() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let rows: Vec<(String, usize)> = hierarchy_rows(&world)
            .into_iter()
            .map(|(entity, depth)| (entity_name(world.get(entity).unwrap()), depth))
            .collect();

        assert_eq!(
            rows,
            vec![
                ("Root".to_owned(), 0),
                // Siblings follow stable-ID order, matching the saved file.
                ("Leg".to_owned(), 1),
                ("Torso".to_owned(), 1),
                ("Arm".to_owned(), 2),
            ]
        );
    }

    #[test]
    fn every_entity_appears_exactly_once_in_the_hierarchy() {
        let world = demo_world();
        let rows = hierarchy_rows(&world);
        assert_eq!(rows.len(), world.len());
        let mut entities: Vec<_> = rows.iter().map(|(entity, _)| *entity).collect();
        entities.sort_by_key(|entity| entity.index());
        entities.dedup();
        assert_eq!(entities.len(), world.len());
    }

    #[test]
    fn an_untouched_draft_produces_no_commands() {
        let world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let draft = EntityDraft::from(world.get(entity).unwrap());
        assert!(draft_commands(entity, &draft.clone(), &draft).is_empty());
    }

    #[test]
    fn inspector_edits_reach_the_world_and_undo_cleanly() {
        let mut world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original = EntityDraft::from(world.get(entity).unwrap());

        let mut draft = original.clone();
        draft.name = "Renamed Cube".to_owned();
        draft.transform_3d = Some(Transform3D {
            position: [1.0, 2.0, 3.0],
            ..draft.transform_3d.unwrap_or_default()
        });

        let buffer = draft_commands(entity, &original, &draft);
        assert_eq!(buffer.len(), 2);

        let mut history = CommandHistory::default();
        history
            .apply(buffer.into_transaction("Edit entity"), &mut world)
            .unwrap();
        let edited = world.get(entity).unwrap();
        assert_eq!(edited.name.as_deref(), Some("Renamed Cube"));
        assert_eq!(edited.transform_3d, draft.transform_3d);

        history.undo(&mut world).unwrap();
        assert_eq!(EntityDraft::from(world.get(entity).unwrap()), original);
    }

    #[test]
    fn a_drag_run_collapses_into_one_undo_step() {
        let mut world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original = EntityDraft::from(world.get(entity).unwrap());
        let mut history = CommandHistory::default();

        for step in [1.0_f32, 2.0, 3.0] {
            let mut draft = original.clone();
            draft.transform_3d = Some(Transform3D {
                position: [step, 0.0, 0.0],
                ..original.transform_3d.unwrap_or_default()
            });
            history
                .apply(
                    draft_commands(entity, &original, &draft)
                        .into_transaction("Edit entity")
                        .merging(format!("inspector:{}", entity.index())),
                    &mut world,
                )
                .unwrap();
        }

        history.undo(&mut world).unwrap();
        assert_eq!(EntityDraft::from(world.get(entity).unwrap()), original);
        assert!(!history.can_undo());
    }

    #[test]
    fn edits_survive_a_save_and_reload_of_the_real_scene() {
        let mut world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original = EntityDraft::from(world.get(entity).unwrap());
        let mut draft = original.clone();
        draft.transform_3d = Some(Transform3D {
            position: [0.0, 1.5, 0.0],
            ..original.transform_3d.unwrap_or_default()
        });

        CommandHistory::default()
            .apply(
                draft_commands(entity, &original, &draft).into_transaction("Move"),
                &mut world,
            )
            .unwrap();

        let saved = world.to_scene().unwrap().to_canonical_json().unwrap();
        let reopened = demo_scene()
            .load_world(&SceneDocument::from_json(&saved).unwrap())
            .unwrap();
        let reloaded = find_by_source_id(&reopened, "checker-cube").unwrap();
        assert_eq!(
            reopened.get(reloaded).unwrap().transform_3d,
            draft.transform_3d
        );
    }

    #[test]
    fn the_lifecycle_drives_play_pause_and_stop() {
        let mut lifecycle = initialized_lifecycle();
        assert_eq!(lifecycle_label(lifecycle.state()), "ready");
        lifecycle.start().unwrap();
        assert_eq!(lifecycle_label(lifecycle.state()), "running");
        lifecycle.pause().unwrap();
        assert_eq!(lifecycle_label(lifecycle.state()), "paused");
        lifecycle.resume().unwrap();
        lifecycle.stop().unwrap();
        assert_eq!(lifecycle_label(lifecycle.state()), "stopped");
    }

    #[test]
    fn labels_are_human_readable() {
        assert_eq!(humanize("checker-cube"), "Checker Cube");
        assert_eq!(component_label("sindri.sprite"), "Sprite");
    }
}
