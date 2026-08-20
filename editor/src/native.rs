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
        ICON_ACCOUNT_TREE, ICON_CAMERA_ALT, ICON_CENTER_FOCUS_STRONG, ICON_CODE,
        ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_FOLDER, ICON_GRID_VIEW, ICON_IMAGE,
        ICON_OPEN_WITH, ICON_PAUSE, ICON_PLAY_ARROW, ICON_REDO, ICON_REFRESH, ICON_SEARCH,
        ICON_STOP, ICON_UNDO, ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};
use glam::{Mat4, Vec2 as GlamVec2, Vec3};
use serde_json::Value;
use sindri_core::{
    CommandBuffer, CommandHistory, EngineLifecycle, EngineState, EntityData, EntityId,
    SceneDocument, Transform3D, UnknownComponentPolicy, World, WorldCommand,
};
use sindri_cube::{
    CameraView, FrameRenderers, FrameTarget, TextureBindings, WorldProjection, demo_textures,
    encode_prepared_frame,
};
use sindri_render::{
    SpriteBatchRenderer, TextureRegistry, TexturedCubeRenderer, Viewport, ViewportTarget,
};
use sindri_scene::{
    CameraComponent, MeshComponent, MeshPrimitive, SceneExtractor, SpriteAnchor, SpriteComponent,
    SpriteSpace, ViewCamera,
};

use crate::{
    console::{Console, Entry, Level},
    // `egui::Layout` is a different thing entirely and is already in scope.
    preferences::{AssetView, BottomTab, CameraProjection, Layout as WorkspaceLayout, Preferences},
    project::{AssetKind, ProjectEntry, ProjectTree},
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
        scene: &SceneExtractor,
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
        let scene = SceneExtractor::new().expect("the built-in component schemas register");
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
            project,
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
                self.saved_revision = self.history.revision();
                self.lifecycle = initialized_lifecycle();
                self.notice = None;
                self.announce_scene();
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
    ///
    /// A texture a scene names and nothing has bound draws the magenta checker
    /// rather than failing the frame, which is the right call and also means
    /// the only way anyone finds out is being told. `unresolved_textures` has
    /// existed since bindings did and nothing asked it.
    fn announce_scene(&mut self) {
        self.console.info(format!(
            "Opened {} - {} entities",
            self.file.label(),
            self.world.len()
        ));
        let mut unresolved: Vec<String> =
            sindri_scene::unresolved_textures(&self.world, &self.renderers.bindings)
                .into_iter()
                .collect();
        unresolved.sort();
        for texture in unresolved {
            self.console.warning(format!(
                "{texture} is not bound, drawing the missing checker"
            ));
        }
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
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
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
                self.lifecycle = initialized_lifecycle();
                self.notice = None;
                self.announce_scene();
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
            _ => self.lifecycle.start(),
        };
        if let Err(error) = result {
            self.report(error.to_string());
        }
    }

    /// Ends a play session, leaving the world exactly as it is.
    ///
    /// There is nothing to restore yet because nothing runs — see
    /// `docs/editor-audit.md`. When play mode does drive the world, stopping
    /// should put back what it was before play started, and that restoration
    /// belongs here.
    fn stop_playback(&mut self) {
        if let Err(error) = self.lifecycle.stop() {
            self.report(error.to_string());
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
                panel_title(ui, "Inspector");
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
                        // An "Add Component" button used to close the panel.
                        // Nothing handled it, and adding one properly means
                        // choosing a type from the schema registry and writing
                        // a default payload through `SetComponent` — a build,
                        // not a button.
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
                    // Select, Move, Rotate, and Scale used to sit here. They
                    // highlighted, wrote an `EditorMode` nothing read, and
                    // there are no gizmos for them to drive — four buttons
                    // promising direct manipulation the editor cannot do.
                    // "Local coordinates" and "Lit shading" went the same way
                    // earlier. A button that cannot be pressed usefully costs
                    // more than the space it takes.
                    //
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
        let camera = if editing {
            self.scene_camera()
        } else {
            camera_for(tab, EditorCamera::default())
        };
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
/// The hierarchy's used to carry an "Add entity" button. Nothing handled it,
/// and creating an entity is not a button away: the world would need a spawn
/// command to make it undoable and a stable ID assigned before the scene could
/// be saved again. It comes back when both exist.
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
fn hierarchy_group(ui: &mut egui::Ui, label: &str, icon: MaterialIcon) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(icon.outlined().rich_text().size(15.0).color(TEXT_MUTED));
        ui.label(RichText::new(label).size(12.0).color(TEXT));
    });
}

/// One row of the hierarchy, reporting whether it was clicked.
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
) -> Response {
    let row = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(9.0 + hierarchy_indent(depth, 14.0));
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
                .sense(Sense::click()),
            );
            let label = ui.add(
                egui::Button::new(RichText::new(name).size(12.0).color(if selected {
                    TEXT
                } else {
                    TEXT_MUTED
                }))
                .selected(selected)
                .frame(false),
            );
            icon | label
        })
        .inner
    });
    // A scope's sense sits below the widgets inside it, so the name still
    // answers for itself and the rest of the row answers for the scope.
    row.response | row.inner
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
    vector_row(ui, "Scale", &mut transform.scale, false);
    property_label(ui, "Rotation", "Quaternion");
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

/// Three drags for a vector, with the last one optionally taken away.
///
/// `lock_z` is what a transform that declares its Z locked looks like here: the
/// number is still shown, because what layer a thing is on is worth reading
/// even when it is not yours to change.
fn vector_row(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3], lock_z: bool) {
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
                ui.add_sized(
                    [48.0, 23.0],
                    egui::DragValue::new(value).speed(0.05).max_decimals(3),
                );
            });
        }
    });
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
}

/// The icon a kind of file is drawn with.
const fn asset_icon(kind: AssetKind) -> MaterialIcon {
    match kind {
        AssetKind::Folder => ICON_FOLDER,
        AssetKind::Scene => ICON_DESCRIPTION,
        AssetKind::Texture | AssetKind::Font => ICON_IMAGE,
        AssetKind::Mesh => ICON_VIEW_IN_AR,
        AssetKind::Script => ICON_CODE,
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
    folders: bool,
    project: &ProjectTree,
    open: Option<&Path>,
) -> BrowserAction {
    if !folders {
        return asset_column(ui, search, view, project, open);
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
        ui.vertical(|ui| action = asset_column(ui, search, view, project, open));
    });
    action
}

/// The asset side of the browser: what it is showing, and how.
fn asset_column(
    ui: &mut egui::Ui,
    search: &mut String,
    view: &mut AssetView,
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
                        if asset_row(ui, entry, depth, searching, open).double_clicked() {
                            action = BrowserAction::Open(entry.path.clone());
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
fn asset_row(
    ui: &mut egui::Ui,
    entry: &ProjectEntry,
    depth: usize,
    searching: bool,
    open: Option<&Path>,
) -> Response {
    let openable = entry.kind == AssetKind::Scene;
    let highlighted = open.is_some_and(|path| path == entry.path);
    let sense = if openable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let row = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0 + hierarchy_indent(depth, 12.0));
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
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        status_dot(ui, color);
        ui.label(RichText::new(&entry.message).size(11.0).color(color));
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
        "Live WGPU viewport  ·  Drag to orbit  ·  Shift-drag to pan",
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
    fn row_click_at(offset: Vec2) -> bool {
        let context = egui::Context::default();
        // The row draws a material icon, and the icon font is registered by the
        // same call the running editor makes.
        egui_material_icons::initialize(&context);
        let row = std::cell::Cell::new(Rect::NOTHING);
        let clicked = std::cell::Cell::new(false);
        let draw = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            context
                .run_ui(input, |ui| {
                    let response = hierarchy_row(ui, ICON_ACCOUNT_TREE, "Checker Cube", false, 0);
                    row.set(response.rect);
                    clicked.set(response.clicked());
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
        clicked.get()
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
                    let response = asset_row(ui, &entry, 0, false, None);
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

    /// Everything else in the browser is a listing. A texture row that
    /// responded would be offering something the editor cannot do.
    #[test]
    fn a_row_that_is_not_a_scene_is_a_listing() {
        assert!(!asset_row_double_click_at(
            AssetKind::Texture,
            Vec2::new(40.0, 0.0)
        ));
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
