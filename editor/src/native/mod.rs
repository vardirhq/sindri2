use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use eframe::{
    egui::{
        self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Shape,
        Stroke, StrokeKind, Vec2,
    },
    wgpu,
};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_ACCOUNT_TREE, ICON_ADD, ICON_CAMERA_ALT, ICON_CENTER_FOCUS_STRONG, ICON_CODE,
        ICON_DELETE, ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_FOLDER, ICON_GRID_4X4,
        ICON_GRID_VIEW, ICON_IMAGE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
        ICON_LABEL, ICON_MOVE, ICON_PAUSE, ICON_PLAY_ARROW, ICON_REDO, ICON_REFRESH,
        ICON_ROTATE_RIGHT, ICON_SCALE, ICON_SELECT, ICON_STOP, ICON_UNDO, ICON_VIEW_IN_AR,
        ICON_VIEW_LIST,
    },
};
use glam::{Mat4, Vec2 as GlamVec2, Vec3};
use sindri_core::{
    CommandBuffer, CommandHistory, EngineLifecycle, EngineState, EntityData, EntityId,
    FixedStepConfig, SceneComponent, SceneDocument, SceneEntityId, Transform3D,
    UnknownComponentPolicy, World, WorldCommand,
};
use sindri_decay::ScriptComponent;
use sindri_render::{
    FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer, TexturedCubeRenderer, Viewport,
    ViewportTarget, encode_prepared_frame,
};
use sindri_scene::{
    CameraView, GridNavigationComponent, GridOccupantComponent, SceneExtractor, SpriteAnimations,
    SpriteSpace, ViewCamera,
};

use crate::{
    animation::AnimationTool,
    console::Console,
    gizmo::{self, Axis, GizmoDrag, GizmoMode, GizmoSpace, Snapping},
    input::EditorInput,
    picking,
    // `egui::Layout` is a different thing entirely and is already in scope.
    preferences::{AssetView, BottomTab, CameraProjection, Layout as WorkspaceLayout, Preferences},
    project::{AssetKind, ProjectEntry, ProjectTree},
    scene_file::{DEFAULT_SCENE_PATH, SceneFile},
    scripts::SceneScripts,
    slicer::Slicer,
    textures::SceneTextures,
    tilemap::{self, TileBrush, TilemapTool, paint as paint_tile},
};

mod camera;
mod console_view;
mod inspector_panel;
mod theme;

#[cfg(test)]
mod tests;

use camera::{EditorCamera, camera_for};
use console_view::console_view;
use inspector_panel::number_row;

use theme::{
    ACCENT, ACCENT_BRIGHT, ACCENT_SOFT, APP_BG, BORDER, BORDER_SUBTLE, PANEL_BG, PANEL_RAISED,
    PROBLEM, SUCCESS, TEXT, TEXT_FAINT, TEXT_MUTED, TOP_BG, configure_theme, icon_button,
    panel_title, search_field, section_header, status_dot, view_title,
};

const TEXT_COMPONENT: &str = "sindri.text";
const GRID_NAVIGATION_COMPONENT: &str = GridNavigationComponent::TYPE_NAME;
const GRID_OCCUPANT_COMPONENT: &str = GridOccupantComponent::TYPE_NAME;
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

/// Something the user asked for that would throw unsaved work away.
///
/// Each of these used to happen the moment it was clicked. Two of them are in a
/// menu, one is the window's close button, and one was the Stop button, which
/// reset the scene rather than stopping anything.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Discarding {
    OpenAnother,
    /// A scene chosen in the project browser, which knows the path already and
    /// so has no dialog to open.
    OpenPath(PathBuf),
    Reload,
    Reset,
    Close,
}

/// One tile under the Scene-view pointer, already projected back into the
/// viewport so input and its feedback use the same answer.
struct TilemapHover {
    entity: EntityId,
    column: u32,
    row: u32,
    outline: [Pos2; 4],
}

impl Discarding {
    /// What the user is about to lose the work to, in the words of the control
    /// they pressed.
    const fn question(&self) -> &'static str {
        match self {
            Self::OpenAnother | Self::OpenPath(_) => {
                "Open another scene and discard the changes to this one?"
            }
            Self::Reload => "Re-read this scene from disk and discard the changes?",
            Self::Reset => "Discard the changes and go back to the scene as it was saved?",
            Self::Close => "Close the editor and discard the changes?",
        }
    }

    const fn verb(&self) -> &'static str {
        match self {
            Self::OpenAnother | Self::OpenPath(_) => "Open anyway",
            Self::Reload => "Reload anyway",
            Self::Reset => "Discard",
            Self::Close => "Close anyway",
        }
    }
}

/// The editing shortcuts pressed this frame.
///
/// Four bools, which the pedantic lint reads as a struct that should have been
/// an enum. It should not: these are independent, a frame can carry more than
/// one, and each is exactly the yes-or-no its key asks.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Shortcuts {
    focus: bool,
    undo: bool,
    redo: bool,
    save: bool,
}

/// Reads the editing shortcuts, most specific first.
///
/// Order is the whole of it. egui matches modifiers logically, so an extra
/// Shift is ignored and a Ctrl+Shift+Z tested against Ctrl+Z matches it —
/// which meant the editor's redo shortcut was consumed by undo and performed
/// one. Redo is asked first so that it sees its own keys.
fn shortcuts(input: &mut egui::InputState) -> Shortcuts {
    let redo = input.consume_key(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    ) || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
    Shortcuts {
        redo,
        undo: input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
        save: input.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
        // Unmodified, as it is everywhere else that frames a selection. A text
        // field with focus consumes the key before this sees it, so typing an
        // "f" into a name does not move the camera.
        focus: input.consume_key(egui::Modifiers::NONE, egui::Key::F),
    }
}

/// What the confirm dialog came back with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Answer {
    Cancel,
    Discard,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTab {
    Scene,
    Game,
}

/// The GPU pipelines every viewport draws with.
///
/// Held once rather than per viewport: a pipeline does not depend on which
/// camera is looking, and two viewports that each built their own would pay
/// twice for the same thing. The textures used to live here too, handed over by
/// the cube example; they belong to the open scene, which is where they are now.
struct SceneRenderers {
    cube: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
}

impl SceneRenderers {
    fn new(render_state: &eframe::egui_wgpu::RenderState) -> Self {
        Self {
            cube: TexturedCubeRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            sprites: SpriteBatchRenderer::new(&render_state.device, ViewportTarget::FORMAT),
            text: TextRenderer::new(
                &render_state.device,
                &render_state.queue,
                ViewportTarget::FORMAT,
            ),
        }
    }
}

/// One rendered view of the world, and the egui texture it is drawn through.
/// Everything a frame is derived from: the world, the schemas that read it, and
/// the runtime state beside it.
///
/// Together rather than as five arguments, because they are one thing — the
/// state of the open scene — and each view draws all of it or none of it.
#[derive(Clone, Copy)]
struct SceneSource<'a> {
    scene: &'a SceneExtractor,
    world: &'a World,
    animations: &'a SpriteAnimations,
    textures: &'a SceneTextures,
}

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
        source: SceneSource<'_>,
        size: (u32, u32),
        camera: CameraView,
    ) -> Result<(), String> {
        self.resize(size.0, size.1);
        let prepared = source
            .scene
            .extract_animated(
                source.world,
                Viewport::new(self.target.width(), self.target.height()),
                camera,
                source.textures.bindings(),
                source.animations,
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

struct EditorApp {
    scene: SceneExtractor,
    world: World,
    file: SceneFile,
    /// The history revision the open file was last agreed with.
    ///
    /// Unsaved work is the world having moved away from it, which undoing back
    /// reverses. The flag this replaced was set by every edit and cleared only
    /// by a save, so undoing to exactly the saved state still claimed there was
    /// something to lose.
    saved_revision: u64,
    /// What the user asked for that would throw unsaved work away, waiting on
    /// an answer.
    confirming: Option<Discarding>,
    /// Set once closing has been agreed to, so the close request the editor
    /// cancelled to ask the question is not cancelled a second time.
    closing: bool,
    selection: Option<EntityId>,
    history: CommandHistory,
    search: String,
    asset_search: String,
    /// The image being sliced, when one was selected in the browser.
    ///
    /// Selecting an asset and selecting an entity are the same act from the
    /// user's side — "show me this" — so they share the inspector and clear
    /// each other rather than fighting over it.
    slicer: Option<Slicer>,
    /// The tile brush and palette are editor state, not scene state. A map
    /// stores what was painted; it does not store which brush the author last
    /// held or whether the Scene view currently belongs to that brush.
    tilemap_tool: TilemapTool,
    /// Clip selection and playback cursor for the inspector's animation
    /// preview. Like runtime animation state, none of this is scene data.
    animation_tool: AnimationTool,
    /// Which sliced images are showing their sprites.
    ///
    /// Collapsed until asked for, and not remembered across launches: which
    /// sheet you were looking inside is about the minute rather than the
    /// project, unlike the panel sizes beside it in `Preferences`.
    expanded_sheets: BTreeSet<PathBuf>,
    /// The directory the open scene lives in, as it was last read.
    ///
    /// Read when a scene is opened rather than every frame: the browser redraws
    /// at the viewport's frame rate and a directory does not, so a walk per
    /// frame would be a syscall for every row sixty times a second.
    project: ProjectTree,
    workspace_tab: WorkspaceTab,
    preferences: Preferences,
    lifecycle: EngineLifecycle,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
    viewport_pan: GlamVec2,
    gizmo_mode: GizmoMode,
    gizmo_space: GizmoSpace,
    gizmo_snapping: Snapping,
    gizmo_drag: Option<GizmoDrag>,
    renderers: SceneRenderers,
    /// eframe's device and queue, kept because textures are uploaded whenever a
    /// load completes rather than only while a viewport is being drawn.
    render_state: eframe::egui_wgpu::RenderState,
    /// The textures the open scene draws with, loaded from its own directory.
    textures: SceneTextures,
    /// The history revision the textures were last asked about.
    ///
    /// An edit can point a mesh at a different texture, and the world is the
    /// only statement of what a scene references, so a change to it is the
    /// signal to ask again. The revision is what makes "changed" cheap to spot.
    textured_revision: u64,
    scene_viewport: RuntimeViewport,
    game_viewport: RuntimeViewport,
    /// Where each animated sprite has got to.
    ///
    /// Runtime state, so it lives here rather than in the world: an animation
    /// playing must not be an unsaved change. Play advances it, pause holds it,
    /// and stop puts every clip back to its first frame.
    animations: SpriteAnimations,
    /// The scripts the open scene runs, and the sources behind them.
    scripts: SceneScripts,
    /// The keyboard a running script reads, translated from egui's.
    input: EditorInput,
    /// The world as it was when Play was pressed.
    ///
    /// Scripts write to the world, which animation never did, so stopping has
    /// to put back what playing changed. Restoring the *authored document*
    /// instead would also throw away every edit made before pressing Play,
    /// which is the author's work rather than the run's.
    play_snapshot: Option<World>,
    /// What the last action the user took had to say, if anything went wrong.
    ///
    /// Kept apart from `render_error` because the two have different lifetimes:
    /// this one stays until something replaces it, while a render result is
    /// recomputed every frame. They shared a field, and the render overwrote a
    /// failed save within one frame of it happening.
    notice: Option<String>,
    /// Whatever the last frame's render reported.
    render_error: Option<String>,
    /// Everything the editor has said, in order.
    console: Console,
    /// The window title as last set.
    ///
    /// Kept so the title is sent when it changes rather than every frame: a
    /// viewport command per frame is sixty round trips a second to say the same
    /// thing.
    title: String,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&context.egui_ctx);
        let preferences = Preferences::load(context.storage);
        let scene = scene_extractor();
        let (file, open_error) = open_requested_scene(preferences.last_scene.as_deref());
        // A scene that will not load must not take the editor down with it.
        // This used to unwrap, so a file that parsed and then failed validation
        // killed the process before the window existed — and the failure it
        // unwrapped was one the editor should not have had in the first place.
        let (world, load_error) = match load_world(&scene, file.document()) {
            Ok(world) => (world, None),
            Err(error) => (World::default(), Some(error)),
        };
        // Nothing is selected until something is chosen. This used to name an
        // entity from the demo scene, which selected the cube in that one scene
        // and silently nothing in every other.
        let selection = None;
        let render_state = context
            .wgpu_render_state
            .clone()
            .expect("the native editor requires eframe's WGPU renderer");
        let renderers = SceneRenderers::new(&render_state);
        let textures =
            SceneTextures::for_scene(&render_state.device, &render_state.queue, file.path());
        let state_for_textures = render_state.clone();
        let scene_viewport = RuntimeViewport::new(render_state.clone(), "Sindri editor scene view");
        let game_viewport = RuntimeViewport::new(render_state, "Sindri editor game view");
        let project = ProjectTree::beside(file.path());
        let mut app = Self {
            scene,
            world,
            file,
            saved_revision: 0,
            confirming: None,
            closing: false,
            selection,
            history: CommandHistory::default(),
            search: String::new(),
            asset_search: String::new(),
            slicer: None,
            tilemap_tool: TilemapTool::default(),
            animation_tool: AnimationTool::default(),
            expanded_sheets: BTreeSet::new(),
            project,
            workspace_tab: WorkspaceTab::Scene,
            preferences,
            lifecycle: initialized_lifecycle(),
            viewport_yaw: 0.0,
            viewport_pitch: 0.0,
            viewport_zoom: 1.0,
            viewport_pan: GlamVec2::ZERO,
            gizmo_mode: GizmoMode::Select,
            gizmo_space: GizmoSpace::Local,
            gizmo_snapping: Snapping::default(),
            gizmo_drag: None,
            renderers,
            render_state: state_for_textures,
            textures,
            textured_revision: 0,
            scene_viewport,
            game_viewport,
            animations: SpriteAnimations::new(),
            scripts: SceneScripts::for_scene(None),
            input: EditorInput::default(),
            play_snapshot: None,
            notice: open_error.or(load_error),
            render_error: None,
            console: Console::default(),
            title: String::new(),
        };
        // Said after the field is built rather than during it, because what
        // there is to say is read off the world and the bindings.
        if let Some(failure) = app.notice.clone() {
            app.console.error(failure);
        }
        app.announce_scene();
        let notes = app.textures.request(&app.world, &mut app.renderers.text);
        app.record_texture_notes(notes);
        app.reload_scripts();
        app.remember_open_scene();
        app
    }

    /// Whether the world has moved away from what the file holds.
    fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    /// Writes the world back to the file it came from.
    fn save(&mut self) {
        match self.file.save(&self.world) {
            Ok(()) => {
                self.saved_revision = self.history.revision();
                self.notice = None;
                self.console.info(format!("Saved {}", self.file.label()));
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Does what was asked, or asks first when it would cost unsaved work.
    fn discard_or_confirm(&mut self, action: Discarding, context: &egui::Context) {
        if self.unsaved() {
            self.confirming = Some(action);
        } else {
            self.discard(action, context);
        }
    }

    /// Carries out an action that throws away whatever is unsaved.
    fn discard(&mut self, action: Discarding, context: &egui::Context) {
        self.confirming = None;
        match action {
            Discarding::OpenAnother => self.open_scene(),
            Discarding::OpenPath(path) => self.open_path(&path),
            Discarding::Reload => self.reload(),
            Discarding::Reset => self.reset_to_authored(),
            // Agreeing to close is not closing. The request that raised the
            // question was cancelled, so nothing is asking the window to go any
            // more; this asks again, and the flag lets that one through.
            Discarding::Close => {
                self.closing = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
                self.report(error.to_string());
                return;
            }
        };
        match load_world(&self.scene, opened.document()) {
            Ok(world) => {
                self.file = opened;
                self.project = ProjectTree::beside(self.file.path());
                self.world = world;
                self.history.clear();
                self.selection = None;
                self.tilemap_tool.reset();
                self.animation_tool.reset();
                self.saved_revision = self.history.revision();
                self.lifecycle = initialized_lifecycle();
                // A cursor belongs to the world it was advanced against, and
                // a freshly loaded world reuses entity slots from the start.
                self.animations = SpriteAnimations::new();
                self.play_snapshot = None;
                self.notice = None;
                self.announce_scene();
                self.reload_textures();
                self.reload_scripts();
                self.remember_open_scene();
            }
            Err(error) => self.report(error),
        }
    }

    /// Re-reads the file, discarding unsaved edits along with their history.
    fn reload(&mut self) {
        if let Err(error) = self.file.reload() {
            self.report(error.to_string());
            return;
        }
        self.refresh_project();
        self.reset_to_authored();
    }

    /// Re-reads the directory the browser is showing.
    ///
    /// The tree is cached, so a file added or removed outside the editor is
    /// invisible until something asks for it again. Opening or reloading a
    /// scene asks, and so does the browser's own refresh control — which is
    /// what used to be an inert filter icon.
    fn refresh_project(&mut self) {
        self.project = ProjectTree::beside(self.file.path());
        self.tilemap_tool.palette.invalidate();
        self.animation_tool.palette.invalidate();
    }

    /// Resolves the selected tilemap and pointer through the same camera used
    /// for this frame. Screen-space maps intentionally do not take the Scene
    /// view pointer: their cells belong to the Game viewport instead.
    fn tilemap_hover(
        &self,
        rect: Rect,
        pointer: Option<Pos2>,
        camera: CameraView,
    ) -> Option<TilemapHover> {
        self.tilemap_tool.brush()?;
        let pointer = pointer.filter(|pointer| rect.contains(*pointer))?;
        let entity = self.selection?;
        let data = self.world.get(entity)?;
        let payload = data.components.get(tilemap::TYPE_NAME)?;
        let map = tilemap::component(payload).ok()?;
        if map.space != SpriteSpace::World {
            return None;
        }
        let transform = data.transform_3d.unwrap_or_default();
        let aspect = rect.width() / rect.height().max(1.0);
        let camera = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()?;
        let normalized = [
            (pointer.x - rect.min.x) / rect.width().max(1.0),
            (pointer.y - rect.min.y) / rect.height().max(1.0),
        ];
        let (column, row) =
            tilemap::tile_at_viewport(&map, transform, camera.view_projection, normalized)?;
        let projected =
            tilemap::tile_outline(&map, transform, camera.view_projection, column, row)?;
        let outline = projected.map(|point| {
            Pos2::new(
                rect.min.x + point[0] * rect.width(),
                rect.min.y + point[1] * rect.height(),
            )
        });
        Some(TilemapHover {
            entity,
            column,
            row,
            outline,
        })
    }

    /// Resolves a Scene-view point through the exact camera that drew it.
    fn pick_viewport(
        &self,
        rect: Rect,
        pointer: Pos2,
        camera: CameraView,
    ) -> Result<Option<EntityId>, String> {
        if !rect.contains(pointer) {
            return Ok(None);
        }
        let aspect = rect.width() / rect.height().max(1.0);
        let Some(camera) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let point = [
            (pointer.x - rect.min.x) / rect.width().max(1.0),
            (pointer.y - rect.min.y) / rect.height().max(1.0),
        ];
        picking::pick_world(
            &self.world,
            self.scene.components(),
            camera.view_projection,
            point,
        )
        .map_err(|error| error.to_string())
    }

    /// Applies a primary Scene-view click without taking drags or paint strokes.
    fn select_viewport_click(
        &mut self,
        rect: Rect,
        response: &Response,
        camera: CameraView,
        painting: bool,
    ) {
        if painting || !response.clicked_by(egui::PointerButton::Primary) {
            return;
        }
        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };
        match self.pick_viewport(rect, pointer, camera) {
            Ok(entity) => self.select(entity),
            Err(error) => self
                .console
                .warning(format!("Viewport selection failed: {error}")),
        }
    }

    /// Writes one cell through the command layer. Repeated calls during one
    /// drag share a merge key, and pointer release closes that merge run.
    fn apply_tile_brush(&mut self, hover: &TilemapHover) {
        let Some(original) = self
            .world
            .get(hover.entity)
            .and_then(|data| data.components.get(tilemap::TYPE_NAME))
            .cloned()
        else {
            return;
        };
        let chosen = self.tilemap_tool.sprite.clone();
        let brush = if self.tilemap_tool.erase {
            TileBrush::Erase
        } else if let Some(chosen) = chosen.as_deref() {
            TileBrush::Sprite(chosen)
        } else {
            return;
        };
        let mut payload = original;
        match paint_tile(&mut payload, hover.column, hover.row, brush) {
            Ok(false) => return,
            Err(error) => {
                self.console.warning(error);
                return;
            }
            Ok(true) => {}
        }
        if let Err(error) = self
            .scene
            .components()
            .validate_payload(tilemap::TYPE_NAME, &payload)
        {
            self.console
                .warning(format!("Tilemap paint was refused: {error}"));
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity: hover.entity,
            type_name: tilemap::TYPE_NAME.to_owned(),
            payload,
        });
        let transaction = buffer
            .into_transaction("Paint tilemap")
            .merging(format!("tilemap:{}", hover.entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Resolves the selected transform into the paths used by both drawing and
    /// hit-testing. The visible handle is therefore the handle the pointer can
    /// actually take.
    fn gizmo_visual(
        &self,
        rect: Rect,
        camera: CameraView,
    ) -> Option<(ViewCamera, gizmo::GizmoVisual)> {
        let entity = self.selection?;
        let transform = self.world.get(entity)?.transform_3d?;
        let aspect = rect.width() / rect.height().max(1.0);
        let camera = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()?;
        let visual = gizmo::visual(
            self.gizmo_mode,
            transform,
            self.gizmo_space,
            camera.view_projection,
            GlamVec2::new(rect.width(), rect.height()),
            camera.framed_half_height,
        )?;
        Some((camera, visual))
    }

    /// Gives a transform handle first claim on primary drag and writes every
    /// intermediate answer through command history.
    fn interact_gizmo(
        &mut self,
        rect: Rect,
        response: &Response,
        camera: ViewCamera,
        visual: &gizmo::GizmoVisual,
    ) -> bool {
        let pointer = response
            .interact_pointer_pos()
            .map(|pointer| GlamVec2::new(pointer.x - rect.min.x, pointer.y - rect.min.y));
        let hovered = pointer.and_then(|pointer| gizmo::hit_test(visual, pointer));
        let owns_primary = self.gizmo_drag.is_some() || hovered.is_some();

        if response.drag_started_by(egui::PointerButton::Primary)
            && let (Some(entity), Some(axis), Some(pointer)) = (self.selection, hovered, pointer)
            && let Some(transform) = self.world.get(entity).and_then(|data| data.transform_3d)
        {
            self.gizmo_drag = gizmo::begin_drag(
                entity,
                self.gizmo_mode,
                axis,
                transform,
                self.gizmo_space,
                camera.view_projection,
                pointer,
                GlamVec2::new(rect.width(), rect.height()),
            );
        }

        if response.dragged_by(egui::PointerButton::Primary)
            && let (Some(drag), Some(pointer)) = (self.gizmo_drag, pointer)
            && let Some(next) = gizmo::update_drag(
                drag,
                camera.view_projection,
                pointer,
                GlamVec2::new(rect.width(), rect.height()),
                self.gizmo_snapping,
            )
        {
            self.apply_gizmo_transform(drag, next);
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.gizmo_drag = None;
        }
        owns_primary
    }

    /// A whole drag is one undo step even though its current answer is applied
    /// every frame, because all of its transactions share this merge key.
    fn apply_gizmo_transform(&mut self, drag: GizmoDrag, transform: Transform3D) {
        if self
            .world
            .get(drag.entity)
            .and_then(|data| data.transform_3d)
            == Some(transform)
        {
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetTransform3D {
            entity: drag.entity,
            transform: Some(transform),
        });
        let transaction = buffer
            .into_transaction(format!("{} entity", drag.mode.label()))
            .merging(format!(
                "gizmo:{}:{}",
                drag.entity.index(),
                drag.mode.label()
            ));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
            self.gizmo_drag = None;
        }
    }

    /// Changes the selection, ending any in-progress merge run so the next
    /// edit starts its own undo step.
    /// What to show the user, preferring what they just did over what the
    /// renderer is doing, since an action's failure is the newer news.
    fn problem(&self) -> Option<&str> {
        self.notice.as_deref().or(self.render_error.as_deref())
    }

    /// Records which scene to reopen on the next launch.
    ///
    /// A detached scene clears it rather than leaving the previous one: the
    /// editor should reopen where it was left, and it was left nowhere.
    fn remember_open_scene(&mut self) {
        self.preferences.last_scene = self.file.path().map(|path| path.display().to_string());
    }

    /// Names the open scene in the window title.
    ///
    /// The title is where an operating system shows what a window is for — in a
    /// task switcher, a dock, a window list — and "Sindri Editor" answers that
    /// with the name of the program. The file goes first because that is the
    /// part a switcher has room for, and the unsaved marker goes with it,
    /// matching the status bar rather than inventing a second vocabulary.
    fn update_title(&mut self, context: &egui::Context) {
        let title = format!(
            "{}{} - Sindri Editor",
            self.file.label(),
            if self.unsaved() { " (unsaved)" } else { "" }
        );
        if title == self.title {
            return;
        }
        context.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        self.title = title;
    }

    /// Says what a scene turned out to be once it was loaded.
    fn announce_scene(&mut self) {
        self.console.info(format!(
            "Opened {} - {} entities",
            self.file.label(),
            self.world.len()
        ));
    }

    /// Starts the open scene's textures loading from its own directory.
    ///
    /// A new scene resolves its references against a new directory, so the
    /// whole set is rebuilt rather than re-rooted. That is also how the previous
    /// scene's textures are released: the registry that owned them is dropped,
    /// and a `Texture2D` frees its GPU texture when it goes.
    fn reload_textures(&mut self) {
        let state = self.render_state.clone();
        self.renderers.text.clear_bindings();
        self.textures = SceneTextures::for_scene(&state.device, &state.queue, self.file.path());
        self.textured_revision = self.history.revision();
        let notes = self.textures.request(&self.world, &mut self.renderers.text);
        self.record_texture_notes(notes);
    }

    /// Asks again for whatever the world references, after an edit that could
    /// have changed it.
    fn refresh_textures(&mut self) {
        if self.textured_revision == self.history.revision() {
            return;
        }
        self.textured_revision = self.history.revision();
        let notes = self.textures.request(&self.world, &mut self.renderers.text);
        self.record_texture_notes(notes);
        self.refresh_scripts();
    }

    /// Asks again for whatever scripts the world names, after an edit that
    /// could have changed it.
    ///
    /// Shares `textured_revision` deliberately: both ask "has the world changed
    /// since we last looked", and two counters that must agree is one more
    /// thing to keep in step than there is any reason for.
    fn refresh_scripts(&mut self) {
        let notes = self.scripts.request(&self.world, self.scene.components());
        self.record_script_notes(notes);
    }

    /// Points the scripts at the open scene's directory and asks for its
    /// sources.
    ///
    /// Rebuilt rather than re-rooted for the same reason the textures are: a
    /// new scene resolves its references somewhere else, and the previous
    /// scene's sources and running instances go when the old one drops.
    fn reload_scripts(&mut self) {
        self.scripts = SceneScripts::for_scene(self.file.path());
        let notes = self.scripts.request(&self.world, self.scene.components());
        self.record_script_notes(notes);
    }

    /// Takes delivery of any script that arrived, then moves every script on by
    /// whatever this frame is worth.
    ///
    /// Called every frame, like the animations, and for the same reason: a
    /// script that will not compile should say so when the scene opens rather
    /// than waiting for someone to press Play. What the transport changes is
    /// how much time a frame is worth, so a scene at rest runs nothing.
    fn advance_scripts(&mut self, context: &egui::Context) {
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

    /// What to call an entity in a console line.
    fn entity_label(&self, entity: EntityId) -> String {
        self.world
            .get(entity)
            .and_then(|data| {
                data.name
                    .clone()
                    .or_else(|| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
            })
            .unwrap_or_else(|| format!("{entity:?}"))
    }

    fn select(&mut self, entity: Option<EntityId>) {
        if entity.is_some() {
            // One inspector, one subject. Selecting an entity puts the image
            // away rather than leaving it behind a panel showing something
            // else.
            self.slicer = None;
        }
        if self.selection != entity {
            self.history.break_merge_run();
            self.gizmo_drag = None;
            self.tilemap_tool.reset();
            self.animation_tool.reset();
            self.selection = entity;
        }
    }

    /// Shows an asset in the inspector, which for a texture means its slice.
    fn select_asset(&mut self, path: &Path) {
        if self.slicer.as_ref().is_some_and(|open| open.path() == path) {
            return;
        }
        self.slicer = Some(Slicer::open(path));
        self.selection = None;
        self.tilemap_tool.reset();
        self.animation_tool.reset();
    }

    /// The slicer, drawn on the image it is cutting.
    fn slicer_panel(&mut self, ui: &mut egui::Ui) {
        let Some(slicer) = &mut self.slicer else {
            return;
        };
        let mut save = false;
        let mut close = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    ICON_IMAGE
                        .outlined()
                        .rich_text()
                        .size(15.0)
                        .color(TEXT_FAINT),
                );
                ui.label(RichText::new(slicer.name()).size(12.0).color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
            let (width, height) = slicer.size();
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("{width} x {height}"))
                        .size(10.0)
                        .color(TEXT_MUTED),
                );
            });
            ui.add_space(6.0);

            let columns = slicer.columns;
            let rows = slicer.rows;
            slice_preview(ui, slicer);
            ui.add_space(8.0);

            section_header(ui, ICON_GRID_VIEW, "Slice");
            let mut columns_now = f64::from(columns);
            let mut rows_now = f64::from(rows);
            let mut resized = number_row(ui, "Columns", &mut columns_now, 10.0, true);
            resized |= number_row(ui, "Rows", &mut rows_now, 10.0, true);
            if resized {
                slicer.columns = grid_side(columns_now);
                slicer.rows = grid_side(rows_now);
                slicer.fit_names();
                slicer.clamp_selection();
            }

            let mut margin_x = f64::from(slicer.margin[0]);
            let mut margin_y = f64::from(slicer.margin[1]);
            let mut spacing_x = f64::from(slicer.spacing[0]);
            let mut spacing_y = f64::from(slicer.spacing[1]);
            let mut measured = number_row(ui, "Margin X", &mut margin_x, 10.0, true);
            measured |= number_row(ui, "Margin Y", &mut margin_y, 10.0, true);
            measured |= number_row(ui, "Spacing X", &mut spacing_x, 10.0, true);
            measured |= number_row(ui, "Spacing Y", &mut spacing_y, 10.0, true);
            if measured {
                slicer.margin = [pixel_count(margin_x), pixel_count(margin_y)];
                slicer.spacing = [pixel_count(spacing_x), pixel_count(spacing_y)];
            }

            ui.add_space(6.0);
            slice_names(ui, slicer);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                if ui.button("Save slice").clicked() {
                    save = true;
                }
            });
            if let Some(problem) = &slicer.problem {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new(problem).size(10.0).color(PROBLEM));
                });
            }
        });

        if save {
            let name = slicer.name();
            if slicer.save() {
                self.console.info(format!("Sliced {name}"));
                // The browser lists a texture's sprites from the sidecar, so a
                // save it did not notice would leave the new names invisible.
                self.refresh_project();
            } else if let Some(problem) = self.slicer.as_ref().and_then(|s| s.problem.clone()) {
                self.console.error(problem);
            }
        }
        if close {
            self.slicer = None;
        }
    }

    /// Creates an empty `GameObject`, optionally under another, and selects it.
    ///
    /// The handle is taken from the world *before* the command runs, so the
    /// command can be redone onto the same handle, and so there is something to
    /// select without asking the world what just appeared.
    fn create_entity(&mut self, parent: Option<EntityId>) {
        let entity = self.world.next_handle();
        let source_id = next_game_object_id(&self.world);
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Spawn {
            entity,
            data: Box::new(EntityData {
                source_id: Some(source_id),
                name: Some("GameObject".to_owned()),
                parent,
                transform_3d: Some(Transform3D::default()),
                ..EntityData::default()
            }),
        });
        self.history.break_merge_run();
        if let Err(error) = self.history.apply(
            buffer.into_transaction(if parent.is_some() {
                "Create child"
            } else {
                "Create GameObject"
            }),
            &mut self.world,
        ) {
            self.report(error.to_string());
            return;
        }
        // Selected, because making something and then having to find it is the
        // kind of small friction that makes a tool tiring to use.
        self.select(Some(entity));
        if let Some(parent) = parent
            && let Some(key) = hierarchy_preference_key(self.file.path(), &self.world, parent)
        {
            self.preferences.collapsed_hierarchy.remove(&key);
        }
    }

    /// Deletes an entity and everything under it.
    ///
    /// The selection is cleared rather than moved to the parent: after a
    /// delete, nothing is what is selected, and guessing otherwise risks an
    /// edit meant for the deleted thing landing somewhere else.
    ///
    /// Undo brings it back at the same handle, so this is not the one-way door
    /// a delete usually is — see [`sindri_core::World::spawn_at`].
    fn delete_entity(&mut self, entity: EntityId) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Despawn { entity });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Delete entity"), &mut self.world)
        {
            self.report(error.to_string());
            return;
        }
        self.select(None);
        self.refresh_textures();
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
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Reparent entity"), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    fn undo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.undo(&mut self.world) {
            self.report(error.to_string());
        }
    }

    fn redo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.redo(&mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Rebuilds the runtime scene from the authored document.
    ///
    /// Every runtime handle is replaced, so recorded history is discarded
    /// rather than left pointing at entities that no longer exist.
    fn reset_to_authored(&mut self) {
        match load_world(&self.scene, self.file.document()) {
            Ok(world) => {
                self.world = world;
                self.history.clear();
                self.saved_revision = self.history.revision();
                self.selection = None;
                self.tilemap_tool.reset();
                self.animation_tool.reset();
                self.lifecycle = initialized_lifecycle();
                // A cursor belongs to the world it was advanced against, and
                // a freshly loaded world reuses entity slots from the start.
                self.animations = SpriteAnimations::new();
                self.play_snapshot = None;
                self.notice = None;
                self.announce_scene();
                self.reload_textures();
                self.reload_scripts();
            }
            Err(error) => self.report(error),
        }
    }

    /// Play and pause move the engine lifecycle rather than a display flag, so
    /// the editor exercises the same transitions a runtime host does.
    fn toggle_playback(&mut self) {
        let result = match self.lifecycle.state() {
            EngineState::Running => self.lifecycle.pause(),
            EngineState::Paused => self.lifecycle.resume(),
            _ => {
                // Taken before the first frame runs, and only on a fresh start
                // rather than on resume, so pausing and carrying on does not
                // move the point stop returns to.
                self.play_snapshot = Some(self.world.clone());
                self.lifecycle.start()
            }
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
    fn stop_playback(&mut self) {
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
    fn advance_animations(&mut self, context: &egui::Context) {
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

    fn pause(&mut self) {
        if self.lifecycle.state() == EngineState::Running
            && let Err(error) = self.lifecycle.pause()
        {
            self.report(error.to_string());
        }
    }

    /// Catches the window's close button while there is unsaved work.
    ///
    /// The close is cancelled and the question asked; answering it either lets
    /// the next request through or leaves the editor open. Without this, the
    /// most ordinary way to leave the editor is also the one way to lose an
    /// afternoon without being asked.
    fn handle_close_request(&mut self, context: &egui::Context) {
        if !context.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.closing || !self.unsaved() {
            return;
        }
        context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.confirming = Some(Discarding::Close);
    }

    /// Asks before throwing work away, and reports whether it is asking.
    ///
    /// Returns `true` while the question is on screen, so the frame's remaining
    /// input handling stands down rather than acting on keys aimed at the
    /// dialog.
    fn confirm_dialog(&mut self, context: &egui::Context) -> bool {
        let Some(action) = self.confirming.clone() else {
            return false;
        };
        let saveable = self.file.path().is_some();
        let mut answered = None;
        egui::Modal::new(egui::Id::new("sindri-discard-confirm")).show(context, |ui| {
            ui.set_width(360.0);
            ui.label(
                RichText::new("Unsaved changes")
                    .strong()
                    .size(13.0)
                    .color(TEXT),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(action.question())
                    .size(12.0)
                    .color(TEXT_MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    answered = Some(Answer::Cancel);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(action.verb()).color(ACCENT_BRIGHT))
                        .clicked()
                    {
                        answered = Some(Answer::Discard);
                    }
                    if ui
                        .add_enabled(saveable, egui::Button::new("Save first"))
                        .clicked()
                    {
                        answered = Some(Answer::Save);
                    }
                });
            });
        });
        match answered {
            None => {}
            Some(Answer::Cancel) => self.confirming = None,
            Some(Answer::Discard) => self.discard(action, context),
            Some(Answer::Save) => {
                self.save();
                // A failed save leaves the question standing rather than
                // discarding the work it could not write.
                if !self.unsaved() {
                    self.discard(action, context);
                }
            }
        }
        self.confirming.is_some()
    }

    fn handle_shortcuts(&mut self, context: &egui::Context) {
        let pressed = context.input_mut(shortcuts);
        if pressed.save {
            self.save();
        }
        if pressed.redo {
            self.redo();
        } else if pressed.undo {
            self.undo();
        }
        if pressed.focus {
            self.focus_selection();
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
                    self.edit_menu(ui);
                    self.view_menu(ui);
                    // "Scene", "Build", "Tools", and "Help" used to sit here.
                    // None of them opened: they were labels shaped like menus,
                    // which is a promise about four features that do not exist.
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
                    // Stop stops. It used to reset the scene to the file,
                    // which is what the symbol between Pause and Play means to
                    // nobody, and it did that without asking. Going back to
                    // the authored scene is File → Discard changes, which now
                    // says what it will cost.
                    let playing = matches!(
                        self.lifecycle.state(),
                        EngineState::Running | EngineState::Paused
                    );
                    if transport_icon(ui, ICON_STOP, false, playing, "Stop").clicked() {
                        self.stop_playback();
                    }
                    if transport_icon(ui, ICON_PAUSE, !running, running, "Pause").clicked() {
                        self.pause();
                    }
                    if transport_icon(ui, ICON_PLAY_ARROW, running, true, "Play").clicked()
                        || play_button(ui, running).clicked()
                    {
                        self.toggle_playback();
                    }
                    // A project name and a chevron used to sit at this end,
                    // naming a project that did not exist and opening nothing.
                    // What project is open is the browser's business, and it
                    // says so from the directory it is reading.
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
                panel_title(ui, "Hierarchy");
                search_field(ui, &mut self.search, "Search");
                let (create, deleted) = self.hierarchy_toolbar(ui);
                ui.add_space(6.0);
                self.hierarchy_contents(ui);
                if let Some(create) = create {
                    self.create_entity(match create {
                        CreateGameObject::Root => None,
                        CreateGameObject::Child(parent) => Some(parent),
                    });
                }
                if let Some(entity) = deleted {
                    self.delete_entity(entity);
                }
            });
    }

    fn hierarchy_toolbar(&self, ui: &mut egui::Ui) -> (Option<CreateGameObject>, Option<EntityId>) {
        let mut create = None;
        let mut deleted = None;
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.menu_button(ICON_ADD.outlined().rich_text().size(14.0), |ui| {
                if ui.button("Create Empty").clicked() {
                    create = Some(CreateGameObject::Root);
                    ui.close();
                }
                if ui
                    .add_enabled(self.selection.is_some(), egui::Button::new("Create Child"))
                    .clicked()
                {
                    create = self.selection.map(CreateGameObject::Child);
                    ui.close();
                }
            })
            .response
            .on_hover_text("Create GameObject");
            // Offered only with something selected, because "delete" with
            // nothing chosen has no answer and a disabled button is a question
            // nobody asked.
            if let Some(entity) = self.selection
                && ui
                    .small_button(ICON_DELETE.outlined().rich_text().size(14.0))
                    .on_hover_text("Delete entity")
                    .clicked()
            {
                deleted = Some(entity);
            }
        });
        (create, deleted)
    }

    fn hierarchy_contents(&mut self, ui: &mut egui::Ui) {
        let mut reparenting = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let root = hierarchy_group(ui, "World", ICON_ACCOUNT_TREE);
                if let Some(entity) = hierarchy_drop_target(ui, &root, &self.world, None) {
                    reparenting = Some((entity, None));
                }
                let needle = self.search.trim().to_lowercase();
                let collapsed: BTreeSet<EntityId> = self
                    .world
                    .entities()
                    .filter_map(|(entity, _)| {
                        hierarchy_preference_key(self.file.path(), &self.world, entity)
                            .filter(|key| self.preferences.collapsed_hierarchy.contains(key))
                            .map(|_| entity)
                    })
                    .collect();
                let mut clicked: Option<Option<EntityId>> = None;
                let mut toggled = None;
                for (entity, depth) in visible_hierarchy_rows(&self.world, &collapsed, &needle) {
                    let Some(data) = self.world.get(entity) else {
                        continue;
                    };
                    let name = entity_name(data);
                    let row = hierarchy_row(
                        ui,
                        entity_icon(data),
                        &name,
                        self.selection == Some(entity),
                        depth + 1,
                        !data.children.is_empty(),
                        !collapsed.contains(&entity) || !needle.is_empty(),
                    );
                    row.select.dnd_set_drag_payload(HierarchyDrag(entity));
                    if row.select.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if let Some(dragged) =
                        hierarchy_drop_target(ui, &row.drop, &self.world, Some(entity))
                    {
                        reparenting = Some((dragged, Some(entity)));
                    }
                    if row.toggle.is_some_and(|response| response.clicked()) {
                        toggled = Some(entity);
                    } else if row.select.clicked() {
                        clicked = Some(Some(entity));
                    }
                }
                if let Some(entity) = toggled
                    && let Some(key) =
                        hierarchy_preference_key(self.file.path(), &self.world, entity)
                    && !self.preferences.collapsed_hierarchy.remove(&key)
                {
                    self.preferences.collapsed_hierarchy.insert(key);
                }
                // Clicking past the last row clears the selection. Without
                // somewhere to click that means "nothing", a selection made by
                // accident can only be replaced.
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
        if let Some((entity, parent)) = reparenting {
            self.reparent(entity, parent);
            self.select(Some(entity));
            if let Some(parent) = parent
                && let Some(key) = hierarchy_preference_key(self.file.path(), &self.world, parent)
            {
                self.preferences.collapsed_hierarchy.remove(&key);
            }
        }
    }

    fn asset_panel(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let mut action = BrowserAction::None;
        let mut clear_console = false;
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
                    BottomTab::Project => {
                        action = project_browser(
                            ui,
                            &mut self.asset_search,
                            &mut self.preferences.asset_view,
                            &mut self.expanded_sheets,
                            folders,
                            &self.project,
                            self.file.path(),
                        );
                    }
                    BottomTab::Console => {
                        if console_view(ui, &self.console, self.lifecycle.state()) {
                            clear_console = true;
                        }
                    }
                }
            });
        if clear_console {
            self.console.clear();
        }
        // Acted on outside the panel, because both answers write to the field
        // the browser was reading from.
        match action {
            BrowserAction::None => {}
            BrowserAction::Refresh => self.refresh_project(),
            BrowserAction::Open(path) => {
                self.discard_or_confirm(Discarding::OpenPath(path), &context);
            }
            BrowserAction::Select(path) => self.select_asset(&path),
        }
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
                self.discard_or_confirm(Discarding::OpenAnother, ui.ctx());
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
                self.discard_or_confirm(Discarding::Reload, ui.ctx());
                ui.close();
            }
            ui.separator();
            if ui.button("Discard changes").clicked() {
                self.discard_or_confirm(Discarding::Reset, ui.ctx());
                ui.close();
            }
        });
    }

    /// Undo and redo, in the menu people look in for them.
    ///
    /// The same two actions as the toolbar icons and the keyboard, labelled
    /// with what they would undo, which is the thing a menu can say and an icon
    /// cannot. "Edit" was a label shaped like a menu until this.
    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("Edit").size(12.0).color(TEXT_MUTED), |ui| {
            ui.set_min_width(190.0);
            let undo = self.history.undo_label().map_or_else(
                || "Undo".to_owned(),
                |label| format!("Undo {}", label.to_lowercase()),
            );
            if ui
                .add_enabled(
                    self.history.can_undo(),
                    egui::Button::new(undo).shortcut_text("Ctrl+Z"),
                )
                .clicked()
            {
                self.undo();
                ui.close();
            }
            let redo = self.history.redo_label().map_or_else(
                || "Redo".to_owned(),
                |label| format!("Redo {}", label.to_lowercase()),
            );
            if ui
                .add_enabled(
                    self.history.can_redo(),
                    egui::Button::new(redo).shortcut_text("Ctrl+Shift+Z"),
                )
                .clicked()
            {
                self.redo();
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
                        // Not "the renderer reported an error": what went wrong
                        // is as likely to be a file that would not open, and
                        // the notice beside the viewport says which.
                        RichText::new(if healthy {
                            "Renderer ready"
                        } else {
                            "Something went wrong"
                        })
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("|").size(11.0).color(BORDER));
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(if self.unsaved() {
                            format!("{} (unsaved)", self.file.label())
                        } else {
                            self.file.label()
                        })
                        .size(11.0)
                        .color(if self.unsaved() {
                            ACCENT
                        } else {
                            TEXT_MUTED
                        }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        // Counted rather than guessed from whether a notice is
                        // showing, which is what this used to do: it said "1
                        // Error" for anything at all and never mentioned a
                        // warning, because nothing in the editor could produce
                        // one.
                        let counts = self.console.counts();
                        ui.label(RichText::new(counts.summary()).size(11.0).color(
                            if counts.errors > 0 {
                                ACCENT_BRIGHT
                            } else {
                                TEXT_FAINT
                            },
                        ));
                    });
                });
            });
    }

    fn transform_tools(&mut self, ui: &mut egui::Ui) {
        let shortcut_mode = ui.input_mut(|input| {
            [
                (egui::Key::Q, GizmoMode::Select),
                (egui::Key::W, GizmoMode::Translate),
                (egui::Key::E, GizmoMode::Rotate),
                (egui::Key::R, GizmoMode::Scale),
            ]
            .into_iter()
            .find_map(|(key, mode)| {
                input
                    .consume_key(egui::Modifiers::NONE, key)
                    .then_some(mode)
            })
        });
        if let Some(mode) = shortcut_mode {
            self.gizmo_mode = mode;
            self.gizmo_drag = None;
            self.history.break_merge_run();
        }
        for (mode, icon, key) in [
            (GizmoMode::Select, ICON_SELECT, "Q"),
            (GizmoMode::Translate, ICON_MOVE, "W"),
            (GizmoMode::Rotate, ICON_ROTATE_RIGHT, "E"),
            (GizmoMode::Scale, ICON_SCALE, "R"),
        ] {
            if icon_button(
                ui,
                icon,
                self.gizmo_mode == mode,
                &format!("{} ({key})", mode.label()),
            )
            .clicked()
            {
                self.gizmo_mode = mode;
                self.gizmo_drag = None;
                self.history.break_merge_run();
            }
        }
        if ui
            .add_sized(
                [48.0, 28.0],
                egui::Button::new(
                    RichText::new(match self.gizmo_space {
                        GizmoSpace::World => "World",
                        GizmoSpace::Local => "Local",
                    })
                    .size(10.0)
                    .color(TEXT_MUTED),
                )
                .fill(PANEL_RAISED)
                .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .on_hover_text("Toggle world/local movement and rotation axes")
            .clicked()
        {
            self.gizmo_space = match self.gizmo_space {
                GizmoSpace::World => GizmoSpace::Local,
                GizmoSpace::Local => GizmoSpace::World,
            };
            self.gizmo_drag = None;
        }
        let snap_tip = format!(
            "Snap: {} units · {}° · {} scale",
            self.gizmo_snapping.translation,
            self.gizmo_snapping.rotation_degrees,
            self.gizmo_snapping.scale
        );
        if icon_button(ui, ICON_GRID_4X4, self.gizmo_snapping.enabled, &snap_tip).clicked() {
            self.gizmo_snapping.enabled = !self.gizmo_snapping.enabled;
        }
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
                    self.transform_tools(ui);
                    // Panning can carry the subject off screen entirely, so
                    // the way back is a control rather than a remembered
                    // number.
                    if icon_button(ui, ICON_CAMERA_ALT, self.view_moved(), "Reset view").clicked() {
                        self.reset_view();
                    }
                    if ui
                        .add_enabled_ui(self.selection.is_some(), |ui| {
                            icon_button(ui, ICON_CENTER_FOCUS_STRONG, false, "Focus selection (F)")
                        })
                        .inner
                        .clicked()
                    {
                        self.focus_selection();
                    }
                });
            });
        });
    }

    /// Draws the cell a tilemap stroke would edit without changing the scene.
    fn paint_tilemap_hover(&self, ui: &egui::Ui, hover: &TilemapHover) {
        let fill = if self.tilemap_tool.erase {
            Color32::from_rgba_unmultiplied(255, 138, 148, 35)
        } else {
            Color32::from_rgba_unmultiplied(246, 169, 35, 35)
        };
        let stroke = Stroke::new(
            2.0,
            if self.tilemap_tool.erase {
                PROBLEM
            } else {
                ACCENT_BRIGHT
            },
        );
        ui.painter()
            .add(Shape::convex_polygon(hover.outline.to_vec(), fill, stroke));
        ui.painter().text(
            hover.outline[0],
            Align2::LEFT_BOTTOM,
            format!("{}, {}", hover.column, hover.row),
            FontId::proportional(10.0),
            TEXT,
        );
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
        let painting = editing && self.tilemap_tool.brush().is_some();
        let camera_before_input = self.scene_camera();
        let gizmo_owned = if editing && !painting {
            self.gizmo_visual(rect, camera_before_input)
                .is_some_and(|(camera, visual)| {
                    self.interact_gizmo(rect, &response, camera, &visual)
                })
        } else {
            false
        };
        if editing {
            self.move_camera(&context, &response, rect.height(), painting || gizmo_owned);
        }
        let scale = context.pixels_per_point();
        let camera = if editing {
            self.scene_camera()
        } else {
            camera_for(tab, EditorCamera::default())
        };
        let hover = editing
            .then(|| self.tilemap_hover(rect, response.hover_pos(), camera))
            .flatten();
        if let Some(hover) = &hover
            && (response.clicked_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Primary))
        {
            self.apply_tile_brush(hover);
        }
        if editing {
            self.select_viewport_click(rect, &response, camera, painting || gizmo_owned);
        }
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
                    world: &self.world,
                    animations: &self.animations,
                    textures: &self.textures,
                },
                (
                    physical_viewport_dimension(rect.width(), scale),
                    physical_viewport_dimension(rect.height(), scale),
                ),
                camera,
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
            if let Some(hover) = &hover {
                self.paint_tilemap_hover(ui, hover);
            }
            if !painting && let Some((_, visual)) = self.gizmo_visual(rect, camera) {
                paint_transform_gizmo(
                    ui.painter(),
                    rect,
                    &visual,
                    self.gizmo_drag.map(|drag| drag.axis),
                );
            }
            // The same view the frame under it was drawn through, asked for
            // rather than re-derived, so the axes cannot drift from the picture.
            let axes = self
                .scene
                .world_camera(&self.world, camera)
                .ok()
                .flatten()
                .map(|camera| camera.view);
            paint_runtime_overlay(
                ui.painter(),
                rect,
                &self
                    .selection
                    .and_then(|entity| self.world.get(entity))
                    .map_or_else(|| "No selection".to_owned(), entity_name),
                self.problem(),
                axes,
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
        // Before anything is drawn, so a texture that arrived since the last
        // frame is bound by the time this one extracts.
        self.refresh_textures();
        let state = self.render_state.clone();
        let arrived = self
            .textures
            .poll(&state.device, &state.queue, &mut self.renderers.text);
        self.record_texture_notes(arrived);
        self.advance_animations(ui.ctx());
        self.advance_scripts(ui.ctx());
        self.update_title(ui.ctx());
        self.handle_close_request(ui.ctx());
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
        // Drawn last so it sits over everything, and asked before Escape is
        // read as clearing the selection.
        if self.confirm_dialog(ui.ctx()) {
            return;
        }
        // Escape clears the selection wherever the pointer happens to be. The
        // hierarchy's empty space does the same, but only while it has empty
        // space to click.
        if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
            self.select(None);
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

/// The root the hierarchy hangs from.
///
/// A collapse chevron used to sit in front of it, and nothing collapsed.
fn hierarchy_group(ui: &mut egui::Ui, label: &str, icon: MaterialIcon) -> Response {
    let width = ui.available_width();
    let row = ui.scope_builder(egui::UiBuilder::new().sense(Sense::hover()), |ui| {
        ui.set_min_width(width);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let icon = ui.add(
                egui::Label::new(icon.outlined().rich_text().size(15.0).color(TEXT_MUTED))
                    .sense(Sense::hover()),
            );
            let label = ui.add(
                egui::Label::new(RichText::new(label).size(12.0).color(TEXT)).sense(Sense::hover()),
            );
            icon | label
        })
        .inner
    });
    row.response | row.inner
}

/// The two independent actions a hierarchy row can report.
struct HierarchyRowResponse {
    select: Response,
    drop: Response,
    toggle: Option<Response>,
}

#[derive(Clone, Copy)]
enum CreateGameObject {
    Root,
    Child(EntityId),
}

/// The hierarchy owns its payload type so future drag-and-drop tools cannot be
/// mistaken for an entity move merely because they also carry an `EntityId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HierarchyDrag(EntityId);

/// One row of the hierarchy, reporting selection separately from folding.
///
/// The response has to be the button's, not the layout's. `ui.horizontal`
/// allocates its region with `Sense::hover`, so asking that value whether it was
/// clicked answers no forever — which is what it did from the first editor
/// commit until this was found by driving the editor rather than reading it. The
/// whole of selection, and therefore every edit the editor can make, hung on
/// this one word.
///
/// The row's rect is re-sensed as well, so the icon and the padding beside the
/// name select too. A row that answers only on its text is the same complaint in
/// miniature.
fn hierarchy_row(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    selected: bool,
    depth: usize,
    has_children: bool,
    expanded: bool,
) -> HierarchyRowResponse {
    let width = ui.available_width();
    let builder = egui::UiBuilder::new().sense(Sense::click_and_drag());
    let row = ui.scope_builder(builder, |ui| {
        ui.set_min_width(width);
        ui.horizontal(|ui| {
            ui.add_space(9.0 + hierarchy_indent(depth, 14.0));
            let toggle = if has_children {
                Some(
                    ui.add(
                        egui::Button::new(
                            if expanded {
                                ICON_KEYBOARD_ARROW_DOWN
                            } else {
                                ICON_KEYBOARD_ARROW_RIGHT
                            }
                            .outlined()
                            .rich_text()
                            .size(15.0)
                            .color(TEXT_MUTED),
                        )
                        .frame(false)
                        .min_size(Vec2::new(16.0, 18.0)),
                    )
                    .on_hover_text(if expanded {
                        "Collapse children"
                    } else {
                        "Expand children"
                    }),
                )
            } else {
                ui.add_space(16.0);
                None
            };
            // The icon senses clicks so that it does not swallow them: a
            // widget inside the scope takes precedence over the scope's own
            // sense, so a hover-only label would be a dead patch in the middle
            // of the row.
            let icon = ui.add(
                egui::Label::new(icon.outlined().rich_text().size(15.0).color(if selected {
                    ACCENT_BRIGHT
                } else {
                    TEXT_MUTED
                }))
                .sense(Sense::click_and_drag()),
            );
            let label = ui.add(
                egui::Button::new(RichText::new(name).size(12.0).color(if selected {
                    TEXT
                } else {
                    TEXT_MUTED
                }))
                .selected(selected)
                .sense(Sense::click_and_drag())
                .frame(false),
            );
            (icon | label, toggle)
        })
        .inner
    });
    // A scope's sense sits below the widgets inside it, so the name still
    // answers for itself and the rest of the row answers for the scope.
    let select = row.response | row.inner.0;
    let toggle = row.inner.1;
    let drop = toggle
        .clone()
        .map_or_else(|| select.clone(), |toggle| select.clone() | toggle);
    HierarchyRowResponse {
        select,
        drop,
        toggle,
    }
}

/// Draws feedback for a hierarchy drop and returns a legal released payload.
fn hierarchy_drop_target(
    ui: &egui::Ui,
    response: &Response,
    world: &World,
    parent: Option<EntityId>,
) -> Option<EntityId> {
    let dragged = response.dnd_hover_payload::<HierarchyDrag>()?;
    let allowed = hierarchy_drop_allowed(world, dragged.0, parent);
    let colour = if allowed { ACCENT } else { PROBLEM };
    ui.painter().rect_stroke(
        response.rect,
        2.0,
        Stroke::new(1.5, colour),
        StrokeKind::Inside,
    );
    ui.ctx().set_cursor_icon(if allowed {
        egui::CursorIcon::Grabbing
    } else {
        egui::CursorIcon::NotAllowed
    });
    if allowed {
        response
            .dnd_release_payload::<HierarchyDrag>()
            .map(|dragged| dragged.0)
    } else {
        None
    }
}

fn hierarchy_drop_allowed(world: &World, entity: EntityId, parent: Option<EntityId>) -> bool {
    world.get(entity).is_some_and(|data| data.parent != parent)
        && world.check_set_parent(entity, parent).is_ok()
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

/// The heading above one section of the inspector.
///
/// A collapse chevron and an overflow menu used to sit at either end of it.
/// Neither was handled: nothing collapsed and nothing overflowed. Adding and
/// removing a component is what the menu would hold, and that is a real build
/// against the schema registry rather than a glyph.
/// An image dimension as a length to lay out with.
///
/// No image this can draw is anywhere near the width an `f32` stops counting
/// exactly, and one that were would not fit in a panel either.
#[allow(clippy::cast_precision_loss)]
fn pixels(value: u32) -> f32 {
    value as f32
}

/// A pixel measurement, as a drag leaves it.
///
/// Clamped to something an image could plausibly carry, for the reason a grid
/// side is: a drag that got away should not become a slice with no cells.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_count(value: f64) -> u32 {
    value.clamp(0.0, 4096.0) as u32
}

/// One side of a slicing grid, as a drag leaves it.
///
/// Clamped rather than validated after the fact: a grid of zero has no cells
/// and one of ten thousand is a drag that got away, and neither is a slice
/// anybody meant. The cast cannot lose anything once the value is inside that
/// range.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn grid_side(value: f64) -> u32 {
    value.clamp(1.0, 256.0) as u32
}

/// Naming the chosen cell, and a list of the ones already named.
///
/// A field per cell is fine at four and unusable at two hundred and fifty-six,
/// so the sheet is named the way it is looked at: pick a cell on the image, give
/// it a name. Everything unnamed already has an answer — its index — so a list
/// of the named ones is the whole of what there is to review.
fn slice_names(ui: &mut egui::Ui, slicer: &mut Slicer) {
    section_header(ui, ICON_LABEL, "Names");
    slicer.fit_names();
    slicer.clamp_selection();

    let selected = slicer.selected;
    let placeholder = selected.to_string();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!("Cell {selected}"))
                .size(11.0)
                .color(TEXT),
        );
    });
    if let Some(name) = slicer.names.get_mut(selected as usize) {
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text(&placeholder)
                    .desired_width(f32::INFINITY),
            );
        });
    }
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(
            RichText::new(
                "Click a cell on the image to name it. A cell left blank is called by its index.",
            )
            .size(9.0)
            .color(TEXT_MUTED),
        );
    });

    let named = slicer.named();
    if named.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!("{} named", named.len()))
                .size(9.0)
                .color(TEXT_FAINT),
        );
    });
    let mut jump = None;
    for (index, name) in named {
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let row = ui.add(
                egui::Label::new(
                    RichText::new(format!("{index:>4}  {name}"))
                        .size(10.0)
                        .color(if index == selected {
                            ACCENT
                        } else {
                            TEXT_MUTED
                        }),
                )
                .selectable(false)
                .sense(Sense::click()),
            );
            if row.clicked() {
                jump = Some(index);
            }
        });
    }
    // The list is also how a cell is found again on a sheet too large to scan.
    if let Some(jump) = jump {
        slicer.selected = jump;
    }
}

/// The image, with the slice drawn over it, and a click choosing a cell.
///
/// The whole point of doing this on the picture: a grid of numbers in a panel
/// tells you nothing about whether the cells fall on the frames, and the cells
/// falling on the frames is the entire job. The rects drawn are the ones the
/// document produces, not a second calculation that could disagree with it.
fn slice_preview(ui: &mut egui::Ui, slicer: &mut Slicer) {
    let (width, height) = slicer.size();
    let rects = slicer.cell_rects();
    let selected = slicer.selected;
    let Some(texture) = slicer.texture(ui.ctx()) else {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("No preview: this build cannot read the image")
                    .size(10.0)
                    .color(TEXT_MUTED),
            );
        });
        return;
    };
    if width == 0 || height == 0 {
        return;
    }

    // Fitted to the panel and never enlarged past a readable multiple: a 16x16
    // sheet blown up to panel width is mostly interpolation artefacts, and a
    // 2048px one has to come down.
    let available = (ui.available_width() - 20.0).max(64.0);
    let (wide, tall) = (pixels(width), pixels(height));
    let scale = (available / wide).min(8.0);
    let size = Vec2::new(wide * scale, tall * scale);
    let texture = texture.id();

    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        // A flat ground behind the image, so a transparent sheet reads as
        // transparent rather than as the panel's own background.
        painter.rect_filled(rect, 2.0, Color32::from_rgb(28, 32, 42));
        painter.image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        // Each cell as its own outline rather than lines across the image,
        // because a cell inset by a margin is not on any dividing line and
        // drawing one would say the gutters belong to a sprite.
        let cell_rect = |[x, y, w, h]: [f32; 4]| {
            egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + x * rect.width(),
                    rect.top() + y * rect.height(),
                ),
                Vec2::new(w * rect.width(), h * rect.height()),
            )
        };
        let faint = Stroke::new(1.0, ACCENT.gamma_multiply(0.55));
        for (index, cell) in rects.iter().enumerate() {
            if u32::try_from(index).is_ok_and(|index| index == selected) {
                continue;
            }
            painter.rect_stroke(cell_rect(*cell), 0.0, faint, egui::StrokeKind::Inside);
        }
        if let Some(cell) = rects.get(selected as usize) {
            let bright = Stroke::new(2.0, ACCENT_BRIGHT);
            painter.rect_stroke(cell_rect(*cell), 0.0, bright, egui::StrokeKind::Inside);
        }

        // Picked by hit-testing the drawn rects rather than by dividing the
        // pointer's position, so a click lands on the cell it looks like it
        // landed on even when gutters mean the cells do not tile.
        if let Some(pointer) = response.interact_pointer_pos()
            && response.clicked()
        {
            picked = rects
                .iter()
                .position(|cell| cell_rect(*cell).contains(pointer))
                .and_then(|index| u32::try_from(index).ok());
        }
    });
    if let Some(picked) = picked {
        slicer.selected = picked;
    }
}

/// What the project browser shows, until it reads a real asset directory.
///
/// Each entry carries its kind as well as its name, because a list has room to
/// say what a thing is and a grid of generic icons does not.
/// What a frame of the project browser asked for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum BrowserAction {
    #[default]
    None,
    /// Re-read the directory, because the editor caches it.
    Refresh,
    /// Open a scene the browser is showing.
    Open(PathBuf),
    /// Show an asset in the inspector, which for a texture means its slice.
    Select(PathBuf),
}

/// The icon a kind of file is drawn with.
const fn asset_icon(kind: AssetKind) -> MaterialIcon {
    match kind {
        AssetKind::Folder => ICON_FOLDER,
        AssetKind::Scene => ICON_DESCRIPTION,
        AssetKind::Texture | AssetKind::Font => ICON_IMAGE,
        // A sprite and the sheet that cuts it are both about a grid over an
        // image, and neither is the image.
        AssetKind::Sprite | AssetKind::Sheet => ICON_GRID_VIEW,
        AssetKind::Mesh => ICON_VIEW_IN_AR,
        AssetKind::Script => ICON_CODE,
        AssetKind::Audio => ICON_PLAY_ARROW,
        AssetKind::Other => ICON_DEPLOYED_CODE,
    }
}

/// The project browser, in one column or two.
///
/// Two panes need width the bottom dock has and a side column does not: at
/// column width the folder tree and the asset list were drawing over each
/// other. So the narrow arrangement drops the tree rather than shrinking it,
/// which is also why a list reads better there than a grid of identical icons.
fn project_browser(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    expanded: &mut BTreeSet<PathBuf>,
    folders: bool,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    if !folders {
        return asset_column(ui, search, view, expanded, project, open);
    }
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(174.0);
            folder_row(ui, &project.label(), true, 0);
            for folder in project.folders() {
                folder_row(ui, &folder.name, false, folder.depth + 1);
            }
        });
        ui.separator();
        ui.vertical(|ui| action = asset_column(ui, search, view, expanded, project, open));
    });
    action
}

/// The asset side of the browser: what it is showing, and how.
fn asset_column(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
    expanded: &mut BTreeSet<PathBuf>,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    let mut action = BrowserAction::None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(project.label()).size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, ICON_VIEW_LIST, *view == AssetView::List, "List view").clicked() {
                *view = AssetView::List;
            }
            if icon_button(ui, ICON_GRID_VIEW, *view == AssetView::Grid, "Grid view").clicked() {
                *view = AssetView::Grid;
            }
            // The directory is read when a scene is opened, so a file added
            // outside the editor needs asking for. This slot used to hold a
            // filter icon that did nothing.
            if icon_button(ui, ICON_REFRESH, false, "Re-read the project directory").clicked() {
                action = BrowserAction::Refresh;
            }
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
    if let Some(error) = project.error() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(error).size(11.0).color(TEXT_FAINT));
        });
        return action;
    }
    let searching = !search.trim().is_empty();
    let entries = project.matching(search);
    if entries.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let message = if searching {
                "Nothing matches"
            } else {
                "This directory is empty"
            };
            ui.label(RichText::new(message).size(11.0).color(TEXT_FAINT));
        });
        return action;
    }
    // A project has more assets than a dock has room for, in either
    // presentation. Scrolling here is what lets the list be the default
    // without the last few assets falling off the bottom.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match view {
                AssetView::Grid => {
                    ui.horizontal_wrapped(|ui| {
                        for entry in &entries {
                            if asset_tile(ui, entry, open).double_clicked() {
                                action = BrowserAction::Open(entry.path.clone());
                            }
                        }
                    });
                }
                AssetView::List => {
                    // Rows are denser than egui's default spacing, so the dock
                    // shows a useful number of them without taking height from
                    // the viewport it sits under.
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for entry in &entries {
                        // A search shows a flat list, so an indentation would
                        // point at a parent the search has removed.
                        let depth = if searching { 0 } else { entry.depth };
                        // A sliced image's parts sit under it, because that is
                        // where a person looks for them: they belong to the
                        // image, not to the directory. Collapsed until asked
                        // for, because a sheet is as likely to hold sixty-four
                        // frames as four, and a browser that cannot be scrolled
                        // past is the failure the hierarchy already taught us.
                        if let Some(chosen) =
                            sliceable_row(ui, entry, depth, searching, open, expanded)
                        {
                            action = chosen;
                        }
                    }
                }
            }
            if project.truncated() {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("More files than the browser reads")
                            .size(10.0)
                            .color(TEXT_FAINT),
                    );
                });
            }
        });
    action
}

/// One asset as a row: what it is called, what it is, and whether it is the
/// scene the editor has open.
///
/// A scene row answers a double click, because opening one is the only thing
/// the editor can do with a file. Every other row is a listing and says so by
/// not responding — a listing that lists is not the same as a control that
/// looks like it does something.
/// One asset row, plus the sprites under it when its image is sliced and
/// showing.
///
/// Its own function because the row and its children are one thing to a reader
/// — an image and its parts — even though the browser draws them as sibling
/// rows.
fn sliceable_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
    expanded: &mut BTreeSet<PathBuf>,
) -> Option<BrowserAction> {
    // A search shows a flat list, so parts under an image would be pointing at
    // a parent the search may have removed.
    let sliced = !entry.sprites.is_empty() && !searching;
    let mut showing = expanded.contains(&entry.path);
    let row = asset_row(
        ui,
        entry,
        depth,
        searching,
        open,
        sliced.then_some(&mut showing),
    );
    if sliced {
        if showing {
            expanded.insert(entry.path.clone());
            for sprite in &entry.sprites {
                sprite_row(ui, sprite, depth + 1);
            }
        } else {
            expanded.remove(&entry.path);
        }
    }
    if row.double_clicked() && entry.kind == AssetKind::Scene {
        return Some(BrowserAction::Open(entry.path.clone()));
    }
    if row.clicked() && entry.kind == AssetKind::Texture {
        return Some(BrowserAction::Select(entry.path.clone()));
    }
    None
}

/// One named part of a sliced image, under the image it came from.
///
/// Not a `ProjectEntry`: a sprite has no file, and giving it one would put it in
/// the directory listing as something that could be opened, renamed, or deleted
/// on its own. It is a row and nothing more.
fn sprite_row(ui: &mut egui::Ui, sprite: &str, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + hierarchy_indent(depth, 12.0));
        ui.label(
            asset_icon(AssetKind::Sprite)
                .outlined()
                .rich_text()
                .size(13.0)
                .color(TEXT_MUTED),
        );
        ui.label(RichText::new(sprite).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            ui.label(
                RichText::new(AssetKind::Sprite.label())
                    .size(9.0)
                    .color(TEXT_MUTED),
            );
        });
    });
}

fn asset_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
    // `Some` for a sliced image, carrying whether its parts are showing. A row
    // with nothing under it gets no triangle rather than a disabled one.
    expanded: Option<&mut bool>,
) -> Response {
    let openable = matches!(entry.kind, AssetKind::Scene | AssetKind::Texture);
    let highlighted = open.is_some_and(|path| path == entry.path);
    let sense = if openable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let row = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0 + hierarchy_indent(depth, 12.0));
            if let Some(expanded) = expanded {
                let triangle = ui.add(
                    egui::Label::new(
                        if *expanded {
                            ICON_KEYBOARD_ARROW_DOWN
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT
                        }
                        .outlined()
                        .rich_text()
                        .size(13.0)
                        .color(TEXT_FAINT),
                    )
                    .sense(Sense::click()),
                );
                if triangle.clicked() {
                    *expanded = !*expanded;
                }
                ui.add_space(2.0);
            } else {
                // The same space a triangle would take, so names in a listing
                // line up whether or not their image is sliced.
                ui.add_space(12.0);
            }
            // Every label in the row is given the row's own sense. A widget
            // inside a scope takes precedence over the scope, and an ordinary
            // label is selectable text, so it answers a double click by
            // selecting a word rather than letting the row have it.
            let icon = ui.add(
                egui::Label::new(
                    asset_icon(entry.kind)
                        .outlined()
                        .rich_text()
                        .size(15.0)
                        .color(if highlighted { ACCENT } else { TEXT_FAINT }),
                )
                .sense(sense),
            );
            ui.add_space(2.0);
            // Under a search the path below the root is what tells two files of
            // the same name apart.
            let text = if searching {
                &entry.relative
            } else {
                &entry.name
            };
            let label = ui.add(
                egui::Label::new(RichText::new(text).size(11.0).color(if highlighted {
                    TEXT
                } else {
                    TEXT_MUTED
                }))
                // Not selectable text: a double click on a file name means
                // open it, and selecting the word "json" is not a thing anyone
                // wanted from a file listing.
                .selectable(false)
                .sense(sense),
            );
            let kind = ui
                .with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(entry.kind.label())
                                .size(10.0)
                                .color(TEXT_FAINT),
                        )
                        .sense(sense),
                    )
                })
                .inner;
            icon | label | kind
        })
        .inner
    });
    let row = row.response | row.inner;
    if openable {
        row.on_hover_text("Double-click to open")
    } else {
        row
    }
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

fn asset_tile(ui: &mut egui::Ui, entry: &ProjectEntry, open: Option<&Path>) -> Response {
    let highlighted = open.is_some_and(|path| path == entry.path);
    ui.vertical(|ui| {
        let tile = ui.add_sized(
            [62.0, 54.0],
            egui::Button::new(
                asset_icon(entry.kind)
                    .outlined()
                    .rich_text()
                    .size(27.0)
                    .color(if highlighted { ACCENT } else { TEXT_MUTED }),
            )
            .fill(PANEL_RAISED)
            .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
        );
        ui.add_sized(
            [62.0, 17.0],
            egui::Label::new(RichText::new(&entry.name).size(10.0).color(TEXT_MUTED)).truncate(),
        );
        if entry.kind == AssetKind::Scene {
            tile.on_hover_text("Double-click to open")
        } else {
            tile
        }
    })
    .inner
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

/// How much time an animation takes from a frame, given where the transport is.
///
/// Only a running engine moves an animation on; paused holds where it is, which
/// is not the same as advancing by nothing only because stopping resets. The cap
/// is the same quarter second `FixedStepConfig` caps a frame at, shared rather
/// than chosen again here, so a window left behind another one for a minute
/// comes back where it left off rather than wherever a minute of animation
/// lands.
fn animation_delta(state: EngineState, frame_seconds: f32) -> f32 {
    if state != EngineState::Running || !frame_seconds.is_finite() || frame_seconds < 0.0 {
        return 0.0;
    }
    frame_seconds.min(FixedStepConfig::default().max_frame_delta.as_secs_f32())
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

fn paint_transform_gizmo(
    painter: &egui::Painter,
    rect: Rect,
    visual: &gizmo::GizmoVisual,
    active: Option<Axis>,
) {
    for handle in &visual.handles {
        let color = if active == Some(handle.axis) {
            Color32::WHITE
        } else {
            match handle.axis {
                Axis::X => Color32::from_rgb(239, 92, 101),
                Axis::Y => Color32::from_rgb(89, 201, 135),
                Axis::Z => Color32::from_rgb(91, 151, 239),
            }
        };
        let points: Vec<Pos2> = handle
            .points
            .iter()
            .map(|point| rect.min + Vec2::new(point.x, point.y))
            .collect();
        painter.add(Shape::line(points.clone(), Stroke::new(2.5, color)));
        if let Some(end) = points.last().copied()
            && handle.points.len() == 2
        {
            painter.circle_filled(end, 4.0, color);
        }
    }
    painter.circle_filled(
        rect.min + Vec2::new(visual.origin.x, visual.origin.y),
        3.5,
        Color32::WHITE,
    );
}

fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    selected_name: &str,
    error: Option<&str>,
    axes: Option<Mat4>,
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
        "Primary: orbit or tool  ·  Secondary: orbit  ·  Shift-drag: pan",
        FontId::proportional(10.0),
        TEXT_FAINT,
    );
    paint_error_banner(painter, rect, error);
    if let Some(view) = axes {
        paint_axis_gizmo(
            painter,
            Pos2::new(rect.right() - 42.0, rect.top() + 48.0),
            view,
        );
    }
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

/// How long an axis arm is when it points straight across the screen.
const AXIS_ARM: f32 = 22.0;

/// Where the three world axes point on screen, and in what order to draw them.
///
/// This used to be three hardcoded offsets, so the indicator claimed the same
/// orientation whichever way the camera was facing — the one control in the
/// editor that was wrong rather than merely idle, and the one the first audit
/// walked past because it swept controls instead of pixels.
///
/// Each axis is turned by the camera's view and then flattened: the screen's Y
/// grows downwards, so the view's Y is negated, and an axis pointing at or away
/// from the viewer foreshortens to a stub of its own accord. The order is back
/// to front by how near the viewer each arm ends, so the arm behind is drawn
/// under the ones in front rather than over them.
fn axis_arms(view: Mat4, length: f32) -> [(Vec2, Color32, &'static str); 3] {
    let mut arms = [
        (Vec3::X, Color32::from_rgb(239, 92, 101), "X"),
        (Vec3::Y, Color32::from_rgb(89, 201, 135), "Y"),
        (Vec3::Z, Color32::from_rgb(91, 151, 239), "Z"),
    ]
    .map(|(axis, color, label)| {
        let facing = view.transform_vector3(axis);
        (
            facing,
            Vec2::new(facing.x, -facing.y) * length,
            color,
            label,
        )
    });
    // Ascending depth: in view space the camera looks down -Z, so the largest Z
    // is the arm nearest the viewer and is drawn last.
    arms.sort_by(|left, right| left.0.z.total_cmp(&right.0.z));
    arms.map(|(_, offset, color, label)| (offset, color, label))
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2, view: Mat4) {
    for (offset, color, label) in axis_arms(view, AXIS_ARM) {
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

/// Rows currently visible after folding and filtering are applied.
///
/// Search deliberately ignores folded state and retains every ancestor of a
/// match. A result therefore still says where it lives instead of becoming a
/// misleading flat list, and clearing the search restores the user's folds.
fn visible_hierarchy_rows(
    world: &World,
    collapsed: &BTreeSet<EntityId>,
    needle: &str,
) -> Vec<(EntityId, usize)> {
    let included = if needle.is_empty() {
        None
    } else {
        let mut included = BTreeSet::new();
        for (entity, data) in world.entities() {
            if !entity_name(data).to_lowercase().contains(needle) {
                continue;
            }
            let mut cursor = Some(entity);
            while let Some(current) = cursor {
                if !included.insert(current) {
                    break;
                }
                cursor = world.get(current).and_then(|data| data.parent);
            }
        }
        Some(included)
    };

    let mut roots: Vec<EntityId> = world
        .entities()
        .filter(|(_, data)| data.parent.is_none())
        .map(|(entity, _)| entity)
        .collect();
    roots.sort_by_key(|entity| hierarchy_sort_key(world, *entity));

    let mut rows = Vec::new();
    for root in roots {
        push_visible_hierarchy_row(world, root, 0, collapsed, included.as_ref(), &mut rows);
    }
    rows
}

fn push_visible_hierarchy_row(
    world: &World,
    entity: EntityId,
    depth: usize,
    collapsed: &BTreeSet<EntityId>,
    included: Option<&BTreeSet<EntityId>>,
    rows: &mut Vec<(EntityId, usize)>,
) {
    if included.is_some_and(|included| !included.contains(&entity)) {
        return;
    }
    rows.push((entity, depth));
    if included.is_none() && collapsed.contains(&entity) {
        return;
    }
    let Some(data) = world.get(entity) else {
        return;
    };
    let mut children = data.children.clone();
    children.sort_by_key(|child| hierarchy_sort_key(world, *child));
    for child in children {
        push_visible_hierarchy_row(world, child, depth + 1, collapsed, included, rows);
    }
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

fn hierarchy_preference_key(
    path: Option<&Path>,
    world: &World,
    entity: EntityId,
) -> Option<String> {
    let path = path?;
    let source_id = world.get(entity)?.source_id.as_ref()?;
    Some(format!("{}::{}", path.display(), source_id.as_str()))
}

/// A stable ID is assigned before the spawn enters history so save, undo, and
/// redo all agree on the identity of a newly authored `GameObject`.
fn next_game_object_id(world: &World) -> SceneEntityId {
    let mut suffix = 1_u32;
    loop {
        let candidate = SceneEntityId::new(format!("game-object-{suffix}"))
            .expect("the generated GameObject ID is valid");
        if world
            .entities()
            .all(|(_, data)| data.source_id.as_ref() != Some(&candidate))
        {
            return candidate;
        }
        suffix += 1;
    }
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
fn open_requested_scene(remembered: Option<&str>) -> (SceneFile, Option<String>) {
    open_scene_for(std::env::args().nth(1).as_deref(), remembered)
}

/// Opens whichever scene was asked for, in order of how deliberately.
///
/// A path on the command line is the most deliberate thing anyone can say, so
/// it wins. Otherwise the scene the editor was last left in, which is what
/// makes reopening the editor continue rather than restart. Otherwise the demo
/// scene, so a clean clone is useful.
///
/// A remembered scene that has moved or been deleted since is not a reason to
/// open on nothing: that choice was made last week, the failure is not the
/// user's doing now, and falling back to the default while saying what happened
/// leaves them somewhere they can work. A path given on the command line gets
/// no such fallback — standing in a different scene for the one someone just
/// named reads as though it opened.
fn open_scene_for(argument: Option<&str>, remembered: Option<&str>) -> (SceneFile, Option<String>) {
    let path = argument.or(remembered).unwrap_or(DEFAULT_SCENE_PATH);
    match SceneFile::open(path) {
        Ok(file) => (file, None),
        // The reported failure is the remembered scene's, not the fallback's:
        // what went wrong is that the file someone was working in is not
        // there, and the demo scene's absence would not be news.
        Err(error) if argument.is_none() && remembered.is_some() => (
            SceneFile::open(DEFAULT_SCENE_PATH)
                .unwrap_or_else(|_| SceneFile::detached(SceneDocument::default())),
            Some(error.to_string()),
        ),
        Err(error) => (
            SceneFile::detached(SceneDocument::default()),
            Some(error.to_string()),
        ),
    }
}

/// The component schemas the editor understands.
///
/// `sindri.script` is registered here rather than in `sindri-scene`, because
/// the engine's scene crate must not learn about a language —
/// `SceneExtractor::register` exists for exactly this, a component the host
/// brings of its own. It is one function rather than an inline registration so
/// that the editor and everything asserting about the editor's scenes agree by
/// construction instead of by both remembering the same list.
pub fn scene_extractor() -> SceneExtractor {
    let mut scene = SceneExtractor::new().expect("the built-in component schemas register");
    scene
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    scene
}

/// Builds a runtime world from a document the editor has opened.
///
/// `Preserve` rather than `Reject`: a scene may carry components this build has
/// never heard of, and the format exists to keep them through a load, an edit,
/// and a save. Rejecting them is how the editor came to refuse — and from the
/// command line, crash on — any project that defined a component of its own.
pub fn load_world(extractor: &SceneExtractor, document: &SceneDocument) -> Result<World, String> {
    extractor
        .validate(document, UnknownComponentPolicy::Preserve)
        .map_err(|error| error.to_string())?;
    Ok(World::from_scene(document)
        .map_err(|error| error.to_string())?
        .world)
}
