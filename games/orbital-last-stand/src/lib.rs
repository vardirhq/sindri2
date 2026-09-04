//! Playing Orbital Last Stand without a window.
//!
//! There is no game code here. Everything that decides what the game does is in
//! `assets/` — the scene, the prefabs, and the Decay — and what this module
//! does is assemble the public pieces a host assembles, in the order a host
//! runs them, so that the game can be played by a test.
//!
//! That it can be written at all is the point of it. Every type it touches is
//! one Sindri exports; if the game needed anything private, this file could not
//! be written and the authoring surface would still be incomplete.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sindri_core::{
    ComponentSchemaRegistry, PREFAB_SUFFIX, PrefabDocument, Rng, SaveStore, SceneDocument, World,
};
use sindri_decay::{
    Physics2d, PrefabSources, ScriptComponent, ScriptFrame, ScriptSources, Scripts,
};
use sindri_platform::InputState;
use sindri_scene::{Effects2d, SceneExtractor, ScenePhysics2d, ScreenExtent, ScreenUi};

/// Where the project is, from wherever the harness is being run.
#[must_use]
pub fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// One run of the game, held together exactly as a host holds it.
pub struct Run {
    pub world: World,
    pub components: ComponentSchemaRegistry,
    pub scripts: Scripts,
    pub sources: ScriptSources,
    pub prefabs: PrefabSources,
    pub physics: ScenePhysics2d,
    pub screen_ui: ScreenUi,
    pub effects: Effects2d,
    pub random: Rng,
    pub saves: SaveStore,
    pub input: InputState,
    /// The viewport the overlay is laid out against. A phone in portrait and a
    /// desktop window differ only in this number.
    pub viewport: (f32, f32),
    pub elapsed: f32,
}

impl Run {
    /// Opens the project: its scene, every script the scene runs, and every
    /// prefab those scripts can spawn.
    ///
    /// # Errors
    /// If the project will not read, will not parse, or will not load.
    pub fn open() -> Result<Self, String> {
        Self::open_from(&project().join("assets"), "orbital.scene.json")
    }

    /// Opens a build rather than a source tree.
    ///
    /// Everything is read by the logical ID the manifest names, out of the
    /// content-hashed directory it names — which is what a browser does, and
    /// the reason this can say whether an export is playable rather than only
    /// whether its bytes arrived.
    ///
    /// # Errors
    /// If the build will not read, will not parse, or will not load.
    pub fn open_export(root: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(root.join("assets/sindri.manifest.json"))
            .map_err(|error| format!("no manifest: {error}"))?;
        let manifest: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let content_root = manifest
            .get("content_root")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let assets = root.join("assets").join(content_root);
        let scene = manifest
            .get("assets")
            .and_then(serde_json::Value::as_object)
            .and_then(|assets| {
                assets
                    .iter()
                    .find(|(_, entry)| {
                        entry.get("kind").and_then(serde_json::Value::as_str) == Some("scene")
                    })
                    .map(|(id, _)| id.clone())
            })
            .ok_or_else(|| "the manifest names no scene".to_owned())?;
        Self::open_from(&assets, &scene)
    }

    fn open_from(root: &Path, scene: &str) -> Result<Self, String> {
        let scene_path = root.join(scene);
        let text =
            std::fs::read_to_string(&scene_path).map_err(|e| format!("{scene_path:?}: {e}"))?;
        let document: SceneDocument = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        document.validate().map_err(|e| e.to_string())?;

        let extractor = SceneExtractor::new().map_err(|e| e.to_string())?;
        let mut components = extractor.components().clone();
        components
            .register::<ScriptComponent>("Script")
            .map_err(|e| e.to_string())?;

        let world = World::from_scene(&document)
            .map_err(|e| e.to_string())?
            .world;

        // Read from the directory rather than listed here, because a list here
        // is the thing that makes adding an enemy mean editing Rust.
        let mut sources = ScriptSources::new();
        for entry in std::fs::read_dir(root.join("scripts")).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|e| e == "decay") {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                sources.insert(format!("scripts/{name}"), text);
            }
        }
        let mut prefabs = PrefabSources::new();
        for entry in std::fs::read_dir(root.join("prefabs")).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if name.ends_with(PREFAB_SUFFIX) {
                let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let prefab =
                    PrefabDocument::from_json(&text).map_err(|e| format!("{name}: {e}"))?;
                prefabs.insert(format!("prefabs/{name}"), prefab);
            }
        }

        Ok(Self {
            world,
            components,
            scripts: Scripts::new(),
            sources,
            prefabs,
            physics: ScenePhysics2d::top_down().map_err(|e| e.to_string())?,
            screen_ui: ScreenUi::default(),
            effects: Effects2d::default(),
            random: Rng::default(),
            saves: SaveStore::default(),
            input: InputState::default(),
            viewport: (1280.0, 720.0),
            elapsed: 0.0,
        })
    }

    /// Every script failure this pass reported, in the order they happened.
    ///
    /// A step returns them rather than ignoring them, because a game whose
    /// scripts are quietly failing looks from the outside like a game that is
    /// simply not doing very much.
    pub fn step(&mut self, delta: f32) -> Vec<String> {
        let step = Duration::from_secs_f32(delta);
        let mut notes = Vec::new();
        if let Err(error) = self.physics.step(&mut self.world, &self.components, step) {
            notes.push(error.to_string());
        }
        if let Err(error) = self.screen_ui.update(
            &self.world,
            &self.components,
            ScreenExtent::new(self.viewport.0, self.viewport.1),
            self.input.presses(),
        ) {
            notes.push(error.to_string());
        }
        self.effects.advance(step);

        let (physics, events) = self.physics.for_scripts();
        let report = self.scripts.advance(
            &mut self.world,
            &self.components,
            ScriptFrame::new(&self.sources, &self.input, delta)
                .with_prefabs(&self.prefabs)
                .with_screen_ui(&self.screen_ui)
                .with_random(&mut self.random)
                .with_saves(&mut self.saves)
                .with_effects(&mut self.effects)
                .with_physics(Physics2d {
                    world: physics,
                    events,
                }),
        );
        for failure in &report.failures {
            notes.push(failure.to_string());
        }
        self.scripts.take_audio_commands();
        self.elapsed += delta;
        // At the end rather than the beginning, because an edge is delivered by
        // the step that follows the event: clearing at the top of a step would
        // wipe the press that had just been reported and never happened.
        self.input.begin_frame(step);
        notes
    }

    /// What a script left on the shared board.
    #[must_use]
    pub fn board(&self, name: &str) -> f32 {
        narrow(self.scripts.blackboard().get(name, 0.0))
    }

    /// How many entities carry a tag, which is how the game names its groups.
    #[must_use]
    pub fn count(&self, tag: &str) -> usize {
        self.world
            .entities()
            .filter(|(entity, _)| {
                self.world.is_active(*entity)
                    && self
                        .components
                        .get::<sindri_core::TagsComponent>(&self.world, *entity)
                        .ok()
                        .flatten()
                        .is_some_and(|tags| tags.has(tag))
            })
            .count()
    }

    /// The entity a scene named, which is how the harness reaches one.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<sindri_core::EntityId> {
        self.world
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some(name))
            .map(|(entity, _)| entity)
    }

    /// The extractor the harness loaded, for a caller that wants to draw a
    /// frame rather than only step one.
    #[must_use]
    pub fn scene_extractor(&self) -> SceneExtractor {
        SceneExtractor::new().expect("the components registered when the run opened")
    }

    /// Every texture the scene names, so a caller can load them off disk.
    #[must_use]
    pub fn referenced_textures(&self) -> Vec<String> {
        sindri_scene::referenced_textures(&self.world)
            .into_iter()
            .collect()
    }

    /// Every font the scene names.
    #[must_use]
    pub fn referenced_fonts(&self) -> Vec<String> {
        sindri_scene::referenced_fonts(&self.world)
            .into_iter()
            .collect()
    }

    /// What a bar is filled to, as the scene now holds it.
    #[must_use]
    pub fn fill(&self, entity: sindri_core::EntityId) -> Option<f32> {
        self.world
            .get(entity)?
            .components
            .get("sindri.ui.image")?
            .get("fill")?
            .get("amount")?
            .as_f64()
            .map(narrow)
    }

    /// The template a label is drawing — the words, which the scene owns.
    #[must_use]
    pub fn text(&self, entity: sindri_core::EntityId) -> Option<String> {
        Some(
            self.world
                .get(entity)?
                .components
                .get("sindri.ui.text")?
                .get("text")?
                .as_str()?
                .to_owned(),
        )
    }

    /// The numbers a script has filled a label's slots with.
    #[must_use]
    pub fn values(&self, entity: sindri_core::EntityId) -> Option<Vec<f32>> {
        Some(
            self.world
                .get(entity)?
                .components
                .get("sindri.ui.text")?
                .get("values")?
                .as_array()?
                .iter()
                .filter_map(|value| value.as_f64().map(narrow))
                .collect(),
        )
    }

    /// Puts a number on the shared board, as a script would.
    ///
    /// For a test that wants to reach an ending without playing until it
    /// happens — the same write the game makes when a hull runs out.
    pub fn set_board(&mut self, name: &str, value: f32) {
        self.scripts.blackboard_mut().set(name, f64::from(value));
    }

    /// The names of the active entities carrying a tag.
    ///
    /// How the harness finds out which three upgrades were offered, without
    /// knowing which three they would be — the point of the catalog being
    /// entities is that nothing knows in advance.
    #[must_use]
    pub fn active_named(&self, tag: &str) -> Vec<String> {
        self.world
            .entities()
            .filter(|(entity, _)| {
                self.world.is_active(*entity)
                    && self
                        .components
                        .get::<sindri_core::TagsComponent>(&self.world, *entity)
                        .ok()
                        .flatten()
                        .is_some_and(|tags| tags.has(tag))
            })
            .filter_map(|(_, data)| data.name.clone())
            .collect()
    }

    /// Where an element is, in the pixels a host reports.
    ///
    /// The overlay is two tall and centred on the origin; a host has already
    /// done this conversion by the time a person has touched anything, and the
    /// harness has to do it too because it is standing in for one.
    fn screen_point(&mut self, name: &str) -> (f32, f32) {
        let entity = self
            .find(name)
            .unwrap_or_else(|| panic!("no entity named {name}"));
        // A person clicks a thing they can see, and a screen switched on during
        // a script pass is laid out by a later one — the pass that places
        // elements has already run by the time the script showing them does. So
        // this waits for the element to have a place on the screen rather than
        // assuming how many frames that takes, which is a number that changes
        // whenever a screen gains a layout.
        let mut waited = 0;
        while self.screen_ui.rect(entity).is_none() && waited < 8 {
            self.step(1.0 / 60.0);
            waited += 1;
        }
        let rect = self
            .screen_ui
            .rect(entity)
            .unwrap_or_else(|| panic!("{name} never got a place on the screen"));
        let (width, height) = self.viewport;
        let half_width = width / height;
        (
            (rect.center[0] / half_width * 0.5 + 0.5) * width,
            (0.5 - rect.center[1] * 0.5) * height,
        )
    }

    /// Puts the mouse pointer over an element.
    fn point_at(&mut self, name: &str) {
        let (x, y) = self.screen_point(name);
        self.input
            .apply(sindri_platform::InputEvent::PointerMoved { x, y });
    }

    /// Presses and releases on an element, which is what a click is.
    pub fn click(&mut self, name: &str) {
        self.point_at(name);
        self.input.apply(sindri_platform::InputEvent::ButtonPressed(
            sindri_platform::MouseButton::Left,
        ));
        self.step(1.0 / 60.0);
        self.input
            .apply(sindri_platform::InputEvent::ButtonReleased(
                sindri_platform::MouseButton::Left,
            ));
        self.step(1.0 / 60.0);
        // One more, because a button leaves a number on the board and whoever
        // reads it may already have run this frame. A click landing on the
        // frame after it is made is what every script here expects.
        self.step(1.0 / 60.0);
    }

    /// Taps an element with a finger, which is what a touch device sends.
    ///
    /// Not the same events as `click`, and worth its own path: a finger
    /// carries its own position and then stops existing, where a mouse has a
    /// position all along and keeps it after the button comes up. The audit
    /// line this serves says "mouse or touch", and only the mouse half was
    /// ever played.
    pub fn tap(&mut self, name: &str) {
        let (x, y) = self.screen_point(name);
        self.input
            .apply(sindri_platform::InputEvent::TouchStarted { id: 1, x, y });
        self.step(1.0 / 60.0);
        self.input
            .apply(sindri_platform::InputEvent::TouchEnded { id: 1 });
        self.step(1.0 / 60.0);
        // As in `click`: the frame after is when whoever reads the board sees
        // it.
        self.step(1.0 / 60.0);
    }

    /// Holds a key down, as a host reports one.
    pub fn hold(&mut self, key: sindri_platform::Key) {
        self.input
            .apply(sindri_platform::InputEvent::KeyPressed(key));
    }

    /// Lets a key up.
    pub fn let_go(&mut self, key: sindri_platform::Key) {
        self.input
            .apply(sindri_platform::InputEvent::KeyReleased(key));
    }
}

/// The one place an `f64` becomes the `f32` the engine stores.
///
/// Decay holds a double and a transform holds a single, so every number
/// crossing back out is narrowed — here rather than at each call, so there is
/// one place to look at what that costs.
#[allow(clippy::cast_possible_truncation)]
fn narrow(value: f64) -> f32 {
    value as f32
}
