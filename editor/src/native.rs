use std::{
    collections::BTreeMap,
    f32::consts::TAU,
    path::{Path, PathBuf},
    sync::Arc,
};

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
        ICON_FOLDER, ICON_GRID_VIEW, ICON_IMAGE, ICON_MORE_VERT, ICON_OPEN_WITH, ICON_PAUSE,
        ICON_PLAY_ARROW, ICON_REDO, ICON_SEARCH, ICON_SETTINGS, ICON_STOP, ICON_TUNE, ICON_UNDO,
        ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};
use glam::Vec2 as GlamVec2;
use serde_json::Value;
use sindri_core::{
    CommandBuffer, CommandHistory, EngineLifecycle, EngineState, EntityData, EntityId, Transform3D,
    World, WorldCommand,
};
use sindri_cube::{
    CameraView, DemoScene, FrameRenderers, FrameTarget, TextureBindings, WorldProjection,
    demo_textures, encode_prepared_frame,
};
use sindri_render::{
    SpriteBatchRenderer, TextureRegistry, TexturedCubeRenderer, Viewport, ViewportTarget,
};
use sindri_scene::{
    CameraComponent, MeshComponent, MeshPrimitive, SpriteAnchor, SpriteComponent, SpriteSpace,
};

use crate::{
    // `egui::Layout` is a different thing entirely and is already in scope.
    preferences::{AssetView, BottomTab, CameraProjection, Layout as WorkspaceLayout, Preferences},
    scene_file::{DEFAULT_SCENE_PATH, SceneFile},
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

/// How the editor is looking at the scene, as opposed to what the scene says.
///
/// The authored camera lives in the world; this moves around it without
/// touching a single entity.
#[derive(Clone, Copy)]
struct EditorCamera {
    orbit: GlamVec2,
    zoom: f32,
    pan: GlamVec2,
    projection: CameraProjection,
}

/// The camera a workspace tab looks through.
///
/// The scene view is where the editor moves around; the game view is what the
/// player would see, which means the authored camera and nothing else. If an
/// orbit or a pan leaked into it, it would stop answering the only question it
/// exists to answer.
fn camera_for(tab: WorkspaceTab, editor: EditorCamera) -> CameraView {
    match tab {
        WorkspaceTab::Scene => CameraView {
            orbit: editor.orbit,
            distance_scale: 1.0 / editor.zoom,
            pan: editor.pan,
            projection: match editor.projection {
                CameraProjection::Perspective => WorldProjection::Perspective,
                CameraProjection::Orthographic => WorldProjection::Orthographic,
            },
        },
        WorkspaceTab::Game => CameraView::default(),
    }
}

/// The GPU resources every viewport draws with.
///
/// Held once rather than per viewport: pipelines and textures do not depend on
/// which camera is looking, and two viewports that each built their own would
/// pay twice for the same thing.
struct SceneRenderers {
    cube: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    textures: TextureRegistry,
    bindings: TextureBindings,
}

impl SceneRenderers {
    fn new(render_state: &eframe::egui_wgpu::RenderState) -> Self {
        let (textures, bindings) = demo_textures(&render_state.device, &render_state.queue);
        Self {
            cube: TexturedCubeRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            sprites: SpriteBatchRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            textures,
            bindings,
        }
    }
}

/// One rendered view of the world, and the egui texture it is drawn through.
struct RuntimeViewport {
    render_state: eframe::egui_wgpu::RenderState,
    target: ViewportTarget,
    texture_id: egui::TextureId,
}

impl RuntimeViewport {
    fn new(render_state: eframe::egui_wgpu::RenderState, label: &str) -> Self {
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

    fn render(
        &mut self,
        renderers: &mut SceneRenderers,
        scene: &DemoScene,
        world: &World,
        size: (u32, u32),
        camera: CameraView,
    ) -> Result<(), String> {
        self.resize(size.0, size.1);
        let prepared = scene
            .extract(
                world,
                Viewport::new(self.target.width(), self.target.height()),
                camera,
                &renderers.bindings,
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
                textures: &renderers.textures,
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

struct EditorApp {
    scene: DemoScene,
    world: World,
    file: SceneFile,
    unsaved: bool,
    selection: Option<EntityId>,
    history: CommandHistory,
    search: String,
    asset_search: String,
    mode: EditorMode,
    workspace_tab: WorkspaceTab,
    preferences: Preferences,
    lifecycle: EngineLifecycle,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
    viewport_pan: GlamVec2,
    renderers: SceneRenderers,
    scene_viewport: RuntimeViewport,
    game_viewport: RuntimeViewport,
    /// What the last action the user took had to say, if anything went wrong.
    ///
    /// Kept apart from `render_error` because the two have different lifetimes:
    /// this one stays until something replaces it, while a render result is
    /// recomputed every frame. They shared a field, and the render overwrote a
    /// failed save within one frame of it happening.
    notice: Option<String>,
    /// Whatever the last frame's render reported.
    render_error: Option<String>,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&context.egui_ctx);
        let preferences = Preferences::load(context.storage);
        let scene = DemoScene::new().expect("the built-in component schemas register");
        let (file, open_error) = open_requested_scene();
        let world = scene
            .load_world(file.document())
            .expect("the opened scene must satisfy the demo component schema");
        // Nothing is selected until something is chosen. This used to name an
        // entity from the demo scene, which selected the cube in that one scene
        // and silently nothing in every other.
        let selection = None;
        let render_state = context
            .wgpu_render_state
            .clone()
            .expect("the native editor requires eframe's WGPU renderer");
        let renderers = SceneRenderers::new(&render_state);
        let scene_viewport = RuntimeViewport::new(render_state.clone(), "Sindri editor scene view");
        let game_viewport = RuntimeViewport::new(render_state, "Sindri editor game view");
        Self {
            scene,
            world,
            file,
            unsaved: false,
            selection,
            history: CommandHistory::default(),
            search: String::new(),
            asset_search: String::new(),
            mode: EditorMode::Select,
            workspace_tab: WorkspaceTab::Scene,
            preferences,
            lifecycle: initialized_lifecycle(),
            viewport_yaw: 0.0,
            viewport_pitch: 0.0,
            viewport_zoom: 1.0,
            viewport_pan: GlamVec2::ZERO,
            renderers,
            scene_viewport,
            game_viewport,
            notice: open_error,
            render_error: None,
        }
    }

    /// Writes the world back to the file it came from.
    fn save(&mut self) {
        match self.file.save(&self.world) {
            Ok(()) => {
                self.unsaved = false;
                self.notice = None;
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Asks for a scene file and opens it.
    ///
    /// Until this existed, the only way to open a scene was the command-line
    /// argument, which meant the editor could edit exactly the scene it was
    /// started on.
    fn open_scene(&mut self) {
        let started_in = self
            .file
            .path()
            .and_then(Path::parent)
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Sindri scene", &["json"])
            .set_directory(started_in)
            .pick_file()
        else {
            return;
        };
        self.open_path(&path);
    }

    /// Replaces the open scene with the one in `path`.
    ///
    /// A different scene is a different world, so every runtime handle in the
    /// old one is now meaningless: history is cleared rather than left pointing
    /// at entities that no longer exist, and so is the selection. The file is
    /// only adopted once its world loads, so a scene that fails to open leaves
    /// the editor on the one that was already working.
    fn open_path(&mut self, path: &Path) {
        let opened = match SceneFile::open(path) {
            Ok(opened) => opened,
            Err(error) => {
                self.notice = Some(error.to_string());
                return;
            }
        };
        match self.scene.load_world(opened.document()) {
            Ok(world) => {
                self.file = opened;
                self.world = world;
                self.history.clear();
                self.selection = None;
                self.unsaved = false;
                self.lifecycle = initialized_lifecycle();
                self.notice = None;
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Re-reads the file, discarding unsaved edits along with their history.
    fn reload(&mut self) {
        if let Err(error) = self.file.reload() {
            self.notice = Some(error.to_string());
            return;
        }
        self.reset_to_authored();
    }

    /// Turns pointer input over the viewport into camera movement.
    ///
    /// Left drag orbits, middle drag or shift-drag pans, and the wheel zooms.
    /// None of it touches the scene: the authored camera stays where it is and
    /// only the view of it moves.
    fn move_camera(&mut self, context: &egui::Context, response: &Response, height: f32) {
        if response.dragged() {
            let delta = response.drag_motion();
            if response.dragged_by(egui::PointerButton::Middle)
                || context.input(|input| input.modifiers.shift)
            {
                // Panning drags the picture, so it is measured against the
                // height of the viewport: dragging halfway up moves the scene
                // halfway up, at any zoom and under either projection.
                let height = height.max(1.0);
                self.viewport_pan.x += delta.x * 2.0 / height;
                self.viewport_pan.y -= delta.y * 2.0 / height;
            } else if response.dragged_by(egui::PointerButton::Primary) {
                self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                self.viewport_pitch = (self.viewport_pitch + delta.y * 0.008).clamp(-1.1, 1.1);
            }
        }
        if response.hovered() {
            let delta = context.input(|input| input.smooth_scroll_delta.y);
            self.viewport_zoom = (self.viewport_zoom + delta * 0.002).clamp(0.65, 1.8);
        }
    }

    /// Whether the viewer has moved away from the authored camera.
    fn view_moved(&self) -> bool {
        self.viewport_yaw != 0.0
            || self.viewport_pitch != 0.0
            || self.viewport_pan != GlamVec2::ZERO
            || (self.viewport_zoom - 1.0).abs() > f32::EPSILON
    }

    /// Returns to the camera the scene authored, without touching the scene.
    fn reset_view(&mut self) {
        self.viewport_yaw = 0.0;
        self.viewport_pitch = 0.0;
        self.viewport_pan = GlamVec2::ZERO;
        self.viewport_zoom = 1.0;
    }

    /// Changes the selection, ending any in-progress merge run so the next
    /// edit starts its own undo step.
    /// What to show the user, preferring what they just did over what the
    /// renderer is doing, since an action's failure is the newer news.
    fn problem(&self) -> Option<&str> {
        self.notice.as_deref().or(self.render_error.as_deref())
    }

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
        match self.history.apply(transaction, &mut self.world) {
            Ok(()) => self.unsaved = true,
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Moves an entity under a new parent, or out to the root with `None`.
    ///
    /// Its own transaction rather than part of the inspector draft: a parent
    /// change is one discrete choice, and merging it into a transform drag
    /// would make one undo step that both moved and reparented.
    ///
    /// The move is offered only where [`World::check_set_parent`] allows it, so
    /// reaching the error here means the world changed under the open menu. It
    /// is reported rather than ignored, because silently doing nothing is how
    /// an interface teaches people it is unreliable.
    fn reparent(&mut self, entity: EntityId, parent: Option<EntityId>) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetParent { entity, parent });
        self.history.break_merge_run();
        match self
            .history
            .apply(buffer.into_transaction("Reparent entity"), &mut self.world)
        {
            Ok(()) => self.unsaved = true,
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn undo(&mut self) {
        self.history.break_merge_run();
        match self.history.undo(&mut self.world) {
            Ok(_) => self.unsaved = true,
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn redo(&mut self) {
        self.history.break_merge_run();
        match self.history.redo(&mut self.world) {
            Ok(_) => self.unsaved = true,
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    /// Rebuilds the runtime scene from the authored document.
    ///
    /// Every runtime handle is replaced, so recorded history is discarded
    /// rather than left pointing at entities that no longer exist.
    fn reset_to_authored(&mut self) {
        match self.scene.load_world(self.file.document()) {
            Ok(world) => {
                self.world = world;
                self.history.clear();
                self.unsaved = false;
                self.selection = None;
                self.lifecycle = initialized_lifecycle();
                self.notice = None;
            }
            Err(error) => self.notice = Some(error.to_string()),
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
            self.notice = Some(error.to_string());
        }
    }

    fn pause(&mut self) {
        if self.lifecycle.state() == EngineState::Running
            && let Err(error) = self.lifecycle.pause()
        {
            self.notice = Some(error.to_string());
        }
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        let (undo, redo, save) = context.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                input.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y),
                input.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
            )
        });
        if save {
            self.save();
        }
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
                    self.file_menu(ui);
                    for menu in ["Edit", "Scene"] {
                        ui.add(
                            egui::Button::new(RichText::new(menu).size(12.0).color(TEXT_MUTED))
                                .frame(false),
                        );
                    }
                    self.view_menu(ui);
                    for menu in ["Build", "Tools", "Help"] {
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
        // Distinct ids per side deliberately: switching layouts should not
        // carry a width chosen for a different arrangement.
        let panel = match self.preferences.layout {
            WorkspaceLayout::TwoByThree => egui::Panel::right("hierarchy-column"),
            WorkspaceLayout::Wide => egui::Panel::left("hierarchy-dock"),
        };
        panel
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
                        let mut clicked: Option<Option<EntityId>> = None;
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
                                clicked = Some(Some(entity));
                            }
                        }
                        // Clicking past the last row clears the selection.
                        // Without somewhere to click that means "nothing", a
                        // selection made by accident can only be replaced.
                        if ui
                            .allocate_response(ui.available_size(), egui::Sense::click())
                            .clicked()
                        {
                            clicked = Some(None);
                        }
                        if let Some(entity) = clicked {
                            self.select(entity);
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
                let parent = data.parent;
                let choices = reparent_choices(&self.world, entity);
                let mut reparented = ParentChoice::Unchanged;
                {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, icon, &mut draft);
                        reparented = inspector_parent(ui, entity, parent, &choices);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
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
                match reparented {
                    ParentChoice::Unchanged => {}
                    ParentChoice::Root => self.reparent(entity, None),
                    ParentChoice::Under(parent) => self.reparent(entity, Some(parent)),
                }
            });
    }

    fn asset_panel(&mut self, ui: &mut egui::Ui) {
        let (panel, default, min, max) = match self.preferences.layout {
            // A tall column, which is what makes the list view worth having.
            WorkspaceLayout::TwoByThree => {
                (egui::Panel::right("project-column"), 280.0, 200.0, 420.0)
            }
            WorkspaceLayout::Wide => (egui::Panel::bottom("project-dock"), 226.0, 140.0, 330.0),
        };
        // The folder tree only fits when the browser is a wide dock.
        let folders = self.preferences.layout == WorkspaceLayout::Wide;
        panel
            .default_size(default)
            .min_size(min)
            .max_size(max)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    bottom_tab(
                        ui,
                        &mut self.preferences.bottom_tab,
                        BottomTab::Project,
                        "Project",
                    );
                    bottom_tab(
                        ui,
                        &mut self.preferences.bottom_tab,
                        BottomTab::Console,
                        "Console",
                    );
                });
                ui.separator();
                match self.preferences.bottom_tab {
                    BottomTab::Project => project_browser(
                        ui,
                        &mut self.asset_search,
                        &mut self.preferences.asset_view,
                        folders,
                    ),
                    BottomTab::Console => {
                        console_view(ui, self.world.len(), self.lifecycle.state());
                    }
                }
            });
    }

    /// Chooses how the workspace is arranged.
    ///
    /// The choice is a preference rather than session state, so it survives a
    /// restart: rearranging the editor every time it opens is the thing this
    /// exists to stop.
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("View").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(170.0);
            ui.label(RichText::new("Layout").size(11.0).color(TEXT_FAINT));
            for layout in WorkspaceLayout::ALL {
                if ui
                    .selectable_label(self.preferences.layout == layout, layout.label())
                    .clicked()
                {
                    self.preferences.layout = layout;
                    ui.close();
                }
            }
        });
    }

    /// Save is disabled rather than hidden when there is no file behind the
    /// scene, so the reason it cannot be used is visible.
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let saveable = self.file.path().is_some();
        ui.menu_button(RichText::new("File").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(190.0);
            if ui.button("Open scene…").clicked() {
                self.open_scene();
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    saveable,
                    egui::Button::new("Save scene").shortcut_text("Ctrl+S"),
                )
                .clicked()
            {
                self.save();
                ui.close();
            }
            if ui
                .add_enabled(saveable, egui::Button::new("Reload from disk"))
                .clicked()
            {
                self.reload();
                ui.close();
            }
            ui.separator();
            if ui.button("Discard changes").clicked() {
                self.reset_to_authored();
                ui.close();
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
                    let healthy = self.problem().is_none();
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
                    ui.add_space(10.0);
                    ui.label(RichText::new("|").size(11.0).color(BORDER));
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(if self.unsaved {
                            format!("{} (unsaved)", self.file.label())
                        } else {
                            self.file.label()
                        })
                        .size(11.0)
                        .color(if self.unsaved {
                            ACCENT
                        } else {
                            TEXT_MUTED
                        }),
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
                            RichText::new(if self.problem().is_some() {
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

    /// The row of tools above the viewport.
    ///
    /// The game view has none of them: they change what the editor is looking
    /// at, and that view exists to show what the player would see.
    fn scene_tools(&mut self, ui: &mut egui::Ui, editing: bool) {
        ui.horizontal(|ui| {
            if !editing {
                // The game view is what the player sees, so the tools
                // for changing what they are looking at do not apply.
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Rendering through the authored camera")
                        .size(11.0)
                        .color(TEXT_FAINT),
                );
                return;
            }
            // The projection pair claims its width first so the icon row
            // shrinks beside it. Laid out the other way round, a narrow
            // viewport drew the icons straight over the buttons.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                projection_button(
                    ui,
                    &mut self.preferences.projection,
                    CameraProjection::Orthographic,
                    "Ortho",
                );
                projection_button(
                    ui,
                    &mut self.preferences.projection,
                    CameraProjection::Perspective,
                    "Perspective",
                );
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
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
                    // "Local coordinates" and "Lit shading" used to sit here.
                    // Neither did anything, and at this width they pushed the
                    // controls that do work off the row. A button that cannot
                    // be pressed usefully costs more than the space it takes.
                    // Panning can carry the subject off screen entirely, so
                    // the way back is a control rather than a remembered
                    // number.
                    if icon_button(ui, ICON_CAMERA_ALT, self.view_moved(), "Reset view").clicked() {
                        self.reset_view();
                    }
                });
            });
        });
    }

    /// Draws one view of the world into whatever space `ui` has left.
    ///
    /// The Scene view takes camera input and wears editor chrome; the Game view
    /// takes neither, because chrome painted across what the player would see
    /// makes it something else. Both go through here so the two views cannot
    /// drift into being two renderers.
    fn render_view(&mut self, ui: &mut egui::Ui, tab: WorkspaceTab) {
        let context = ui.ctx().clone();
        let editing = tab == WorkspaceTab::Scene;
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
        if editing {
            self.move_camera(&context, &response, rect.height());
        }
        let scale = context.pixels_per_point();
        let camera = camera_for(
            tab,
            EditorCamera {
                orbit: GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                zoom: self.viewport_zoom,
                pan: self.viewport_pan,
                projection: self.preferences.projection,
            },
        );
        let viewport = if editing {
            &mut self.scene_viewport
        } else {
            &mut self.game_viewport
        };
        let failure = viewport
            .render(
                &mut self.renderers,
                &self.scene,
                &self.world,
                (
                    physical_viewport_dimension(rect.width(), scale),
                    physical_viewport_dimension(rect.height(), scale),
                ),
                camera,
            )
            .err();
        // Two views can be live at once, and the first thing to go wrong is the
        // thing worth reading, so a later success does not erase it.
        if self.render_error.is_none() {
            self.render_error = failure;
        }
        ui.painter().image(
            viewport.texture_id,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        if editing {
            paint_runtime_overlay(
                ui.painter(),
                rect,
                &self
                    .selection
                    .and_then(|entity| self.world.get(entity))
                    .map_or_else(|| "No selection".to_owned(), entity_name),
                self.problem(),
            );
        } else {
            paint_viewport_border(ui.painter(), rect, self.problem());
        }
        context.request_repaint();
    }

    /// The 2 by 3 workspace: Scene above Game, with the panels beside them.
    fn two_by_three_views(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                // The Game view is a bottom panel so it keeps its height while
                // the Scene view takes whatever is left.
                egui::Panel::bottom("game-view")
                    .default_size(300.0)
                    .min_size(120.0)
                    .resizable(true)
                    .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
                    .show(ui, |ui| {
                        view_title(ui, "Game");
                        ui.separator();
                        self.render_view(ui, WorkspaceTab::Game);
                    });
                view_title(ui, "Scene");
                ui.separator();
                self.scene_tools(ui, true);
                ui.separator();
                self.render_view(ui, WorkspaceTab::Scene);
            });
    }

    /// The wide workspace: one view at a time, chosen by a tab.
    fn tabbed_view(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Scene, "Scene");
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Game, "Game");
                });
                ui.separator();
                let tab = self.workspace_tab;
                self.scene_tools(ui, tab == WorkspaceTab::Scene);
                ui.separator();
                // Only the visible view is drawn: rendering the hidden one would
                // spend a frame's GPU work on something nobody is looking at.
                self.render_view(ui, tab);
            });
    }
}

impl eframe::App for EditorApp {
    /// Settings are written when eframe decides to, which includes shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.preferences.save(storage);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.top_bar(ui);
        self.status_bar(ui);
        // Panels claim space in the order they are shown, so this order is the
        // arrangement: each right panel sits to the left of the one before it.
        match self.preferences.layout {
            WorkspaceLayout::TwoByThree => {
                self.inspector_panel(ui);
                self.asset_panel(ui);
                self.hierarchy_panel(ui);
                self.render_error = None;
                self.two_by_three_views(ui);
            }
            WorkspaceLayout::Wide => {
                self.hierarchy_panel(ui);
                self.inspector_panel(ui);
                self.asset_panel(ui);
                self.render_error = None;
                self.tabbed_view(ui);
            }
        }
        // Releasing the pointer ends a drag, so the next one is its own step.
        if ui.ctx().input(|input| input.pointer.any_released()) {
            self.history.break_merge_run();
        }
        // Escape clears the selection wherever the pointer happens to be. The
        // hierarchy's empty space does the same, but only while it has empty
        // space to click.
        if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
            self.select(None);
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

/// What the root is called wherever a parent is named.
const ROOT_LABEL: &str = "World";

/// The parents `entity` may legally be moved under, in the order the hierarchy
/// lists them.
///
/// Legality is asked of the world rather than decided here, so the menu cannot
/// offer a move the command layer would then refuse. The root is not in this
/// list because it is not an entity; it is the separate "World" choice.
fn reparent_choices(world: &World, entity: EntityId) -> Vec<(EntityId, String)> {
    hierarchy_rows(world)
        .into_iter()
        .filter(|(candidate, _)| world.check_set_parent(entity, Some(*candidate)).is_ok())
        .filter_map(|(candidate, _)| {
            world
                .get(candidate)
                .map(|data| (candidate, entity_name(data)))
        })
        .collect()
}

/// What the parent menu came back with.
///
/// "Move to the root" and "nothing was chosen" are both an absence of a parent
/// and are not the same answer, so they are separate variants rather than two
/// layers of `Option` the caller has to remember the order of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParentChoice {
    /// The menu offered no change: it is closed, or the current parent was
    /// picked again.
    Unchanged,
    /// Move out to the root.
    Root,
    /// Move under this entity.
    Under(EntityId),
}

/// The parent row, reporting a choice only when it is a change.
fn inspector_parent(
    ui: &mut egui::Ui,
    entity: EntityId,
    parent: Option<EntityId>,
    choices: &[(EntityId, String)],
) -> ParentChoice {
    let mut chosen = parent;
    let current = parent
        .and_then(|parent| {
            choices
                .iter()
                .find(|(candidate, _)| *candidate == parent)
                .map(|(_, name)| name.clone())
        })
        .unwrap_or_else(|| ROOT_LABEL.to_owned());
    ui.horizontal(|ui| {
        ui.add_space(27.0);
        ui.label(RichText::new("Parent").size(11.0).color(TEXT_FAINT));
        egui::ComboBox::from_id_salt(("parent", entity.index()))
            .selected_text(RichText::new(current).size(11.0).color(TEXT_MUTED))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, None, ROOT_LABEL);
                for (candidate, name) in choices {
                    ui.selectable_value(&mut chosen, Some(*candidate), name);
                }
            });
    });
    if chosen == parent {
        return ParentChoice::Unchanged;
    }
    chosen.map_or(ParentChoice::Root, ParentChoice::Under)
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

/// The components an entity carries, read from the entity's own payloads.
///
/// The rows here used to be fixed text that described the demo scene whatever
/// was open, which is worse than showing nothing: a sprite anchored bottom
/// right read as an overlay badge. Each built-in component is deserialized
/// through the same schema the runtime uses, so a row is either the value the
/// scene holds or an admission that the payload could not be read.
fn components_sections(ui: &mut egui::Ui, components: &BTreeMap<String, Value>) {
    for (name, payload) in components {
        let icon = match name.as_str() {
            "sindri.camera" => ICON_CAMERA_ALT,
            "sindri.sprite" => ICON_IMAGE,
            "sindri.mesh" => ICON_VIEW_IN_AR,
            _ => ICON_DEPLOYED_CODE,
        };
        section_header(ui, icon, &component_label(name));
        for (label, value) in component_rows(name, payload) {
            property_label(ui, &label, &value);
        }
    }
}

/// What one component's rows say, kept apart from the drawing of them so the
/// claim that they are the entity's own values is something a test can check.
fn component_rows(name: &str, payload: &Value) -> Vec<(String, String)> {
    fn row(label: &str, value: impl Into<String>) -> (String, String) {
        (label.to_owned(), value.into())
    }

    match name {
        "sindri.camera" => match serde_json::from_value::<CameraComponent>(payload.clone()) {
            Ok(CameraComponent::Perspective {
                vertical_fov_degrees,
                near,
                far,
                ..
            }) => vec![
                row("Projection", "Perspective"),
                row("Field of view", format!("{vertical_fov_degrees}°")),
                row("Clipping", format!("{near} - {far}")),
            ],
            Ok(CameraComponent::Orthographic {
                vertical_size,
                near,
                far,
                ..
            }) => vec![
                row("Projection", "Orthographic"),
                row("Vertical size", vertical_size.to_string()),
                row("Clipping", format!("{near} - {far}")),
            ],
            Err(error) => unreadable_payload(&error),
        },
        "sindri.sprite" => match serde_json::from_value::<SpriteComponent>(payload.clone()) {
            Ok(sprite) => {
                let mut rows = vec![
                    row("Texture", sprite.texture.clone()),
                    row("Space", sprite_space_label(sprite.space)),
                ];
                // Only a screen-space sprite has an edge to hang from, so a
                // world-space one is not offered an anchor to misread.
                if let Some(anchor) = sprite.screen_anchor() {
                    rows.push(row("Anchor", anchor_label(anchor)));
                }
                rows.push(row("Layer", sprite.layer.to_string()));
                rows
            }
            Err(error) => unreadable_payload(&error),
        },
        "sindri.mesh" => match serde_json::from_value::<MeshComponent>(payload.clone()) {
            Ok(mesh) => vec![
                row(
                    "Mesh",
                    match mesh.primitive {
                        MeshPrimitive::Cube => "Cube",
                        // The enum is deliberately open: a primitive added to
                        // the engine must not stop the editor building, and
                        // this row is the only thing that has to catch up.
                        _ => "Unnamed primitive",
                    },
                ),
                row("Texture", mesh.texture.clone()),
                row("Layer", mesh.layer.to_string()),
            ],
            Err(error) => unreadable_payload(&error),
        },
        _ => vec![row("Fields", payload_summary(payload))],
    }
}

/// A payload the built-in schema rejected. Said out loud, because a component
/// row that silently showed nothing would look like a component with nothing in
/// it.
fn unreadable_payload(error: &serde_json::Error) -> Vec<(String, String)> {
    vec![("Unreadable".to_owned(), error.to_string())]
}

/// What a component nothing here knows about is carrying, which is at least its
/// field names.
fn payload_summary(payload: &Value) -> String {
    payload.as_object().map_or_else(
        || payload.to_string(),
        |fields| {
            if fields.is_empty() {
                "none".to_owned()
            } else {
                fields.keys().cloned().collect::<Vec<_>>().join(", ")
            }
        },
    )
}

const fn sprite_space_label(space: SpriteSpace) -> &'static str {
    match space {
        SpriteSpace::Screen => "Screen",
        SpriteSpace::World => "World",
    }
}

const fn anchor_label(anchor: SpriteAnchor) -> &'static str {
    match anchor {
        SpriteAnchor::Center => "Center",
        SpriteAnchor::Top => "Top",
        SpriteAnchor::Bottom => "Bottom",
        SpriteAnchor::Left => "Left",
        SpriteAnchor::Right => "Right",
        SpriteAnchor::TopLeft => "Top left",
        SpriteAnchor::TopRight => "Top right",
        SpriteAnchor::BottomLeft => "Bottom left",
        SpriteAnchor::BottomRight => "Bottom right",
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

/// What the project browser shows, until it reads a real asset directory.
///
/// Each entry carries its kind as well as its name, because a list has room to
/// say what a thing is and a grid of generic icons does not.
fn project_assets() -> [(MaterialIcon, &'static str, &'static str); 8] {
    [
        (ICON_FOLDER, "Materials", "Folder"),
        (ICON_FOLDER, "Models", "Folder"),
        (ICON_FOLDER, "Scenes", "Folder"),
        (ICON_FOLDER, "Scripts", "Folder"),
        (ICON_DESCRIPTION, "demo.scene", "Scene"),
        (ICON_VIEW_IN_AR, "checker_cube", "Mesh"),
        (ICON_IMAGE, "badge", "Texture"),
        (ICON_CODE, "scene.rs", "Script"),
    ]
}

/// The project browser, in one column or two.
///
/// Two panes need width the bottom dock has and a side column does not: at
/// column width the folder tree and the asset list were drawing over each
/// other. So the narrow arrangement drops the tree rather than shrinking it,
/// which is also why a list reads better there than a grid of identical icons.
fn project_browser(ui: &mut egui::Ui, search: &mut String, view: &mut AssetView, folders: bool) {
    if !folders {
        asset_column(ui, search, view);
        return;
    }
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
        ui.vertical(|ui| asset_column(ui, search, view));
    });
}

/// The asset side of the browser: what it is showing, and how.
fn asset_column(ui: &mut egui::Ui, search: &mut String, view: &mut AssetView) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Assets").size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_VIEW_LIST, *view == AssetView::List, "List view").clicked() {
                *view = AssetView::List;
            }
            if icon_button(ui, ICON_GRID_VIEW, *view == AssetView::Grid, "Grid view").clicked() {
                *view = AssetView::Grid;
            }
            icon_button(ui, ICON_FILTER_LIST, false, "Filter assets");
            // Whatever is left after the buttons, rather than a fixed width
            // that overflowed the moment the browser became a column.
            let room = (ui.available_width() - 6.0).clamp(60.0, 210.0);
            ui.add_sized(
                [room, 27.0],
                egui::TextEdit::singleline(search).hint_text("Search"),
            );
        });
    });
    ui.add_space(8.0);
    // A project has more assets than a dock has room for, in either
    // presentation. Scrolling here is what lets the list be the default
    // without the last few assets falling off the bottom.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match view {
            AssetView::Grid => {
                ui.horizontal_wrapped(|ui| {
                    for (icon, label, _) in project_assets() {
                        asset_tile(ui, icon, label);
                    }
                });
            }
            AssetView::List => {
                // Rows are denser than egui's default spacing, so the dock
                // shows a useful number of them without taking height from
                // the viewport it sits under.
                ui.spacing_mut().item_spacing.y = 1.0;
                for (icon, label, kind) in project_assets() {
                    asset_row(ui, icon, label, kind);
                }
            }
        });
}

/// One asset as a row: what it is called, and what it is.
fn asset_row(ui: &mut egui::Ui, icon: MaterialIcon, label: &str, kind: &str) {
    let highlighted = label == "demo.scene";
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            icon.outlined()
                .rich_text()
                .size(15.0)
                .color(if highlighted { ACCENT } else { TEXT_FAINT }),
        );
        ui.add_space(2.0);
        ui.label(RichText::new(label).size(11.0).color(if highlighted {
            TEXT
        } else {
            TEXT_MUTED
        }));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(kind).size(10.0).color(TEXT_FAINT));
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

/// The name above a view when both are on screen at once.
///
/// A label rather than a tab: in this layout the view is already visible, so a
/// control that selects it would do nothing.
fn view_title(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(12.0).color(TEXT));
    });
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
        "Live WGPU viewport  ·  Drag to orbit  ·  Shift-drag to pan",
        FontId::proportional(10.0),
        TEXT_FAINT,
    );
    paint_error_banner(painter, rect, error);
    paint_axis_gizmo(painter, Pos2::new(rect.right() - 42.0, rect.top() + 48.0));
}

/// The game view's chrome: a frame, and anything that went wrong.
///
/// A render failure is still reported here, because a blank view with no
/// explanation is worse than a view with a message across it.
fn paint_viewport_border(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    paint_error_banner(painter, rect, error);
}

fn paint_error_banner(painter: &egui::Painter, rect: Rect, error: Option<&str>) {
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
    transform_3d: Option<Transform3D>,
}

impl From<&EntityData> for EntityDraft {
    fn from(data: &EntityData) -> Self {
        Self {
            name: entity_name(data),
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

/// Only tests look entities up by their authored ID; the editor works in
/// runtime handles.
#[cfg(test)]
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

/// Opens the scene named on the command line, or the demo scene beside it.
///
/// A missing or unreadable file is reported rather than fatal: the editor opens
/// on the scene compiled into it and says what went wrong, which beats a window
/// that never appears.
fn open_requested_scene() -> (SceneFile, Option<String>) {
    let requested = std::env::args().nth(1);
    let path = requested.as_deref().unwrap_or(DEFAULT_SCENE_PATH);
    match SceneFile::open(path) {
        Ok(file) => (file, None),
        Err(error) => {
            let embedded = DemoScene::authored_document()
                .expect("the embedded editor fixture must remain a valid scene");
            (SceneFile::detached(embedded), Some(error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use sindri_core::{CommandHistory, SceneDocument, SceneEntity, SceneEntityId};

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

    /// The parent menu must not offer a move the command layer would refuse,
    /// which for an ancestor means none of its own descendants.
    #[test]
    fn the_parent_menu_never_offers_a_move_that_would_make_a_cycle() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let named = |id: &str| find_by_source_id(&world, id).unwrap();
        let offered: Vec<String> = reparent_choices(&world, named("torso"))
            .into_iter()
            .map(|(_, name)| name)
            .collect();

        assert_eq!(
            offered,
            vec!["Root".to_owned(), "Leg".to_owned()],
            "torso may move to the root or under its sibling, but not under \
             itself or its own child"
        );
    }

    /// The selected-parent label is looked up in this list, so a parent missing
    /// from it would be drawn as though the entity sat at the root.
    #[test]
    fn the_parent_menu_always_contains_the_parent_an_entity_already_has() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        for (entity, data) in world.entities() {
            let Some(parent) = data.parent else {
                continue;
            };
            assert!(
                reparent_choices(&world, entity)
                    .iter()
                    .any(|(candidate, _)| *candidate == parent),
                "{} sits under a parent its own menu does not list",
                entity_name(data)
            );
        }
    }

    /// A leaf can go anywhere, so this is the case that would hide a filter
    /// that was accidentally excluding legal parents.
    #[test]
    fn a_leaf_may_move_under_anything_but_itself() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let arm = find_by_source_id(&world, "arm").unwrap();
        let offered: Vec<String> = reparent_choices(&world, arm)
            .into_iter()
            .map(|(_, name)| name)
            .collect();

        assert_eq!(
            offered,
            vec!["Root".to_owned(), "Leg".to_owned(), "Torso".to_owned()]
        );
    }

    #[test]
    fn reparenting_moves_the_entity_and_undoes_in_one_step() {
        let mut world = World::from_scene(&nested_scene()).unwrap().world;
        let arm = find_by_source_id(&world, "arm").unwrap();
        let leg = find_by_source_id(&world, "leg").unwrap();
        let torso = world.get(arm).unwrap().parent.unwrap();

        let mut history = CommandHistory::default();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetParent {
            entity: arm,
            parent: Some(leg),
        });
        history
            .apply(buffer.into_transaction("Reparent entity"), &mut world)
            .unwrap();

        assert_eq!(world.get(arm).unwrap().parent, Some(leg));
        assert!(
            !world.get(torso).unwrap().children.contains(&arm),
            "the old parent should no longer claim the child"
        );

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(arm).unwrap().parent, Some(torso));
        assert!(world.get(torso).unwrap().children.contains(&arm));
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

    fn moved_camera() -> EditorCamera {
        EditorCamera {
            orbit: GlamVec2::new(0.7, -0.3),
            zoom: 1.4,
            pan: GlamVec2::new(0.25, 0.5),
            projection: CameraProjection::Orthographic,
        }
    }

    /// The game view answers one question — what would the player see — and an
    /// orbit, pan, or zoom leaking into it would stop it answering that.
    #[test]
    fn the_game_view_ignores_wherever_the_editor_has_moved_its_camera() {
        assert_eq!(
            camera_for(WorkspaceTab::Game, moved_camera()),
            CameraView::default(),
            "the game view must render through the authored camera"
        );
    }

    #[test]
    fn the_scene_view_carries_every_editor_adjustment() {
        let camera = camera_for(WorkspaceTab::Scene, moved_camera());
        assert_eq!(camera.orbit, GlamVec2::new(0.7, -0.3));
        assert_eq!(camera.pan, GlamVec2::new(0.25, 0.5));
        assert!(
            (camera.distance_scale - 1.0 / 1.4).abs() < 1.0e-6,
            "zooming in should shorten the distance to the target"
        );
        assert_eq!(camera.projection, WorldProjection::Orthographic);
    }

    #[test]
    fn an_unmoved_scene_view_matches_the_authored_camera() {
        // Opening the editor and drawing nothing must not move the camera, or
        // the scene and game views would disagree before anyone touched them.
        let resting = EditorCamera {
            orbit: GlamVec2::ZERO,
            zoom: 1.0,
            pan: GlamVec2::ZERO,
            projection: CameraProjection::Perspective,
        };
        assert_eq!(
            camera_for(WorkspaceTab::Scene, resting),
            camera_for(WorkspaceTab::Game, resting)
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

    /// The inspector's component rows are the entity's own values. They were
    /// fixed text for long enough that this is worth holding in place.
    #[test]
    fn component_rows_read_the_payload() {
        let sprite = serde_json::json!({
            "texture": "textures/badge.png",
            "anchor": "bottom_right",
            "layer": 100
        });
        assert_eq!(
            component_rows("sindri.sprite", &sprite),
            [
                ("Texture".to_owned(), "textures/badge.png".to_owned()),
                ("Space".to_owned(), "Screen".to_owned()),
                ("Anchor".to_owned(), "Bottom right".to_owned()),
                ("Layer".to_owned(), "100".to_owned()),
            ]
        );

        let mesh = serde_json::json!({ "primitive": "cube", "texture": "procedural:checkerboard" });
        assert_eq!(
            component_rows("sindri.mesh", &mesh),
            [
                ("Mesh".to_owned(), "Cube".to_owned()),
                ("Texture".to_owned(), "procedural:checkerboard".to_owned()),
                ("Layer".to_owned(), "0".to_owned()),
            ]
        );
    }

    /// A world-space sprite has no edge to anchor to, so it is offered no
    /// anchor row to read as though it did.
    #[test]
    fn a_world_space_sprite_is_shown_no_anchor() {
        let sprite = serde_json::json!({
            "texture": "textures/tree.png",
            "space": "world",
            "anchor": "top_left"
        });
        let rows = component_rows("sindri.sprite", &sprite);
        assert_eq!(rows[1], ("Space".to_owned(), "World".to_owned()));
        assert!(
            !rows.iter().any(|(label, _)| label == "Anchor"),
            "a world sprite was offered an anchor: {rows:?}"
        );
    }

    /// A component nothing here knows about still says what it is carrying.
    #[test]
    fn an_unknown_component_lists_its_fields() {
        let payload = serde_json::json!({ "speed": 4.0, "facing": "north" });
        assert_eq!(
            component_rows("game.walker", &payload),
            [("Fields".to_owned(), "facing, speed".to_owned())]
        );
    }
}
