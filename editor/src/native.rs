use std::{
    collections::{BTreeMap, BTreeSet},
    f32::consts::TAU,
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{
    egui::{
        self, Align, Align2, Color32, FontData, FontFamily, FontId, Layout, Pos2, Rect, Response,
        RichText, Sense, Shape, Stroke, StrokeKind, TextStyle, Vec2,
    },
    wgpu,
};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_ACCOUNT_TREE, ICON_ADD, ICON_CAMERA_ALT, ICON_CENTER_FOCUS_STRONG, ICON_CODE,
        ICON_DELETE, ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_FOLDER, ICON_GRID_4X4,
        ICON_GRID_VIEW, ICON_IMAGE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
        ICON_LABEL, ICON_MOVE, ICON_OPEN_WITH, ICON_PAUSE, ICON_PLAY_ARROW, ICON_REDO,
        ICON_REFRESH, ICON_ROTATE_RIGHT, ICON_SCALE, ICON_SEARCH, ICON_SELECT, ICON_STOP,
        ICON_UNDO, ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};
use glam::{EulerRot, Mat4, Quat, Vec2 as GlamVec2, Vec3};
use serde_json::Value;
use sindri_core::{
    CommandBuffer, CommandHistory, ComponentMetadata, ComponentSchemaRegistry, EngineLifecycle,
    EngineState, EntityData, EntityId, FixedStepConfig, SceneComponent, SceneDocument,
    SceneEntityId, SpriteRef, Transform3D, UnknownComponentPolicy, World, WorldCommand,
};
use sindri_decay::{ScriptComponent, ScriptValue};
use sindri_render::{
    FrameRenderers, FrameTarget, SpriteBatchRenderer, TextRenderer, TexturedCubeRenderer, Viewport,
    ViewportTarget, encode_prepared_frame,
};
use sindri_scene::{
    CameraView, GridNavigationComponent, GridOccupantComponent, SceneExtractor, SpriteAnimations,
    SpriteSpace, ViewCamera, WorldProjection,
};

use crate::{
    animation::{self, AnimationTool},
    console::{Console, Entry, Level},
    gizmo::{self, Axis, GizmoDrag, GizmoMode, GizmoSpace, Snapping},
    input::EditorInput,
    inspector,
    picking,
    // `egui::Layout` is a different thing entirely and is already in scope.
    preferences::{AssetView, BottomTab, CameraProjection, Layout as WorkspaceLayout, Preferences},
    project::{AssetKind, ProjectEntry, ProjectTree},
    scene_file::{DEFAULT_SCENE_PATH, SceneFile},
    scripts::{SceneScripts, ScriptNote},
    slicer::Slicer,
    textures::{SceneTextures, TextureNote},
    tilemap::{
        self, PaletteSprite, TileBrush, TilemapTool, paint as paint_tile, resize as resize_tilemap,
    },
};

const INTER_FONT: &[u8] = include_bytes!("../assets/Inter.ttf");
const TEXT_COMPONENT: &str = "sindri.text";
const GRID_NAVIGATION_COMPONENT: &str = GridNavigationComponent::TYPE_NAME;
const GRID_OCCUPANT_COMPONENT: &str = GridOccupantComponent::TYPE_NAME;
const ACCENT: Color32 = Color32::from_rgb(246, 169, 35);
/// What a panel says something is wrong in, matching the console's errors.
const PROBLEM: Color32 = Color32::from_rgb(255, 138, 148);
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

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            orbit: GlamVec2::ZERO,
            zoom: 1.0,
            pan: GlamVec2::ZERO,
            projection: CameraProjection::Perspective,
        }
    }
}

/// The pan that would put `position` in the middle of what `camera` frames.
///
/// The pan's own definition, read backwards: a pan of one moves the picture by
/// exactly the framed half-height, so a subject that far from the middle is
/// exactly one pan away from it. Kept apart from the control that calls it
/// because the way this goes wrong is a sign, and a sign is only visible by
/// asking where the subject ended up.
fn pan_to_centre(camera: ViewCamera, pan: GlamVec2, position: Vec3) -> GlamVec2 {
    if camera.framed_half_height <= 0.0 {
        return pan;
    }
    let offset = camera.view.transform_point3(position);
    pan - GlamVec2::new(offset.x, offset.y) / camera.framed_half_height
}

/// How far from the authored camera's own elevation a drag can pitch.
///
/// A little under a right angle either way. The orbit cannot reach the pole
/// whatever this says — the extractor guarantees that, where the authored
/// elevation is known — so this is only about how much drag is worth spending.
const PITCH_LIMIT: f32 = 1.5;

/// How far in and out the wheel can take the scene view.
///
/// The old pair, 0.65 to 1.8, could not frame anything much larger or smaller
/// than the demo cube: not quite twice as far out, and not quite twice as
/// close. A scene is whatever someone builds, so the range is a factor of four
/// hundred and the wheel moves through it proportionally.
const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 20.0;

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

    /// Turns pointer input over the viewport into camera movement.
    ///
    /// Left drag orbits, middle drag or shift-drag pans, and the wheel zooms.
    /// None of it touches the scene: the authored camera stays where it is and
    /// only the view of it moves.
    fn move_camera(
        &mut self,
        context: &egui::Context,
        response: &Response,
        height: f32,
        painting: bool,
    ) {
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
            } else if response.dragged_by(egui::PointerButton::Secondary)
                || (!painting && response.dragged_by(egui::PointerButton::Primary))
            {
                self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                // Most of a right angle either way, from wherever the scene
                // authored its camera. The extractor stops the orbit short of
                // the pole itself, because that is where it knows how far the
                // authored camera was already tilted; this only decides how far
                // a drag is worth carrying.
                self.viewport_pitch =
                    (self.viewport_pitch + delta.y * 0.008).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
        }
        if response.hovered() {
            let delta = context.input(|input| input.smooth_scroll_delta.y);
            // Multiplied rather than added: the range spans a factor of four
            // hundred, and a fixed step that moves the picture usefully at one
            // end does nothing at the other. A notch of the wheel is the same
            // proportion of the distance wherever the camera is.
            self.viewport_zoom =
                (self.viewport_zoom * (delta * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
        }
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

    /// Puts the selected entity in the middle of the scene view.
    ///
    /// Centres rather than fits: fitting needs the bounds of what is selected,
    /// and an entity's bounds are a mesh's business, not a transform's. What
    /// this fixes is the ordinary way a subject is lost — panned off the edge,
    /// or never in frame because the authored camera was aimed elsewhere.
    ///
    /// The arithmetic is the pan's own definition read backwards. A pan of one
    /// moves the picture by exactly the framed half-height, so a subject sitting
    /// that far from the middle is exactly one pan away from it, and the
    /// extractor is asked for both numbers rather than the editor keeping its
    /// own copy of either.
    fn focus_selection(&mut self) {
        let Some(position) = self
            .selection
            .and_then(|entity| self.world.get(entity))
            .and_then(|data| data.transform_3d)
            .map(|transform| Vec3::from_array(transform.position))
        else {
            return;
        };
        let Ok(Some(camera)) = self.scene.world_camera(&self.world, self.scene_camera()) else {
            return;
        };
        self.viewport_pan = pan_to_centre(camera, self.viewport_pan, position);
    }

    /// The camera the scene view is looking through.
    fn scene_camera(&self) -> CameraView {
        camera_for(
            WorkspaceTab::Scene,
            EditorCamera {
                orbit: GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                zoom: self.viewport_zoom,
                pan: self.viewport_pan,
                projection: self.preferences.projection,
            },
        )
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

    /// Says that something the user asked for did not happen.
    ///
    /// The notice is the one line beside the viewport and is replaced by the
    /// next thing that goes wrong; the console keeps it. Every failure goes
    /// through here so the two cannot disagree about what happened.
    fn report(&mut self, message: String) {
        self.console.error(&message);
        self.notice = Some(message);
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

    fn record_script_notes(&mut self, notes: Vec<ScriptNote>) {
        for note in notes {
            match note {
                ScriptNote::Loaded(message) | ScriptNote::Reloaded(message) => {
                    self.console.info(message);
                }
                ScriptNote::Failed(message) => self.console.warning(message),
            }
        }
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

    fn record_texture_notes(&mut self, notes: Vec<TextureNote>) {
        for note in notes {
            match note {
                TextureNote::Loaded(message) | TextureNote::Reloaded(message) => {
                    self.console.info(message);
                }
                TextureNote::Failed(message) => self.console.warning(message),
            }
        }
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
            self.report(error.to_string());
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

    /// The components this entity does not have and the registry can create.
    ///
    /// A type with no default payload is missing from the list rather than
    /// offered and refused: a button that adds a component the engine will
    /// then reject is worse than no button, which is why the old Add Component
    /// was removed instead of left drawn.
    fn addable_components(
        &self,
        present: &BTreeMap<String, Value>,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) -> Vec<ComponentMetadata> {
        addable_components(
            self.scene.components(),
            present,
            first_font,
            first_sprite,
            first_grid,
        )
    }

    /// Turns every changed component payload into a command.
    ///
    /// Each is checked against its own schema first. A payload is written back
    /// exactly as stored, so an edit that stopped it decoding would produce a
    /// scene the engine refuses to open — and the author would find out on the
    /// next launch rather than at the field they were editing.
    fn commit_components(
        &mut self,
        entity: EntityId,
        original: &BTreeMap<String, Value>,
        draft: &BTreeMap<String, Value>,
    ) {
        let (buffer, refused) =
            component_commands(entity, original, draft, self.scene.components());
        for message in refused {
            self.console.warning(message);
        }
        if buffer.is_empty() {
            return;
        }
        // The same merge key the rest of the inspector uses, so dragging a tint
        // is one undo step rather than one per frame of the drag.
        let transaction = buffer
            .into_transaction("Edit components")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Adds a component with the payload its schema says a fresh one starts as.
    fn add_component(
        &mut self,
        entity: EntityId,
        type_name: &str,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) {
        let Some(payload) = component_default(
            self.scene.components(),
            type_name,
            first_font,
            first_sprite,
            first_grid,
        ) else {
            return;
        };
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.to_owned(),
            payload,
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Add component"), &mut self.world)
        {
            self.report(error.to_string());
        }
        self.refresh_textures();
    }

    fn remove_component(&mut self, entity: EntityId, type_name: &str) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::RemoveComponent {
            entity,
            type_name: type_name.to_owned(),
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Remove component"), &mut self.world)
        {
            self.report(error.to_string());
        }
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
                panel_title(ui, "Inspector");
                if self.slicer.is_some() {
                    self.slicer_panel(ui);
                    return;
                }
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
                let original_components = data.components.clone();
                let mut components = original_components.clone();
                let parent = data.parent;
                let choices = reparent_choices(&self.world, entity);
                let mut reparented = ParentChoice::Unchanged;
                let mut removed = None;
                let mut added = None;
                let fonts = self.project.fonts();
                let first_font = fonts.first().map(String::as_str);
                let animation_texture = components
                    .get("sindri.sprite")
                    .and_then(|sprite| sprite.get("texture"))
                    .and_then(Value::as_str)
                    .and_then(|reference| SpriteRef::parse(reference).ok())
                    .map(|reference| reference.texture().to_owned());
                let animation_sprites = animation_texture
                    .as_deref()
                    .map(|texture| self.project.sprites_for_texture(texture))
                    .unwrap_or_default();
                let first_sprite = animation_sprites.first().map(String::as_str);
                let grids = grid_choices(&self.world);
                let first_grid = grids.first().map(|(_, id)| id.as_str());
                let addable =
                    self.addable_components(&components, first_font, first_sprite, first_grid);
                let project_root = self.project.root().map(Path::to_path_buf);
                {
                    let scripts = &self.scripts;
                    let mut tools = InspectorTools {
                        animation: &mut self.animation_tool,
                        tilemap: &mut self.tilemap_tool,
                    };
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, icon, &mut draft);
                        reparented = inspector_parent(ui, entity, parent, &choices);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        removed = components_sections(
                            ui,
                            &mut components,
                            &InspectorProject {
                                scripts,
                                root: project_root.as_deref(),
                                fonts: &fonts,
                                animation_texture: animation_texture.as_deref(),
                                grids: &grids,
                            },
                            &mut tools,
                        );
                        added = add_component_button(ui, &addable);
                    });
                }
                self.commit_draft(entity, &original, &draft);
                self.commit_components(entity, &original_components, &components);
                if let Some(type_name) = removed {
                    self.remove_component(entity, &type_name);
                }
                if let Some(type_name) = added {
                    self.add_component(entity, &type_name, first_font, first_sprite, first_grid);
                }
                match reparented {
                    ParentChoice::Unchanged => {}
                    ParentChoice::Root => self.reparent(entity, None),
                    ParentChoice::Under(parent) => self.reparent(entity, Some(parent)),
                }
            });
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

/// A panel's heading.
///
/// Actions live beneath the heading rather than being smuggled into this
/// shared decoration. The hierarchy's create menu, for example, now has both
/// an undoable spawn command and stable authored IDs behind it.
fn panel_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
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
    // "Tag  Untagged" and "Layer  Default" used to sit under the name. Neither
    // is a thing a Sindri entity has, so they were two lines of a different
    // engine's inspector printed over this one's.
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

fn section_header(ui: &mut egui::Ui, icon: MaterialIcon, title: &str) {
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
    });
}

fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    section_header(ui, ICON_OPEN_WITH, "Transform");
    // The Z drag is taken away rather than left to fail: the command layer
    // would refuse the edit anyway, and a control that cannot do what it looks
    // like it does is the thing this editor is trying not to grow.
    vector_row(ui, "Position", &mut transform.position, transform.z_locked);
    let rotation = Quat::from_array(transform.rotation);
    let rotation = if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
    if vector_row(ui, "Rotation", &mut degrees, false) {
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            degrees[0].to_radians(),
            degrees[1].to_radians(),
            degrees[2].to_radians(),
        )
        .to_array();
    }
    vector_row(ui, "Scale", &mut transform.scale, false);
    property_toggle(ui, "Z lock", &mut transform.z_locked, "Locked", "Free");
}

/// A property row whose value is a choice rather than a readout.
///
/// Shaped like [`property_label`] because it sits among those rows, and reading
/// as a label until you notice it responds is the point: what it says is the
/// state, and pressing it is how the state changes.
fn property_toggle(ui: &mut egui::Ui, label: &str, value: &mut bool, on: &str, off: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            let text = if *value { on } else { off };
            let color = if *value { ACCENT } else { TEXT_MUTED };
            if ui
                .selectable_label(*value, RichText::new(text).size(11.0).color(color))
                .clicked()
            {
                *value = !*value;
            }
        });
    });
}

/// Stateful authoring surfaces shared across component sections.
struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// What the inspector reads about the project it is editing inside.
///
/// Grouped rather than passed one by one because every component section wants
/// some subset of it, and the list only grows: each new component that names a
/// project asset would otherwise add another parameter to one signature.
struct InspectorProject<'a> {
    scripts: &'a SceneScripts,
    root: Option<&'a Path>,
    fonts: &'a [String],
    animation_texture: Option<&'a str>,
    grids: &'a [(String, String)],
}

/// Draws every component on an entity, editable, and reports what changed.
///
/// The payload is edited in place on a draft; the caller diffs it and turns
/// each difference into a `SetComponent`. Nothing here writes to the world.
fn components_sections(
    ui: &mut egui::Ui,
    components: &mut BTreeMap<String, Value>,
    project: &InspectorProject<'_>,
    tools: &mut InspectorTools<'_>,
) -> Option<String> {
    let InspectorProject {
        scripts,
        root: project_root,
        fonts,
        animation_texture,
        grids,
    } = *project;
    let grid_size = components
        .get(tilemap::TYPE_NAME)
        .and_then(|payload| tilemap::component(payload).ok())
        .map(|map| (map.columns, map.rows));
    let mut removed = None;
    for (name, payload) in components.iter_mut() {
        let icon = match name.as_str() {
            "sindri.camera" => ICON_CAMERA_ALT,
            "sindri.sprite" => ICON_IMAGE,
            "sindri.mesh" => ICON_VIEW_IN_AR,
            "sindri.script" => ICON_CODE,
            "sindri.text" => ICON_LABEL,
            "sindri.animation.sprite" => ICON_PLAY_ARROW,
            "sindri.tilemap" => ICON_GRID_VIEW,
            _ => ICON_DEPLOYED_CODE,
        };
        ui.add_space(4.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
            ui.label(
                RichText::new(component_label(name))
                    .strong()
                    .size(12.0)
                    .color(TEXT),
            );
            if inspector::is_removable(name) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(7.0);
                    if ui
                        .small_button(ICON_DELETE.outlined().rich_text().size(13.0))
                        .on_hover_text(format!("Remove {}", component_label(name)))
                        .clicked()
                    {
                        removed = Some(name.clone());
                    }
                });
            }
        });

        // A script's @export fields come first and are drawn from what the
        // script declared, which is the whole reason the language is typed.
        // The rest of the payload -- the source, the container -- follows as
        // ordinary rows.
        if name == "sindri.script" {
            script_exports_section(ui, payload, scripts);
        }
        if name == TEXT_COMPONENT {
            text_section(ui, payload, fonts);
        }
        if name == animation::TYPE_NAME {
            animation_section(
                ui,
                payload,
                project_root,
                animation_texture,
                tools.animation,
            );
        }
        if name == tilemap::TYPE_NAME {
            tilemap_section(ui, payload, project_root, tools.tilemap);
        }
        if name == GRID_NAVIGATION_COMPONENT {
            grid_navigation_section(ui, payload, grid_size);
        }
        if name == GRID_OCCUPANT_COMPONENT {
            grid_occupant_section(ui, payload, grids);
        }
        object_rows(ui, name, payload, name == "sindri.script");
    }
    removed
}

/// The two text fields whose meaning is richer than their JSON shape.
///
/// Content is multiline gameplay/UI copy, and a font is a project-owned asset
/// reference. Leaving either as an ordinary one-line string technically edits
/// the payload but makes the editor less useful than editing JSON by hand.
fn text_section(ui: &mut egui::Ui, payload: &mut Value, fonts: &[String]) {
    let mut content = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Text").size(11.0).color(TEXT_MUTED));
    });
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let width = (ui.available_width() - 7.0).max(120.0);
        if ui
            .add_sized(
                [width, 76.0],
                egui::TextEdit::multiline(&mut content)
                    .desired_rows(3)
                    .hint_text("Text shown in the game"),
            )
            .changed()
        {
            payload["text"] = Value::String(content);
        }
    });

    let current = payload
        .get("font")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Font").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("text-font-asset")
                .selected_text(if chosen.is_empty() {
                    "Choose a font"
                } else {
                    chosen.as_str()
                })
                .width(190.0)
                .show_ui(ui, |ui| {
                    for font in fonts {
                        ui.selectable_value(&mut chosen, font.clone(), font);
                    }
                });
        });
    });
    if chosen != current {
        payload["font"] = Value::String(chosen.clone());
    }

    let missing = fonts.is_empty() || chosen.is_empty() || !fonts.contains(&chosen);
    if missing {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            let message = if fonts.is_empty() {
                "Add an OpenType font to the project before adding text."
            } else {
                "The selected font is not present in this project."
            };
            ui.label(RichText::new(message).size(9.0).color(PROBLEM));
        });
    }
}

/// Clip authoring for the selected entity's sprite sheet.
///
/// The sheet owns sprite names; the animation only arranges those names into
/// timed clips. Every edit stays in the stored payload so unknown future fields
/// survive, while the typed component is used to interpret and preview it.
#[allow(clippy::too_many_lines)]
fn animation_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    project_root: Option<&Path>,
    texture: Option<&str>,
    tool: &mut AnimationTool,
) {
    let Some(texture) = texture else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Add a Sprite component before authoring animation clips.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
        return;
    };
    tool.palette.ensure(project_root, texture);
    let sprite_names: Vec<String> = tool
        .palette
        .sprites()
        .iter()
        .map(|sprite| sprite.name.clone())
        .collect();

    let Ok(mut authored) = animation::component(payload) else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("This animation cannot be read; repair its stored fields first.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
        return;
    };

    section_header(ui, ICON_PLAY_ARROW, "Clips");
    let mut selected = tool.selected(&authored).map(str::to_owned);
    let clip_names: Vec<String> = authored.clips.keys().cloned().collect();
    let mut chosen = selected.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Clip").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("animation-clip")
                .selected_text(if chosen.is_empty() {
                    "No clips"
                } else {
                    chosen.as_str()
                })
                .width(170.0)
                .show_ui(ui, |ui| {
                    for name in &clip_names {
                        ui.selectable_value(&mut chosen, name.clone(), name);
                    }
                });
        });
    });
    if selected.as_deref() != Some(chosen.as_str()) && !chosen.is_empty() {
        tool.select(chosen.clone());
        selected = Some(chosen);
    }

    let mut add = false;
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        add = ui
            .add_enabled(!sprite_names.is_empty(), egui::Button::new("Add clip"))
            .clicked();
        remove = ui
            .add_enabled(selected.is_some(), egui::Button::new("Remove"))
            .clicked();
    });
    if add
        && let Some(first) = sprite_names.first()
        && let Ok(name) = animation::add_clip(payload, first)
    {
        tool.select(name.clone());
        selected = Some(name);
        authored = animation::component(payload).unwrap_or(authored);
    }
    if remove
        && let Some(name) = selected.as_deref()
        && animation::remove_clip(payload, name).unwrap_or(false)
    {
        tool.reset();
        tool.palette.ensure(project_root, texture);
        authored = animation::component(payload).unwrap_or(authored);
        selected = tool.selected(&authored).map(str::to_owned);
    }

    let Some(selected) = selected else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(if sprite_names.is_empty() {
                    "Slice and name the sprite texture before adding a clip."
                } else {
                    "Add a clip to arrange the sheet's sprites into playback."
                })
                .size(9.0)
                .color(TEXT_MUTED),
            );
        });
        if let Some(problem) = tool.palette.problem() {
            animation_problem(ui, problem);
        }
        return;
    };

    let mut rename_to = tool.rename().clone();
    let mut rename = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Name").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            rename = ui
                .add_enabled(rename_to.trim() != selected, egui::Button::new("Rename"))
                .clicked();
            ui.add_sized([128.0, 23.0], egui::TextEdit::singleline(&mut rename_to));
        });
    });
    tool.rename().clone_from(&rename_to);
    let mut problem = None;
    let selected = if rename {
        match animation::rename_clip(payload, &selected, &rename_to) {
            Ok(true) => {
                let renamed = rename_to.trim().to_owned();
                tool.renamed(renamed.clone());
                authored = animation::component(payload).unwrap_or(authored);
                renamed
            }
            Ok(false) => selected,
            Err(error) => {
                problem = Some(error);
                selected
            }
        }
    } else {
        selected
    };

    let mut playing = authored.playing.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Playing").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("animation-playing")
                .selected_text(if playing.is_empty() {
                    "None"
                } else {
                    playing.as_str()
                })
                .width(170.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut playing, String::new(), "None");
                    for name in authored.clips.keys() {
                        ui.selectable_value(&mut playing, name.clone(), name);
                    }
                });
        });
    });
    let stored_playing = payload.get("playing").and_then(Value::as_str).unwrap_or("");
    if playing != stored_playing {
        payload["playing"] = if playing.is_empty() {
            Value::Null
        } else {
            Value::String(playing)
        };
    }

    let Some(clip) = authored.clips.get(&selected).cloned() else {
        animation_problem(ui, "The selected clip no longer exists.");
        return;
    };
    let mut seconds = f64::from(clip.seconds_per_frame);
    if number_row(ui, "Frame time", &mut seconds, 10.0, false) {
        payload["clips"][selected.as_str()]["seconds_per_frame"] = Value::from(seconds.max(0.001));
    }
    let mut looping = clip.looping;
    if bool_row(ui, "Loop", &mut looping, 10.0) {
        payload["clips"][selected.as_str()]["looping"] = Value::Bool(looping);
    }

    section_header(ui, ICON_IMAGE, "Frames");
    let mut replace = None;
    let mut frame_action = None;
    for (index, frame) in clip.frames.iter().enumerate() {
        let mut sprite = frame.clone();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{}", index + 1))
                    .size(10.0)
                    .color(TEXT_FAINT),
            );
            egui::ComboBox::from_id_salt(("animation-frame", index))
                .selected_text(&sprite)
                .width(132.0)
                .show_ui(ui, |ui| {
                    for name in &sprite_names {
                        ui.selectable_value(&mut sprite, name.clone(), name);
                    }
                });
            if ui.small_button("Up").clicked() {
                frame_action = Some((index, -1));
            }
            if ui.small_button("Down").clicked() {
                frame_action = Some((index, 1));
            }
            if ui
                .add_enabled(clip.frames.len() > 1, egui::Button::new("Remove"))
                .clicked()
            {
                frame_action = Some((index, 0));
            }
        });
        if sprite != *frame {
            replace = Some((index, sprite));
        }
        if !sprite_names.contains(frame) {
            animation_problem(
                ui,
                &format!("Frame {} names missing sprite {frame:?}.", index + 1),
            );
        }
    }
    if let Some((index, sprite)) = replace {
        let _ = animation::set_frame(payload, &selected, index, &sprite);
    }
    if let Some((index, direction)) = frame_action {
        if direction == 0 {
            let _ = animation::remove_frame(payload, &selected, index);
        } else {
            let _ = animation::move_frame(payload, &selected, index, direction);
        }
    }
    let mut appended = None;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.menu_button("Add frame", |ui| {
            for sprite in &sprite_names {
                if ui.button(sprite).clicked() {
                    appended = Some(sprite.clone());
                    ui.close();
                }
            }
            if sprite_names.is_empty() {
                ui.label("No named sprites");
            }
        });
    });
    if let Some(sprite) = appended {
        let _ = animation::push_frame(payload, &selected, &sprite);
    }

    if let Ok(updated) = animation::component(payload)
        && let Some(clip) = updated.clips.get(&selected)
    {
        animation_preview(ui, texture, &selected, clip, tool);
    }
    if let Some(message) = problem.as_deref().or_else(|| tool.palette.problem()) {
        animation_problem(ui, message);
    }
}

fn animation_preview(
    ui: &mut egui::Ui,
    texture_name: &str,
    clip_name: &str,
    clip: &sindri_scene::AnimationClip,
    tool: &mut AnimationTool,
) {
    section_header(ui, ICON_PLAY_ARROW, "Preview");
    let mut previewing = tool.previewing();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui
            .button(if previewing { "Stop" } else { "Play" })
            .clicked()
        {
            previewing = !previewing;
            tool.set_previewing(previewing);
        }
        ui.label(
            RichText::new(format!(
                "{} frames · {:.3}s",
                clip.frames.len(),
                clip.seconds_per_frame
            ))
            .size(10.0)
            .color(TEXT_MUTED),
        );
    });
    if previewing {
        ui.ctx().request_repaint();
    }
    let delta = ui.ctx().input(|input| input.stable_dt);
    let frame = tool.advance(clip_name, clip, delta);
    let sprite_name = clip.frames.get(frame).cloned();
    let sprite_rect = sprite_name
        .as_deref()
        .and_then(|name| tool.palette.sprite(name))
        .and_then(|sprite| sprite.rect);
    let texture = tool.palette.texture_id(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(176.0, 150.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, FIELD_BG);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, BORDER_SUBTLE),
        StrokeKind::Inside,
    );
    let image = Rect::from_min_max(
        rect.min + Vec2::splat(10.0),
        rect.max - Vec2::new(10.0, 28.0),
    );
    if let (Some(texture), Some(sprite_rect)) = (texture, sprite_rect) {
        let [x, y, width, height] = sprite_rect;
        painter.image(
            texture,
            image,
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
            Color32::WHITE,
        );
    } else {
        painter.line_segment(
            [image.left_top(), image.right_bottom()],
            Stroke::new(1.5, PROBLEM),
        );
        painter.line_segment(
            [image.right_top(), image.left_bottom()],
            Stroke::new(1.5, PROBLEM),
        );
    }
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 13.0),
        Align2::CENTER_CENTER,
        sprite_name.unwrap_or_else(|| format!("{texture_name}: no frame")),
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    let _ = response.on_hover_text("Animation preview uses the project texture and sheet");
}

fn animation_problem(ui: &mut egui::Ui, problem: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(problem).size(9.0).color(PROBLEM));
    });
}

/// The part of a tilemap that cannot be represented as independent JSON rows:
/// its dimensions, compact palette, and the brush that writes its cell array.
fn tilemap_section(
    ui: &mut egui::Ui,
    payload: &mut Value,
    project_root: Option<&Path>,
    tool: &mut TilemapTool,
) {
    let Ok(mut map) = tilemap::component(payload) else {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("This tilemap cannot be read; repair its stored fields first")
                    .size(10.0)
                    .color(PROBLEM),
            );
        });
        return;
    };

    section_header(ui, ICON_GRID_VIEW, "Map");
    let mut columns = f64::from(map.columns);
    let mut rows = f64::from(map.rows);
    let mut resized = number_row(ui, "Columns", &mut columns, 10.0, true);
    resized |= number_row(ui, "Rows", &mut rows, 10.0, true);
    if resized && let Err(error) = resize_tilemap(payload, grid_side(columns), grid_side(rows)) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(error).size(10.0).color(PROBLEM));
        });
    }
    // The resize above changes the payload this frame. Read it again so the
    // palette and the cell count below describe what the command will write.
    if let Ok(resized) = tilemap::component(payload) {
        map = resized;
    }

    let world_space = map.space == SpriteSpace::World;
    if !world_space {
        tool.enabled = false;
    }
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let label = if tool.enabled {
            "Painting in Scene view"
        } else {
            "Paint in Scene view"
        };
        if ui
            .add_enabled_ui(world_space, |ui| ui.selectable_label(tool.enabled, label))
            .inner
            .clicked()
        {
            tool.enabled = !tool.enabled;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(if !world_space {
                "Scene painting supports world-space tilemaps; switch Space to world first."
            } else if tool.enabled {
                "Primary drag paints. Middle or Shift-drag pans; secondary drag orbits."
            } else {
                "Enable painting, then choose a sprite or the eraser."
            })
            .size(9.0)
            .color(if world_space { TEXT_MUTED } else { PROBLEM }),
        );
    });

    tool.palette.ensure(project_root, &map.texture);
    let texture = tool.palette.texture_id(ui.ctx());
    let mut sprites = tool.palette.sprites().to_vec();
    // A broken or changed sheet must not make a sprite already used by the map
    // impossible to select and replace. It stays visible as a named fallback,
    // without a thumbnail that would pretend it still resolves.
    for name in &map.palette {
        if !sprites.iter().any(|sprite| sprite.name == *name) {
            sprites.push(PaletteSprite {
                name: name.clone(),
                rect: None,
            });
        }
    }
    if !tool.erase
        && tool
            .sprite
            .as_ref()
            .is_none_or(|chosen| !sprites.iter().any(|sprite| sprite.name == *chosen))
    {
        tool.sprite = sprites.first().map(|sprite| sprite.name.clone());
    }

    section_header(ui, ICON_IMAGE, "Palette");
    tile_palette(ui, texture, &sprites, tool);
    if let Some(problem) = tool.palette.problem() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(problem).size(9.0).color(PROBLEM));
        });
    }
}

fn tile_palette(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    sprites: &[PaletteSprite],
    tool: &mut TilemapTool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0);
        if palette_cell(ui, None, None, tool.erase) {
            tool.erase = true;
        }
        for sprite in sprites {
            let selected = !tool.erase && tool.sprite.as_deref() == Some(sprite.name.as_str());
            if palette_cell(ui, Some(sprite), texture, selected) {
                tool.erase = false;
                tool.sprite = Some(sprite.name.clone());
            }
        }
    });
    if sprites.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Slice and name the map's texture to populate this palette.")
                    .size(9.0)
                    .color(TEXT_MUTED),
            );
        });
    }
}

/// One compact palette swatch. Drawn directly so a named slice can preview a
/// UV rectangle without creating one egui texture per sprite.
fn palette_cell(
    ui: &mut egui::Ui,
    sprite: Option<&PaletteSprite>,
    texture: Option<egui::TextureId>,
    selected: bool,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(72.0, 72.0), Sense::click());
    let painter = ui.painter_at(rect);
    let border = if selected {
        ACCENT_BRIGHT
    } else if response.hovered() {
        TEXT_MUTED
    } else {
        BORDER
    };
    painter.rect_filled(rect, 4.0, if selected { ACCENT_SOFT } else { FIELD_BG });
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(if selected { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let preview = Rect::from_min_max(
        rect.min + Vec2::new(7.0, 6.0),
        Pos2::new(rect.max.x - 7.0, rect.max.y - 20.0),
    );
    match (sprite, texture) {
        (Some(sprite), Some(texture)) if sprite.rect.is_some() => {
            let [x, y, width, height] = sprite.rect.expect("checked above");
            painter.image(
                texture,
                preview,
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height)),
                Color32::WHITE,
            );
        }
        (Some(_), _) => {
            painter.rect_stroke(
                preview,
                2.0,
                Stroke::new(1.0, BORDER_SUBTLE),
                StrokeKind::Inside,
            );
        }
        (None, _) => {
            painter.line_segment(
                [preview.left_top(), preview.right_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
            painter.line_segment(
                [preview.right_top(), preview.left_bottom()],
                Stroke::new(2.0, PROBLEM),
            );
        }
    }
    let label = sprite.map_or("Erase", |sprite| sprite.name.as_str());
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 12.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        if selected { TEXT } else { TEXT_MUTED },
    );
    response.on_hover_text(label).clicked()
}

fn grid_choices(world: &World) -> Vec<(String, String)> {
    world
        .entities()
        .filter_map(|(_, data)| {
            data.components
                .contains_key(tilemap::TYPE_NAME)
                .then_some(())?;
            let id = data.source_id.as_ref()?.as_str().to_owned();
            let label = data.name.clone().unwrap_or_else(|| id.clone());
            Some((label, id))
        })
        .collect()
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn grid_coord_row(ui: &mut egui::Ui, label: &str, value: &mut Value) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    if items.len() != 2 {
        return false;
    }
    let mut numbers = [
        items[0].as_i64().unwrap_or_default() as f64,
        items[1].as_i64().unwrap_or_default() as f64,
    ];
    let labels = ["X".to_owned(), "Y".to_owned()];
    if !numbers_row(ui, label, &labels, &mut numbers, 18.0) {
        return false;
    }
    *value = serde_json::json!([numbers[0].round() as i64, numbers[1].round() as i64]);
    true
}

fn grid_navigation_section(ui: &mut egui::Ui, payload: &mut Value, grid_size: Option<(u32, u32)>) {
    section_header(ui, ICON_GRID_4X4, "Walls");
    let Some(walls) = payload.get_mut("walls").and_then(Value::as_array_mut) else {
        property_label(ui, "Walls", "stored value is not a wall list");
        return;
    };
    let mut remove = None;
    for (index, wall) in walls.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Wall {}", index + 1))
                    .size(10.0)
                    .color(TEXT_MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(7.0);
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        });
        if let Some(coord) = wall.get_mut("first") {
            grid_coord_row(ui, "First", coord);
        }
        if let Some(coord) = wall.get_mut("second") {
            grid_coord_row(ui, "Second", coord);
        }
        let first = wall.get("first").and_then(Value::as_array);
        let second = wall.get("second").and_then(Value::as_array);
        let valid = first.zip(second).is_some_and(|(first, second)| {
            if first.len() != 2 || second.len() != 2 {
                return false;
            }
            let (Some(ax), Some(ay), Some(bx), Some(by)) = (
                first[0].as_i64(),
                first[1].as_i64(),
                second[0].as_i64(),
                second[1].as_i64(),
            ) else {
                return false;
            };
            let adjacent = (ax - bx).abs() + (ay - by).abs() == 1;
            let inside = grid_size.is_none_or(|(columns, rows)| {
                let inside = |x: i64, y: i64| {
                    x >= 0 && y >= 0 && x < i64::from(columns) && y < i64::from(rows)
                };
                inside(ax, ay) && inside(bx, by)
            });
            adjacent && inside
        });
        if !valid {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(18.0);
                ui.label(
                    RichText::new("Wall endpoints must be adjacent cells inside the tilemap.")
                        .size(9.0)
                        .color(PROBLEM),
                );
            });
        }
    }
    if let Some(index) = remove {
        walls.remove(index);
    }
    let can_add = grid_size.is_some_and(|(columns, rows)| columns > 1 || rows > 1);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui
            .add_enabled(can_add, egui::Button::new("Add wall"))
            .clicked()
        {
            let second = if grid_size.is_some_and(|(columns, _)| columns > 1) {
                [1, 0]
            } else {
                [0, 1]
            };
            walls.push(serde_json::json!({ "first": [0, 0], "second": second }));
        }
    });
}

fn grid_occupant_section(ui: &mut egui::Ui, payload: &mut Value, grids: &[(String, String)]) {
    section_header(ui, ICON_GRID_4X4, "Occupancy");
    let current = payload
        .get("grid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut chosen = current.clone();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Grid").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            egui::ComboBox::from_id_salt("grid-occupant-grid")
                .selected_text(
                    grids
                        .iter()
                        .find(|(_, id)| *id == chosen)
                        .map_or(chosen.as_str(), |(label, _)| label.as_str()),
                )
                .width(170.0)
                .show_ui(ui, |ui| {
                    for (label, id) in grids {
                        ui.selectable_value(&mut chosen, id.clone(), label);
                    }
                });
        });
    });
    if chosen != current && !chosen.is_empty() {
        payload["grid"] = Value::String(chosen);
    }
    if grids.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Add a tilemap before authoring an occupant.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
    }

    let Some(footprint) = payload.get_mut("footprint").and_then(Value::as_array_mut) else {
        property_label(ui, "Footprint", "stored value is not a cell list");
        return;
    };
    let may_remove = footprint.len() > 1;
    let mut remove = None;
    for (index, cell) in footprint.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Cell {}", index + 1))
                    .size(10.0)
                    .color(TEXT_MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(7.0);
                if ui
                    .add_enabled(may_remove, egui::Button::new("Remove"))
                    .clicked()
                {
                    remove = Some(index);
                }
            });
        });
        grid_coord_row(ui, "Offset", cell);
    }
    if let Some(index) = remove {
        footprint.remove(index);
    }
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        if ui.button("Add cell").clicked() {
            let next_x = footprint
                .iter()
                .filter_map(Value::as_array)
                .filter_map(|cell| cell.first()?.as_i64())
                .max()
                .unwrap_or(-1)
                + 1;
            footprint.push(serde_json::json!([next_x, 0]));
        }
    });
    let mut seen = BTreeSet::new();
    let duplicate = footprint.iter().filter_map(Value::as_array).any(|cell| {
        let key = (
            cell.first().and_then(Value::as_i64),
            cell.get(1).and_then(Value::as_i64),
        );
        !seen.insert(key)
    });
    if duplicate {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Footprint cells must be unique offsets.")
                    .size(9.0)
                    .color(PROBLEM),
            );
        });
    }
}

/// The rows of one payload, indented under its heading.
///
/// `skip_properties` keeps a script's authored values from appearing twice:
/// they are drawn above as typed fields, from what the script declared.
fn object_rows(ui: &mut egui::Ui, type_name: &str, payload: &mut Value, skip_properties: bool) {
    let Value::Object(fields) = payload else {
        return;
    };
    // Which fields apply can depend on the others, so the decision is made
    // against the payload as it was before this frame's edits.
    let whole = Value::Object(fields.clone());
    for (key, value) in fields.iter_mut() {
        if skip_properties && key == "properties" {
            continue;
        }
        if !inspector::applies(type_name, key, &whole) {
            continue;
        }
        value_row(ui, key, value, 10.0);
    }
}

/// One field, drawn as whatever its stored shape deserves.
fn value_row(ui: &mut egui::Ui, key: &str, value: &mut Value, indent: f32) {
    let label = inspector::humanize(key);
    match inspector::value_kind(value) {
        inspector::ValueKind::Number => {
            let mut number = value.as_f64().unwrap_or_default();
            // Integers stay integers, so editing a layer does not turn `3`
            // into `3.0` and change a scene byte for byte.
            let whole = value.is_i64() || value.is_u64();
            if number_row(ui, &label, &mut number, indent, whole) {
                *value = if whole {
                    #[allow(clippy::cast_possible_truncation)]
                    Value::from(number.round() as i64)
                } else {
                    Value::from(number)
                };
            }
        }
        inspector::ValueKind::Bool => {
            let mut flag = value.as_bool().unwrap_or_default();
            if bool_row(ui, &label, &mut flag, indent) {
                *value = Value::Bool(flag);
            }
        }
        inspector::ValueKind::Text => {
            let mut text = value.as_str().unwrap_or_default().to_owned();
            if text_row(ui, &label, &mut text, indent) {
                *value = Value::String(text);
            }
        }
        inspector::ValueKind::Numbers(len) => {
            let labels = inspector::axis_labels(key, len);
            let mut numbers: Vec<f64> = value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.as_f64().unwrap_or_default())
                        .collect()
                })
                .unwrap_or_default();
            if numbers_row(ui, &label, &labels, &mut numbers, indent) {
                *value = Value::Array(numbers.into_iter().map(Value::from).collect());
            }
        }
        inspector::ValueKind::Object => {
            ui.horizontal(|ui| {
                ui.add_space(indent);
                ui.label(RichText::new(&label).size(11.0).color(TEXT_MUTED));
            });
            let Value::Object(nested) = value else {
                return;
            };
            for (key, value) in nested.iter_mut() {
                value_row(ui, key, value, indent + 12.0);
            }
        }
        // Shown as stored and left alone. A text field over a tilemap's tiles
        // or a clip table is a way to break a scene, not a way to edit one.
        inspector::ValueKind::Opaque => {
            property_label(ui, &label, &opaque_summary(value));
        }
    }
}

/// What an uneditable value says about itself.
fn opaque_summary(value: &Value) -> String {
    match value {
        Value::Null => "not set".to_owned(),
        Value::Array(items) => format!("{} items", items.len()),
        other => other.to_string(),
    }
}

/// A script's `@export` fields, drawn from what the script declared.
///
/// This is the capability that justified a statically typed language: the panel
/// knows a field exists, what it is called, what type it is, and what it starts
/// as, without running anything. A field the scene has not set shows its
/// default and says so.
fn script_exports_section(ui: &mut egui::Ui, payload: &mut Value, scripts: &SceneScripts) {
    let source = payload.get("source").and_then(Value::as_str).unwrap_or("");
    let script = payload.get("script").and_then(Value::as_str).unwrap_or("");
    let Some(exports) = scripts.exports(source, script) else {
        // Not the same as having no properties, and saying so matters: a panel
        // that showed nothing would look like a script with nothing to author.
        property_label(ui, "Properties", "waiting for the script");
        return;
    };
    if exports.is_empty() {
        property_label(ui, "Properties", "none declared");
        return;
    }

    for export in exports {
        let stored = payload
            .get("properties")
            .and_then(|properties| properties.get(&export.name))
            .cloned();
        let authored = stored.is_some();
        let mut value = stored.unwrap_or_else(|| script_value_json(&export.default));
        let label = inspector::humanize(&export.name);

        let before = value.clone();
        value_row(ui, &export.name, &mut value, 10.0);
        if value != before {
            // Setting a property is what puts it in the scene: a field left
            // alone stays absent, so a scene records the author's choices
            // rather than a copy of every default.
            let properties = payload
                .as_object_mut()
                .expect("a script component is an object")
                .entry("properties")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(properties) = properties.as_object_mut() {
                properties.insert(export.name.clone(), value);
            }
        } else if !authored {
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                ui.label(
                    RichText::new(format!(
                        "default{}",
                        export
                            .type_name
                            .as_ref()
                            .map_or_else(String::new, |name| format!(" · {name}"))
                    ))
                    .size(9.0)
                    .color(TEXT_MUTED),
                );
            });
        }
        let _ = label;
    }
}

/// A Decay value as the JSON a scene stores.
///
/// A reference stores as null, because it names a runtime handle and runtime
/// handles are never serialized: writing one to a scene would produce a file
/// that means something different the next time it is opened. An `@export` of
/// an entity is not authorable for that reason, and the inspector shows it as
/// empty rather than as a number nobody can act on.
fn script_value_json(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::Number(number) => Value::from(*number),
        ScriptValue::Bool(flag) => Value::Bool(*flag),
        ScriptValue::String(text) => Value::String(text.clone()),
        ScriptValue::Reference(_) | ScriptValue::Null | ScriptValue::Unit => Value::Null,
    }
}

/// A labelled drag, reporting whether it moved.
fn number_row(ui: &mut egui::Ui, label: &str, value: &mut f64, indent: f32, whole: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            let drag = egui::DragValue::new(value).speed(if whole { 1.0 } else { 0.01 });
            let drag = if whole { drag.fixed_decimals(0) } else { drag };
            changed = ui.add(drag).changed();
        });
    });
    changed
}

fn bool_row(ui: &mut egui::Ui, label: &str, value: &mut bool, indent: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            changed = ui.checkbox(value, "").changed();
        });
    });
    changed
}

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, indent: f32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            changed = ui
                .add(egui::TextEdit::singleline(value).desired_width(150.0))
                .changed();
        });
    });
    changed
}

/// A row of drags for a short numeric array, each under its own axis letter.
fn numbers_row(
    ui: &mut egui::Ui,
    label: &str,
    axes: &[String],
    values: &mut [f64],
    indent: f32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            for (index, value) in values.iter_mut().enumerate().rev() {
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .speed(0.01)
                            .prefix(format!("{} ", axes.get(index).map_or("", String::as_str))),
                    )
                    .changed();
            }
        });
    });
    changed
}

/// The Add Component menu, offering only what can actually be added.
///
/// Absent entirely when there is nothing to add, rather than shown disabled: an
/// entity that already has everything is not a state worth drawing a greyed-out
/// control for.
fn add_component_button(ui: &mut egui::Ui, addable: &[ComponentMetadata]) -> Option<String> {
    if addable.is_empty() {
        return None;
    }
    let mut chosen = None;
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        // Words rather than a bare "+", because an inspector has several things
        // it could plausibly be adding. Drawn like the File and View menus,
        // which is what it is.
        ui.menu_button(
            RichText::new("Add Component").size(12.0).color(TEXT),
            |ui| {
                ui.set_min_width(170.0);
                for metadata in addable {
                    if ui.button(&metadata.display_name).clicked() {
                        chosen = Some(metadata.type_name.clone());
                        ui.close();
                    }
                }
            },
        );
    });
    chosen
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

/// Three drags for a vector, with the last one optionally taken away.
///
/// `lock_z` is what a transform that declares its Z locked looks like here: the
/// number is still shown, because what layer a thing is on is worth reading
/// even when it is not yours to change.
fn vector_row(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3], lock_z: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            let locked = lock_z && index == 2;
            ui.label(
                RichText::new(["X", "Y", "Z"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_enabled_ui(!locked, |ui| {
                changed |= ui
                    .add_sized(
                        [48.0, 23.0],
                        egui::DragValue::new(value).speed(0.05).max_decimals(3),
                    )
                    .changed();
            });
        }
    });
    changed
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

/// What the editor has said, newest at the bottom.
///
/// This used to be three fixed lines, two of them interpolating a real number,
/// which made it a status readout wearing a log's clothes. The engine's state
/// is still worth a line, so it is one — at the top, marked as the standing
/// state rather than something that just happened.
///
/// Returns true when the user asked to clear it.
fn console_view(ui: &mut egui::Ui, console: &Console, state: EngineState) -> bool {
    let mut cleared = false;
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        status_dot(ui, ACCENT);
        ui.label(
            RichText::new(format!("Engine {}", lifecycle_label(state)))
                .size(11.0)
                .color(TEXT_MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if ui
                .add_enabled(
                    !console.is_empty(),
                    egui::Button::new(RichText::new("Clear").size(11.0).color(TEXT_MUTED))
                        .frame(false),
                )
                .clicked()
            {
                cleared = true;
            }
        });
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        // Pinned to the newest entry: a log you have to scroll to the bottom of
        // to see what just happened is a log nobody reads.
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for entry in console.entries() {
                console_row(ui, entry);
            }
        });
    cleared
}

fn console_row(ui: &mut egui::Ui, entry: &Entry) {
    let color = match entry.level {
        Level::Info => TEXT_MUTED,
        Level::Warning => ACCENT_BRIGHT,
        Level::Error => Color32::from_rgb(255, 138, 148),
    };
    ui.horizontal_top(|ui| {
        ui.add_space(10.0);
        status_dot(ui, color);
        // Wrapped, not truncated: an asset failure names a path and an
        // operating system error, and a line that runs off the edge of the dock
        // is a line nobody can act on.
        ui.add(egui::Label::new(RichText::new(&entry.message).size(11.0).color(color)).wrap());
        if entry.count > 1 {
            ui.label(
                RichText::new(format!("x{}", entry.count))
                    .size(10.0)
                    .color(TEXT_FAINT),
            );
        }
    });
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
/// Turns every changed component payload into a command, and says what it
/// refused.
///
/// Kept apart from the drawing of it so the claims — that an edit becomes a
/// command, and that one which breaks a schema becomes nothing — are things a
/// test can check without a window or a GPU.
///
/// A payload is written back exactly as stored, so an edit that stopped it
/// decoding would produce a scene the engine refuses to open. Checking here
/// means the author hears about it at the field they were editing rather than
/// at the next launch.
fn component_commands(
    entity: EntityId,
    original: &BTreeMap<String, Value>,
    draft: &BTreeMap<String, Value>,
    components: &ComponentSchemaRegistry,
) -> (CommandBuffer, Vec<String>) {
    let mut buffer = CommandBuffer::new();
    let mut refused = Vec::new();
    for (type_name, payload) in draft {
        if original.get(type_name) == Some(payload) {
            continue;
        }
        if let Err(error) = components.validate_payload(type_name, payload) {
            refused.push(error.to_string());
            continue;
        }
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.clone(),
            payload: payload.clone(),
        });
    }
    (buffer, refused)
}

/// The components an entity does not have and the registry can create.
///
/// A type with no default payload is missing from the list rather than offered
/// and refused: a button that adds a component the engine will then reject is
/// worse than no button, which is why the old Add Component was removed rather
/// than left drawn.
fn addable_components(
    components: &ComponentSchemaRegistry,
    present: &BTreeMap<String, Value>,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Vec<ComponentMetadata> {
    components
        .registered_components()
        .filter(|metadata| !present.contains_key(&metadata.type_name))
        .filter(|metadata| {
            metadata.type_name != GRID_NAVIGATION_COMPONENT
                || present.contains_key(tilemap::TYPE_NAME)
        })
        .filter(|metadata| {
            component_default(
                components,
                &metadata.type_name,
                first_font,
                first_sprite,
                first_grid,
            )
            .is_some()
        })
        .cloned()
        .collect()
}

/// What Add Component writes for a fresh component.
///
/// Built-ins normally own a fixed default in the registry. Text and sprite
/// animation cannot: their reproducible asset references must come from the
/// project, so their defaults are completed at the editor boundary.
fn component_default(
    components: &ComponentSchemaRegistry,
    type_name: &str,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Option<Value> {
    if type_name == GRID_OCCUPANT_COMPONENT {
        return first_grid.map(|grid| {
            serde_json::json!({
                "grid": grid,
                "footprint": [[0, 0]]
            })
        });
    }
    if type_name == TEXT_COMPONENT {
        return first_font.map(|font| {
            serde_json::json!({
                "text": "Text",
                "font": font
            })
        });
    }
    if type_name == animation::TYPE_NAME {
        return first_sprite.map(|sprite| {
            serde_json::json!({
                "clips": {
                    "clip": {
                        "frames": [sprite],
                        "seconds_per_frame": 0.1,
                        "looping": true
                    }
                },
                "playing": "clip",
                "speed": 1.0
            })
        });
    }
    components.default_payload(type_name).cloned()
}

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

#[cfg(test)]
mod tests {
    use sindri_core::{CommandHistory, SceneDocument, SceneEntity, SceneEntityId};
    // The demo scene is embedded in the example, so a test reaches it without
    // depending on the working directory. Only the tests want it: the editor
    // itself no longer loads through the example's scene type.
    use sindri_cube::DemoScene;
    use sindri_render::look_at;

    use super::*;

    fn extractor() -> SceneExtractor {
        SceneExtractor::new().unwrap()
    }

    /// The scene the editor opens with no argument, loaded the way the editor
    /// loads it.
    fn demo_world() -> World {
        load_world(&extractor(), &DemoScene::authored_document().unwrap())
            .expect("the demo scene loads")
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
    fn collapsing_a_game_object_hides_its_whole_subtree() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let root = find_by_source_id(&world, "root").unwrap();
        let rows = visible_hierarchy_rows(&world, &BTreeSet::from([root]), "");
        assert_eq!(rows, vec![(root, 0)]);
    }

    #[test]
    fn hierarchy_search_keeps_the_ancestor_path_visible() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let named = |id: &str| find_by_source_id(&world, id).unwrap();
        let collapsed = BTreeSet::from([named("root"), named("torso")]);
        let rows = visible_hierarchy_rows(&world, &collapsed, "arm");

        assert_eq!(
            rows,
            vec![(named("root"), 0), (named("torso"), 1), (named("arm"), 2)],
            "search opens only the path to the match without changing stored folds"
        );
    }

    #[test]
    fn new_game_object_ids_are_stable_and_skip_existing_ids() {
        let mut world = World::default();
        world.spawn(EntityData {
            source_id: Some(SceneEntityId::new("game-object-1").unwrap()),
            ..EntityData::default()
        });
        assert_eq!(next_game_object_id(&world).as_str(), "game-object-2");
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
    fn hierarchy_drop_rules_allow_moves_but_reject_noops_and_cycles() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let named = |id: &str| find_by_source_id(&world, id).unwrap();
        let root = named("root");
        let torso = named("torso");
        let arm = named("arm");
        let leg = named("leg");

        assert!(hierarchy_drop_allowed(&world, arm, Some(leg)));
        assert!(hierarchy_drop_allowed(&world, arm, None));
        assert!(!hierarchy_drop_allowed(&world, arm, Some(torso)));
        assert!(!hierarchy_drop_allowed(&world, arm, Some(arm)));
        assert!(!hierarchy_drop_allowed(&world, root, Some(arm)));
        assert!(!hierarchy_drop_allowed(&world, root, None));
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

    /// The whole point: a component edit reaches the world through the command
    /// layer, and undo puts it back. Until this existed, a component was a
    /// read-only label and every value was set by editing the scene file.
    #[test]
    fn a_component_edit_reaches_the_world_and_undoes_cleanly() {
        let mut world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original = world.get(entity).unwrap().components.clone();

        let mut draft = original.clone();
        draft
            .get_mut("sindri.mesh")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("layer".to_owned(), serde_json::json!(4));

        let (buffer, refused) =
            component_commands(entity, &original, &draft, extractor().components());
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(buffer.len(), 1);

        let mut history = CommandHistory::default();
        history
            .apply(buffer.into_transaction("Edit components"), &mut world)
            .unwrap();
        assert_eq!(
            world.get(entity).unwrap().components["sindri.mesh"]["layer"],
            serde_json::json!(4)
        );

        history.undo(&mut world).unwrap();
        assert_eq!(world.get(entity).unwrap().components, original);
    }

    /// An edit that would stop a component decoding never becomes a command.
    /// The payload is written back exactly as stored, so letting it through
    /// would produce a scene the engine refuses to open — discovered at the
    /// next launch rather than at the field being edited.
    #[test]
    fn an_edit_that_breaks_a_schema_is_refused_rather_than_written() {
        let world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original = world.get(entity).unwrap().components.clone();

        let mut draft = original.clone();
        draft
            .get_mut("sindri.mesh")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("primitive".to_owned(), serde_json::json!("dodecahedron"));

        let (buffer, refused) =
            component_commands(entity, &original, &draft, extractor().components());
        assert!(buffer.is_empty(), "nothing is written");
        assert_eq!(refused.len(), 1, "and the author is told why");
        assert!(refused[0].contains("sindri.mesh"), "{refused:?}");
    }

    /// A component nothing understands is still editable, which is what the
    /// preserve policy promises and could not previously deliver.
    #[test]
    fn an_unknown_component_can_still_be_edited() {
        let world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let original: BTreeMap<String, Value> =
            [("game.health".to_owned(), serde_json::json!({ "hp": 3 }))]
                .into_iter()
                .collect();
        let mut draft = original.clone();
        draft.get_mut("game.health").unwrap()["hp"] = serde_json::json!(5);

        let (buffer, refused) =
            component_commands(entity, &original, &draft, extractor().components());
        assert!(
            refused.is_empty(),
            "nothing is known about its shape, so nothing is claimed"
        );
        assert_eq!(buffer.len(), 1);
    }

    /// Add Component offers what the entity lacks and the registry can create,
    /// and nothing else.
    #[test]
    fn add_component_offers_only_what_it_can_actually_add() {
        let extractor = extractor();
        let present: BTreeMap<String, Value> = [("sindri.mesh".to_owned(), serde_json::json!({}))]
            .into_iter()
            .collect();
        let offered: Vec<String> = addable_components(
            extractor.components(),
            &present,
            Some("fonts/Inter.ttf"),
            None,
            None,
        )
        .into_iter()
        .map(|metadata| metadata.type_name)
        .collect();

        assert!(
            !offered.contains(&"sindri.mesh".to_owned()),
            "not one it already has"
        );
        assert!(offered.contains(&"sindri.sprite".to_owned()));
        assert!(offered.contains(&TEXT_COMPONENT.to_owned()));
        assert!(
            !offered.contains(&animation::TYPE_NAME.to_owned()),
            "and not one with no sensible blank, which the engine would refuse"
        );
    }

    /// Every default the registry offers has to produce a component the engine
    /// accepts, or Add Component is a button that breaks a scene.
    #[test]
    fn every_offered_default_is_one_the_engine_accepts() {
        let extractor = extractor();
        let components = extractor.components();
        for metadata in addable_components(
            components,
            &BTreeMap::new(),
            Some("fonts/Inter.ttf"),
            Some("idle"),
            Some("floor"),
        ) {
            let payload = component_default(
                components,
                &metadata.type_name,
                Some("fonts/Inter.ttf"),
                Some("idle"),
                Some("floor"),
            )
            .expect("it was offered, so it has one");
            components
                .validate_payload(&metadata.type_name, &payload)
                .unwrap_or_else(|error| {
                    panic!(
                        "the default for {} does not decode: {error}",
                        metadata.type_name
                    )
                });
        }
    }

    #[test]
    fn text_is_addable_only_when_the_project_has_a_font() {
        let extractor = extractor();
        let components = extractor.components();
        let present = BTreeMap::new();

        assert!(
            !addable_components(components, &present, None, None, None)
                .iter()
                .any(|metadata| metadata.type_name == TEXT_COMPONENT)
        );

        let payload = component_default(
            components,
            TEXT_COMPONENT,
            Some("fonts/Inter.ttf"),
            None,
            None,
        )
        .expect("a project font completes a valid text component");
        assert_eq!(payload["font"], "fonts/Inter.ttf");
        assert_eq!(payload["text"], "Text");
        components
            .validate_payload(TEXT_COMPONENT, &payload)
            .unwrap();
    }

    #[test]
    fn sprite_animation_is_addable_only_with_a_named_sheet_sprite() {
        let extractor = extractor();
        let components = extractor.components();
        let present = BTreeMap::new();

        assert!(
            !addable_components(components, &present, None, None, None)
                .iter()
                .any(|metadata| metadata.type_name == animation::TYPE_NAME)
        );

        let payload = component_default(components, animation::TYPE_NAME, None, Some("idle"), None)
            .expect("a named sheet sprite completes a valid animation component");
        assert_eq!(
            payload["clips"]["clip"]["frames"],
            serde_json::json!(["idle"])
        );
        assert_eq!(payload["playing"], "clip");
        components
            .validate_payload(animation::TYPE_NAME, &payload)
            .unwrap();
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
        let reopened =
            load_world(&extractor(), &SceneDocument::from_json(&saved).unwrap()).unwrap();
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

    /// The transport decides whether an animation moves, and nothing else does.
    #[test]
    fn only_a_running_engine_moves_an_animation_on() {
        let cap = FixedStepConfig::default().max_frame_delta.as_secs_f32();
        assert_eq!(
            animation_delta(EngineState::Running, 0.016).to_bits(),
            0.016_f32.to_bits(),
            "a running frame is worth its own length"
        );
        for state in [
            EngineState::Created,
            EngineState::Initialized,
            EngineState::Paused,
            EngineState::Stopped,
        ] {
            assert_eq!(
                animation_delta(state, 0.016).to_bits(),
                0.0_f32.to_bits(),
                "{state:?} does not move an animation on"
            );
        }
        assert_eq!(
            animation_delta(EngineState::Running, 60.0).to_bits(),
            cap.to_bits(),
            "and a minute behind another window is capped, not caught up on"
        );
        assert_eq!(
            animation_delta(EngineState::Running, f32::NAN).to_bits(),
            0.0_f32.to_bits(),
            "a frame time that is not a length of time is worth nothing"
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

    /// Presses and releases the pointer at `target`, and reports whether a
    /// hierarchy row drawn at the same place says it was clicked.
    ///
    /// egui reports a click on the release, so the press and the release are
    /// separate frames, as they are for a real pointer.
    fn hierarchy_row_click_at(offset: Vec2, has_children: bool) -> (bool, bool) {
        let context = egui::Context::default();
        // The row draws a material icon, and the icon font is registered by the
        // same call the running editor makes.
        egui_material_icons::initialize(&context);
        let row = std::cell::Cell::new(Rect::NOTHING);
        let clicked = std::cell::Cell::new(false);
        let toggled = std::cell::Cell::new(false);
        let draw = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            context
                .run_ui(input, |ui| {
                    let response = hierarchy_row(
                        ui,
                        ICON_ACCOUNT_TREE,
                        "Checker Cube",
                        false,
                        0,
                        has_children,
                        true,
                    );
                    row.set(response.select.rect);
                    clicked.set(response.select.clicked());
                    toggled.set(response.toggle.is_some_and(|response| response.clicked()));
                })
                .drop_without_applying_deltas();
        };

        draw(Vec::new());
        let target = row.get().left_center() + offset;
        let button = |pressed| egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        draw(vec![egui::Event::PointerMoved(target), button(true)]);
        draw(vec![button(false)]);
        (clicked.get(), toggled.get())
    }

    fn row_click_at(offset: Vec2) -> bool {
        hierarchy_row_click_at(offset, false).0
    }

    #[test]
    fn a_hierarchy_drag_releases_onto_another_row() {
        let world = World::from_scene(&nested_scene()).unwrap().world;
        let arm = find_by_source_id(&world, "arm").unwrap();
        let leg = find_by_source_id(&world, "leg").unwrap();
        let context = egui::Context::default();
        egui_material_icons::initialize(&context);
        let source_rect = std::cell::Cell::new(Rect::NOTHING);
        let target_rect = std::cell::Cell::new(Rect::NOTHING);
        let dropped = std::cell::Cell::new(None);
        let draw = |events: Vec<egui::Event>| {
            context
                .run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let source = hierarchy_row(ui, ICON_LABEL, "Arm", false, 0, false, false);
                        source.select.dnd_set_drag_payload(HierarchyDrag(arm));
                        source_rect.set(source.select.rect);

                        let target = hierarchy_row(ui, ICON_LABEL, "Leg", false, 0, false, false);
                        target_rect.set(target.select.rect);
                        if let Some(entity) =
                            hierarchy_drop_target(ui, &target.drop, &world, Some(leg))
                        {
                            dropped.set(Some(entity));
                        }
                    },
                )
                .drop_without_applying_deltas();
        };

        draw(Vec::new());
        let source = source_rect.get().center();
        let target = target_rect.get().center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        draw(vec![
            egui::Event::PointerMoved(source),
            button(source, true),
        ]);
        draw(vec![egui::Event::PointerMoved(target)]);
        draw(vec![button(target, false)]);

        assert_eq!(dropped.get(), Some(arm));
    }

    /// Whether an asset row reports a double click `offset` points into it.
    ///
    /// Driven through real frames for the same reason the hierarchy row is: the
    /// row is a sensing scope wrapped around labels, and whether a label
    /// swallows the click is not something reading the code answers.
    fn asset_row_double_click_at(kind: AssetKind, offset: Vec2) -> bool {
        let context = egui::Context::default();
        egui_material_icons::initialize(&context);
        let entry = ProjectEntry {
            sprites: Vec::new(),
            path: PathBuf::from("/project/level.scene.json"),
            name: "level.scene.json".to_owned(),
            relative: "level.scene.json".to_owned(),
            kind,
            depth: 0,
        };
        let row = std::cell::Cell::new(Rect::NOTHING);
        let opened = std::cell::Cell::new(false);
        let draw = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            context
                .run_ui(input, |ui| {
                    let response = asset_row(ui, &entry, 0, false, None, None);
                    row.set(response.rect);
                    opened.set(response.double_clicked());
                })
                .drop_without_applying_deltas();
        };

        draw(Vec::new());
        let target = row.get().left_center() + offset;
        let button = |pressed| egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        draw(vec![egui::Event::PointerMoved(target), button(true)]);
        draw(vec![button(false)]);
        draw(vec![button(true)]);
        draw(vec![button(false)]);
        opened.get()
    }

    /// The editor reopens where it was left, and a path on the command line
    /// still wins — the most deliberate thing anyone can say about which scene
    /// to open should not be overruled by a choice made last week.
    #[test]
    fn the_remembered_scene_is_reopened_unless_one_was_named() {
        let directory = tempfile::tempdir().unwrap();
        let write = |name: &str| {
            let path = directory.path().join(name);
            std::fs::write(
                &path,
                DemoScene::authored_document()
                    .unwrap()
                    .to_canonical_json()
                    .unwrap(),
            )
            .unwrap();
            path.display().to_string()
        };
        let remembered = write("remembered.scene.json");
        let named = write("named.scene.json");

        let (file, error) = open_scene_for(None, Some(&remembered));
        assert_eq!(error, None);
        assert_eq!(file.label(), "remembered.scene.json");

        let (file, error) = open_scene_for(Some(&named), Some(&remembered));
        assert_eq!(error, None);
        assert_eq!(
            file.label(),
            "named.scene.json",
            "an argument outranks what was remembered"
        );
    }

    /// A project can move or be deleted between launches. Refusing to open
    /// anything because of that would make a remembered path a liability.
    #[test]
    fn a_remembered_scene_that_is_gone_says_so_rather_than_opening_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("gone.scene.json");
        let (_, error) = open_scene_for(None, Some(&missing.display().to_string()));
        let error = error.expect("a scene that is not there is worth saying");
        assert!(error.contains("gone.scene.json"), "{error}");
    }

    /// Which shortcuts a key press produces, read through a real egui frame.
    fn shortcuts_for(modifiers: egui::Modifiers, key: egui::Key) -> Shortcuts {
        let context = egui::Context::default();
        let pressed = std::cell::Cell::new(Shortcuts::default());
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        context
            .run_ui(input, |ui| {
                pressed.set(ui.ctx().input_mut(shortcuts));
            })
            .drop_without_applying_deltas();
        pressed.get()
    }

    /// Where one axis ends up on screen, by name.
    fn arm(view: Mat4, axis: &str) -> Vec2 {
        axis_arms(view, 1.0)
            .into_iter()
            .find(|(_, _, label)| *label == axis)
            .map(|(offset, _, _)| offset)
            .expect("every axis is drawn")
    }

    /// The indicator has to answer the camera. It was painted at three fixed
    /// offsets, so it claimed the same orientation from every angle — the one
    /// control in the editor that was wrong rather than merely idle.
    #[test]
    fn the_axis_indicator_turns_with_the_camera() {
        let front = look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        assert!(arm(front, "X").x > 0.9, "X points across the picture");
        assert!(
            arm(front, "Y").y < -0.9,
            "Y points up it, and the screen's Y grows downwards"
        );
        assert!(
            arm(front, "Z").length() < 0.01,
            "Z points at the viewer, so it has nowhere to go on screen"
        );

        // A quarter turn to the side and the two swap: Z now lies across the
        // picture and X points at the viewer.
        let side = look_at(Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, Vec3::Y);
        // Standing on +X and facing the origin puts world +Z on the left,
        // which is where X's arm no longer is.
        assert!(
            arm(side, "Z").x < -0.9,
            "Z has taken the across-screen axis"
        );
        assert!(arm(side, "X").length() < 0.01, "and X points at the viewer");
        assert!(arm(side, "Y").y < -0.9, "up is still up");
    }

    /// An arm behind the origin is drawn under the ones in front of it, so the
    /// indicator reads as three arms in space rather than three flat lines.
    #[test]
    fn the_axis_indicator_draws_back_to_front() {
        // Looking from above and to one side, so no two arms share a depth.
        let view = look_at(Vec3::new(4.0, 3.0, 5.0), Vec3::ZERO, Vec3::Y);
        let order: Vec<&str> = axis_arms(view, AXIS_ARM)
            .iter()
            .map(|(_, _, label)| *label)
            .collect();
        let depth = |axis: Vec3| view.transform_vector3(axis).z;
        let mut expected = [
            (depth(Vec3::X), "X"),
            (depth(Vec3::Y), "Y"),
            (depth(Vec3::Z), "Z"),
        ];
        expected.sort_by(|left, right| left.0.total_cmp(&right.0));
        let expected: Vec<&str> = expected.iter().map(|(_, label)| *label).collect();
        assert_eq!(order, expected, "the nearest arm is drawn last");
    }

    /// Framing a subject puts it in the middle, which is the whole claim.
    ///
    /// Checked against the extractor rather than against the number the editor
    /// computed: the pan is worked out by reading the pan's own definition
    /// backwards, and the way that goes wrong is a sign, which only shows up by
    /// asking where the subject ended up.
    #[test]
    fn focusing_a_selection_puts_it_in_the_middle_of_the_view() {
        let extractor = extractor();
        let world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let position = Vec3::from_array(world.get(entity).unwrap().transform_3d.unwrap().position);

        // Somewhere the subject is well off centre to begin with.
        let mut pan = GlamVec2::new(0.8, -0.5);
        let view = |pan| CameraView {
            orbit: GlamVec2::new(0.4, -0.2),
            distance_scale: 1.0,
            pan,
            projection: sindri_scene::WorldProjection::Perspective,
        };
        let camera = extractor
            .world_camera(&world, view(pan))
            .unwrap()
            .expect("the demo scene has a perspective camera");
        let before = camera.view.transform_point3(position);
        assert!(
            before.x.abs() + before.y.abs() > 0.5,
            "the subject has to start off centre for this to prove anything"
        );

        pan = pan_to_centre(camera, pan, position);

        let after = extractor
            .world_camera(&world, view(pan))
            .unwrap()
            .unwrap()
            .view
            .transform_point3(position);
        assert!(
            after.x.abs() < 1.0e-4 && after.y.abs() < 1.0e-4,
            "the subject should be in the middle and is at ({}, {})",
            after.x,
            after.y
        );
    }

    /// The wheel moves the same proportion of the distance wherever it is, or
    /// the far end of a four-hundredfold range is unusable.
    #[test]
    fn zooming_is_proportional_rather_than_a_fixed_step() {
        let step = |zoom: f32| (zoom * (50.0_f32 * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
        let near = MIN_ZOOM * 2.0;
        let far = MAX_ZOOM * 0.5;
        let ratio = |zoom: f32| step(zoom) / zoom;
        assert!(
            (ratio(near) - ratio(far)).abs() < 1.0e-5,
            "one notch is {} of the distance close in and {} far out",
            ratio(near),
            ratio(far)
        );
        assert_eq!(
            step(MAX_ZOOM).to_bits(),
            MAX_ZOOM.to_bits(),
            "and it stops at the far end"
        );
    }

    /// Redo must be asked for before undo, because egui ignores an extra Shift
    /// when matching: Ctrl+Shift+Z tested against Ctrl+Z matches, so the
    /// editor's redo shortcut used to be consumed by undo and perform one.
    #[test]
    fn redo_is_not_swallowed_by_undo() {
        let redo = shortcuts_for(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        assert!(redo.redo, "Ctrl+Shift+Z must redo");
        assert!(!redo.undo, "and must not also undo");

        let undo = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::Z);
        assert!(undo.undo && !undo.redo, "Ctrl+Z is still undo");

        let also_redo = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::Y);
        assert!(
            also_redo.redo && !also_redo.undo,
            "and Ctrl+Y is still redo"
        );

        let save = shortcuts_for(egui::Modifiers::COMMAND, egui::Key::S);
        assert!(save.save && !save.undo && !save.redo);
    }

    /// The bug that made the editor read-only for a fortnight.
    ///
    /// `hierarchy_row` returned the response of the `ui.horizontal` around the
    /// button rather than the button's own. A layout is allocated with
    /// `Sense::hover`, so it answers no to `clicked` forever, and selection —
    /// which every edit in the editor is behind — could never happen. Reading
    /// the code found nothing; driving the editor found it in one click.
    #[test]
    fn clicking_a_hierarchy_row_reports_the_click() {
        assert!(
            row_click_at(Vec2::new(60.0, 0.0)),
            "clicking a row's name must select it"
        );
    }

    /// A row answers everywhere, not only on its text.
    ///
    /// The offsets walk across the indent, the icon, and the name. The middle
    /// of that range is where the icon sits, and it was a dead patch until the
    /// icon was given a sense of its own: a widget inside a click-sensing scope
    /// takes precedence over the scope, so a hover-only label swallows the
    /// click rather than passing it down.
    #[test]
    fn a_hierarchy_row_answers_across_its_whole_width() {
        for offset in [2.0_f32, 10.0, 16.0, 22.0, 30.0, 60.0, 90.0] {
            assert!(
                row_click_at(Vec2::new(offset, 0.0)),
                "a click {offset} points into the row was lost"
            );
        }
    }

    #[test]
    fn a_hierarchy_chevron_folds_without_selecting() {
        let (selected, toggled) = hierarchy_row_click_at(Vec2::new(12.0, 0.0), true);
        assert!(toggled, "the child-bearing row's chevron must fold it");
        assert!(!selected, "folding a row must not also change selection");
    }

    /// A scene row opens the scene, and answers everywhere rather than only on
    /// its text — the same complaint as the hierarchy row, in the other panel.
    ///
    /// The labels have to carry the row's sense: a widget inside a sensing
    /// scope takes precedence over the scope, and an ordinary egui label is
    /// selectable text, so it answered the double click by selecting the word
    /// "json" and the row never heard about it.
    #[test]
    fn double_clicking_a_scene_row_opens_it() {
        for offset in [2.0_f32, 10.0, 20.0, 40.0, 80.0] {
            assert!(
                asset_row_double_click_at(AssetKind::Scene, Vec2::new(offset, 0.0)),
                "a double click {offset} points into a scene row was lost"
            );
        }
    }

    /// A texture row responds, because there is now something to do with one:
    /// selecting it opens the slicer. It did not until an image had a slice.
    #[test]
    fn a_texture_row_responds_because_it_can_be_sliced() {
        assert!(asset_row_double_click_at(
            AssetKind::Texture,
            Vec2::new(40.0, 0.0)
        ));
    }

    /// A row with nothing behind it is still a listing. A script row that
    /// responded would be offering something the editor cannot do — it lists
    /// `.decay` files and cannot open one.
    #[test]
    fn a_row_with_nothing_to_open_is_a_listing() {
        for kind in [AssetKind::Script, AssetKind::Mesh, AssetKind::Other] {
            assert!(
                !asset_row_double_click_at(kind, Vec2::new(40.0, 0.0)),
                "{kind:?} has nothing behind it and should not respond"
            );
        }
    }

    /// The marker means the file and the world differ, not that something was
    /// touched. Undoing back to the saved state is being back at it.
    #[test]
    fn undoing_back_to_the_saved_state_is_not_unsaved_work() {
        let mut world = demo_world();
        let entity = find_by_source_id(&world, "checker-cube").unwrap();
        let mut history = CommandHistory::default();
        let saved_revision = history.revision();

        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetTransform3D {
            entity,
            transform: Some(Transform3D {
                position: [3.0, 0.0, 0.0],
                ..Transform3D::default()
            }),
        });
        history
            .apply(buffer.into_transaction("Move"), &mut world)
            .unwrap();
        assert_ne!(
            history.revision(),
            saved_revision,
            "an edit is unsaved work"
        );

        history.undo(&mut world).unwrap();
        assert_eq!(history.revision(), saved_revision, "and undoing it is not");
    }

    /// Every discarding action asks a question naming what it will do, so the
    /// dialog cannot say "discard?" about closing the window.
    #[test]
    fn each_discarding_action_says_what_it_is_about_to_do() {
        for action in [
            Discarding::OpenAnother,
            Discarding::OpenPath(PathBuf::from("other.scene.json")),
            Discarding::Reload,
            Discarding::Reset,
            Discarding::Close,
        ] {
            assert!(action.question().ends_with('?'), "{action:?} must ask");
            assert!(!action.verb().is_empty(), "{action:?} needs a button");
        }
        assert_ne!(
            Discarding::Close.question(),
            Discarding::Reload.question(),
            "closing and reloading are different losses"
        );
    }

    #[test]
    fn labels_are_human_readable() {
        assert_eq!(humanize("checker-cube"), "Checker Cube");
        assert_eq!(component_label("sindri.sprite"), "Sprite");
    }
}
