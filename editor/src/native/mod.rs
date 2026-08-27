//! The native editor shell.
//!
//! `EditorApp` is the whole of the editor's state; everything else here is one
//! region of the window or one thing the user can do to that state. The
//! submodules hold the work, this file holds what they all share: the state
//! itself, the constants they agree on, and how the window is opened.

use std::path::PathBuf;

use eframe::egui;
use glam::Vec2 as GlamVec2;
use sindri_core::{CommandHistory, EngineLifecycle, EntityId, SceneComponent, World};
use sindri_decay::ScriptComponent;
use sindri_scene::{
    AudioSourceComponent, CameraComponent, GridNavigationComponent, GridOccupantComponent,
    SceneExtractor, SpriteAnimations, SpriteComponent, UiImageComponent, UiTextComponent,
};

use crate::audition::Audition;
use crate::preview::TextPreview;
use crate::typeface::Typeface;
use crate::{
    animation::AnimationTool,
    console::Console,
    gizmo::{GizmoDrag, GizmoMode, GizmoSpace},
    input::EditorInput,
    preferences::Preferences,
    project::ProjectTree,
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
mod inspector_panel;
mod overlay;
mod pointer;
mod preview_view;
mod project_panel;
mod runtime;
mod scene_io;
mod scene_new;
mod shortcuts;
mod slicer_view;
mod tools;
mod unsaved;
mod viewport;

#[cfg(test)]
mod tests;

// The scene helpers are this module's public API: `tests/` and anything else
// outside the editor reaches them here, not at the file they happen to live in.
pub use scene_io::{load_world, scene_extractor};

use project_panel::state::BrowserState;
use runtime::initialized_lifecycle;
use scene_io::open_requested_scene;
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
    selection: Option<EntityId>,
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
        crate::ui::theme::install(&context.egui_ctx);
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
}
