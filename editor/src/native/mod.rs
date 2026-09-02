//! The native editor shell.
//!
//! `EditorApp` is the whole of the editor's state; everything else here is one
//! region of the window or one thing the user can do to that state. The
//! submodules hold the work, this file holds what they all share: the state
//! itself, the constants they agree on, and how the window is opened.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;
use glam::Vec2 as GlamVec2;
use sindri_core::{CommandHistory, EngineLifecycle, EntityId, SceneComponent, Transform3D, World};
use sindri_decay::ScriptComponent;
use sindri_scene::{
    AudioSourceComponent, CameraComponent, GridNavigationComponent, GridOccupantComponent,
    SceneExtractor, ScenePhysics2d, ScreenUi, SpriteAnimations, SpriteComponent, UiImageComponent,
    UiTextComponent,
};

use crate::audition::Audition;
use crate::preview::TextPreview;
use crate::selection::Selection;
use crate::typeface::Typeface;
use crate::{
    animation::AnimationTool,
    console::Console,
    gizmo::{GizmoDrag, GizmoMode, GizmoSpace},
    input::EditorInput,
    preferences::Preferences,
    project::{Launch, ProjectTree, launch},
    scene_file::SceneFile,
    scripts::SceneScripts,
    slicer::Slicer,
    textures::SceneTextures,
    tilemap::TilemapTool,
};

mod camera;
mod chrome;
mod console_view;
mod editing;
mod frame;
mod hierarchy;
mod history_view;
mod inspector_panel;
mod overlay;
mod pointer;
mod preview_view;
mod project_open;
mod project_panel;
mod runtime;
mod scene_io;
mod scene_new;
mod shortcuts;
mod slicer_view;
mod tools;
mod unsaved;
mod viewport;
mod welcome;

#[cfg(test)]
mod tests;

// The scene helpers are this module's public API: `tests/` and anything else
// outside the editor reaches them here, not at the file they happen to live in.
pub use scene_io::{load_world, scene_extractor};

use project_panel::state::BrowserState;
use runtime::initialized_lifecycle;
use scene_io::open_named_scene;
use unsaved::Discarding;
use viewport::{RuntimeViewport, SceneRenderers};

/// Which of the editor's two selections the keys act on.
///
/// Set by choosing something rather than by clicking a panel: picking an entity
/// means the keys mean that entity, and picking a file means they mean the
/// file. That is what a selection already communicates, and it needs no
/// separate notion of which panel has focus.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Focus {
    #[default]
    Hierarchy,
    Project,
}

const CAMERA_COMPONENT: &str = CameraComponent::TYPE_NAME;
const SPRITE_COMPONENT: &str = SpriteComponent::TYPE_NAME;
const UI_IMAGE_COMPONENT: &str = UiImageComponent::TYPE_NAME;
const UI_TEXT_COMPONENT: &str = UiTextComponent::TYPE_NAME;
const GRID_NAVIGATION_COMPONENT: &str = GridNavigationComponent::TYPE_NAME;
const GRID_OCCUPANT_COMPONENT: &str = GridOccupantComponent::TYPE_NAME;
const AUDIO_SOURCE_COMPONENT: &str = AudioSourceComponent::TYPE_NAME;
const SCRIPT_COMPONENT: &str = ScriptComponent::TYPE_NAME;
const INITIAL_VIEWPORT_WIDTH: u32 = 960;
const INITIAL_VIEWPORT_HEIGHT: u32 = 540;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sindri Editor")
            .with_inner_size([1_440.0, 1_024.0])
            .with_min_inner_size([1_100.0, 720.0])
            // Hidden until there is something to show. A launch that opens the
            // welcome window would otherwise flash an empty editor up behind
            // it, and the first frame is where the editor learns which of the
            // two this launch is: the preferences it decides from live in
            // eframe's storage, which does not exist until the app is built.
            //
            // eframe paints a hidden window directly, ten times a second, so
            // that a `Visible` command still reaches it. That is what makes
            // this safe rather than a window that can never be shown again.
            .with_visible(false),
        ..Default::default()
    };
    eframe::run_native(
        "Sindri Editor",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)))),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTab {
    Scene,
    Game,
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
    /// Which entities the editor is pointing at, and which of them the
    /// inspector and the gizmo are about.
    selection: Selection,
    /// What a gizmo drag moves besides its own entity, and where each of them
    /// started. Empty unless a drag on a multiple selection is in progress.
    gizmo_followers: Vec<(EntityId, Transform3D)>,
    /// The entity whose name is being typed into, and the draft.
    ///
    /// Renaming lives on the hierarchy row rather than in a dialog, so this is
    /// the row that has turned into a text field. Editor state, never scene
    /// state: an abandoned rename changes nothing.
    renaming: Option<EntityId>,
    rename_draft: String,
    /// The stable ID being typed into the inspector, and whose it is.
    ///
    /// Held across frames because a stable ID is not written as it is typed:
    /// renaming `orb-1` to `player` passes through `p`, `pl`, `pla`, each of
    /// which would be a real identity written into the world and into every
    /// component pointing at it.
    id_edit: Option<(EntityId, String)>,
    /// The scene's name being typed, for the same reason.
    scene_name_edit: Option<String>,
    /// The file the inspector is showing the contents of.
    ///
    /// Beside the slicer rather than inside it: an image and a script are both
    /// "a file the inspector is showing instead of an entity", but what the
    /// panel does with them has nothing in common.
    preview: Option<TextPreview>,
    /// The clip the inspector is offering to play, and the device that plays it.
    heard: Option<PathBuf>,
    audition: Audition,
    /// The font the inspector is showing a sample of.
    shown_font: Option<PathBuf>,
    typeface: Typeface,
    /// The asset being renamed in the project browser, and the name so far.
    asset_rename: Option<(PathBuf, String)>,
    /// Which panel the keys that act on "the selection" mean.
    ///
    /// The editor holds two selections — an entity and an asset — and Delete
    /// has to act on one of them. Without this it acted on the entity always,
    /// so a project row's menu could offer a key that did something else to
    /// something else.
    focus: Focus,
    /// The file a delete is waiting to be confirmed for.
    ///
    /// A disk write has no undo behind it, so this is the one browser action
    /// that stops to ask.
    deleting: Option<PathBuf>,
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
    /// Where the project browser is looking: its selection, the folder it is
    /// scoped to, and what it has folded away.
    ///
    /// Not remembered across launches: which folder you were looking inside is
    /// about the minute rather than the project, unlike the panel sizes beside
    /// it in `Preferences`.
    browser: BrowserState,
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
    /// The physics Play steps, and the bodies a scene's colliders became.
    ///
    /// No gravity: the engine has no opinion about which way is down, and a
    /// scene-level setting is a project-format field that arrives with the
    /// feature that reads it. `docs/physics.md` has the open item.
    physics: ScenePhysics2d,
    /// Where the screen elements are and what the pointer is doing to them.
    ///
    /// Recomputed every frame from the world, so a button moved in the
    /// inspector is pressable where it now is rather than where it was.
    screen_ui: ScreenUi,
    /// The run's random stream.
    ///
    /// Put back to its seed every time Play starts, so pressing Play twice
    /// gives the same run twice. That is what makes a bug found in Play a bug
    /// that can be found again, and it is the opposite of what a shipped game
    /// wants — which is why a game seeds itself instead.
    random: sindri_core::Rng,
    /// Where the Game view was drawn last frame, in window points.
    ///
    /// Kept because scripts advance before the layout runs, so the rectangle a
    /// pointer is made relative to is the previous frame's. `None` until the
    /// view has been drawn once, and while the workspace is showing the Scene
    /// view instead — a pointer has nowhere to be when the game is not on
    /// screen.
    game_view_rect: Option<egui::Rect>,
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
    /// The welcome window, while it is open.
    ///
    /// Behind an `Arc<Mutex<_>>` because it is drawn by a viewport callback
    /// that egui requires to be `Send + Sync`, and read back here every frame.
    welcome: Option<Arc<Mutex<welcome::Welcome>>>,
    /// Whether the editor's own window has been revealed.
    ///
    /// It starts hidden, so this is also the answer to "is the editor what the
    /// user is looking at" — which decides whether closing the welcome window
    /// closes the editor or just the window.
    window_shown: bool,
    /// The root of the open project, when a project is open.
    ///
    /// A scene can still be edited without one — that is what the editor did
    /// before projects existed, and what a scene named on the command line
    /// outside any project still does.
    open_project_root: Option<PathBuf>,
    /// What the open project calls itself, for the browser's header.
    project_name: Option<String>,
    /// The scene the open project opens on, as its manifest last said.
    ///
    /// Kept beside the name because the browser reads both every frame it
    /// draws, and a manifest read per frame is a file read at the rate a
    /// viewport redraws. The editor is the only thing that changes it.
    project_main_scene: Option<PathBuf>,
}

impl EditorApp {
    /// What this launch opens, before anything else is built.
    ///
    /// Its own step because deciding is one thing and constructing is another,
    /// and the constructor had grown past what a reader can hold.
    fn opening(preferences: &Preferences) -> (Launch, SceneFile, Option<String>) {
        // Only a scene opens a file here — opening a project needs the editor
        // that this is building.
        let decided = launch::decide(
            std::env::args().nth(1).as_deref(),
            preferences.recent_projects.most_recent(),
            preferences.open_last_project,
        );
        let (file, open_error) = match &decided {
            Launch::Scene(path) => open_named_scene(&path.display().to_string()),
            Launch::Project(_) | Launch::Welcome => (
                SceneFile::detached(sindri_core::SceneDocument::default()),
                None,
            ),
        };
        (decided, file, open_error)
    }

    fn new(context: &eframe::CreationContext<'_>) -> Self {
        crate::ui::theme::install(&context.egui_ctx);
        let preferences = Preferences::load(context.storage);
        let scene = scene_extractor();
        let (decided, file, open_error) = Self::opening(&preferences);
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
        let selection = Selection::default();
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
            gizmo_followers: Vec::new(),
            renaming: None,
            rename_draft: String::new(),
            id_edit: None,
            scene_name_edit: None,
            preview: None,
            heard: None,
            audition: Audition::default(),
            shown_font: None,
            typeface: Typeface::default(),
            asset_rename: None,
            focus: Focus::Hierarchy,
            deleting: None,
            history: CommandHistory::default(),
            search: String::new(),
            asset_search: String::new(),
            slicer: None,
            tilemap_tool: TilemapTool::default(),
            animation_tool: AnimationTool::default(),
            browser: BrowserState::default(),
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
            gizmo_drag: None,
            renderers,
            render_state: state_for_textures,
            textures,
            textured_revision: 0,
            scene_viewport,
            game_viewport,
            game_view_rect: None,
            physics: ScenePhysics2d::top_down().expect("zero gravity is finite"),
            screen_ui: ScreenUi::default(),
            random: sindri_core::Rng::default(),
            animations: SpriteAnimations::new(),
            scripts: SceneScripts::for_scene(None),
            input: EditorInput::default(),
            play_snapshot: None,
            notice: open_error.or(load_error),
            render_error: None,
            console: Console::default(),
            title: String::new(),
            welcome: None,
            window_shown: false,
            open_project_root: None,
            project_name: None,
            project_main_scene: None,
        };
        // Said after the field is built rather than during it, because what
        // there is to say is read off the world and the bindings.
        if let Some(failure) = app.notice.clone() {
            app.console.error(failure);
        }
        app.arrange_for(decided);
        let notes = app.textures.request(&app.world, &mut app.renderers.text);
        app.record_texture_notes(notes);
        app.reload_scripts();
        app.remember_open_scene();
        app
    }

    /// Puts the editor where the launch said it should be.
    ///
    /// Only a scene has been opened by the time this runs. A project is opened
    /// through the same path the welcome window opens one through, so a launch
    /// and a click arrange the editor identically rather than in two places
    /// that have to be kept agreeing.
    fn arrange_for(&mut self, decided: Launch) {
        match decided {
            Launch::Scene(path) => {
                self.announce_scene();
                self.adopt_project_for(&path);
                self.project = self.project_tree();
            }
            Launch::Project(root) => self.open_project_at(&root),
            Launch::Welcome => self.open_welcome(),
        }
    }
}
