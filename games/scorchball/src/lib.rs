//! Playing Scorchball without a window.
//!
//! There is no game code here. What the game does is in `assets/`, the scene,
//! the prefabs and the Decay, and this module assembles the public pieces a
//! host assembles, in the order a host runs them, so a test can play it with
//! pads it presses itself.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sindri_core::{
    ComponentSchemaRegistry, EntityId, PREFAB_SUFFIX, PrefabDocument, Rng, SceneDocument,
    TagsComponent, World,
};
use sindri_decay::{
    Physics2d, PrefabSources, ScriptComponent, ScriptFrame, ScriptSources, Scripts,
};
use sindri_platform::{GamepadAxis, GamepadButton, InputEvent, InputState, PadId};
use sindri_scene::{
    Effects2d, SceneExtractor, ScenePhysics2d, ScreenExtent, ScreenUi, SpriteAnimations,
};

/// Where the project is, from wherever the harness is being run.
#[must_use]
pub fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// One run of the game, held together as a host holds it.
pub struct Run {
    pub world: World,
    pub components: ComponentSchemaRegistry,
    pub scripts: Scripts,
    pub sources: ScriptSources,
    pub prefabs: PrefabSources,
    pub physics: ScenePhysics2d,
    /// The overlay's text, laid out each step so a label the engine would
    /// refuse is reported here rather than first seen in the editor.
    pub screen_ui: ScreenUi,
    pub effects: Effects2d,
    pub animations: SpriteAnimations,
    pub random: Rng,
    pub input: InputState,
}

impl Run {
    /// Opens the project: its scene, its scripts and the prefabs they spawn.
    ///
    /// # Errors
    /// If the project will not read, will not parse, or will not load.
    pub fn open() -> Result<Self, String> {
        let root = project().join("assets");
        let text = std::fs::read_to_string(root.join("scorchball.scene.json"))
            .map_err(|error| error.to_string())?;
        let document: SceneDocument =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        document.validate().map_err(|error| error.to_string())?;

        let mut components = SceneExtractor::new()
            .map_err(|error| error.to_string())?
            .components()
            .clone();
        components
            .register::<ScriptComponent>("Script")
            .map_err(|error| error.to_string())?;
        let world = World::from_scene(&document)
            .map_err(|error| error.to_string())?
            .world;

        let mut sources = ScriptSources::new();
        for (name, text) in files(&root.join("scripts"), ".decay")? {
            sources.insert(format!("scripts/{name}"), text);
        }
        let mut prefabs = PrefabSources::new();
        for (name, text) in files(&root.join("prefabs"), PREFAB_SUFFIX)? {
            let prefab =
                PrefabDocument::from_json(&text).map_err(|error| format!("{name}: {error}"))?;
            prefabs.insert(format!("prefabs/{name}"), prefab);
        }

        Ok(Self {
            world,
            components,
            scripts: Scripts::new(),
            sources,
            prefabs,
            physics: ScenePhysics2d::top_down().map_err(|error| error.to_string())?,
            screen_ui: ScreenUi::default(),
            effects: Effects2d::default(),
            animations: SpriteAnimations::new(),
            random: Rng::default(),
            input: InputState::default(),
        })
    }

    /// One fixed step, returning every failure it reported.
    pub fn step(&mut self, delta: f32) -> Vec<String> {
        let step = Duration::from_secs_f32(delta);
        let mut notes = Vec::new();
        if let Err(error) = self.physics.step(&mut self.world, &self.components, step) {
            notes.push(error.to_string());
        }
        if let Err(error) = self.screen_ui.update(
            &mut self.world,
            &self.components,
            ScreenExtent::new(1280.0, 720.0),
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
                .with_effects(&mut self.effects)
                .with_physics(Physics2d {
                    world: physics,
                    events,
                })
                .with_animations(&mut self.animations),
        );
        notes.extend(report.failures.iter().map(ToString::to_string));
        if let Err(error) = self
            .animations
            .advance(&self.world, &self.components, delta)
        {
            notes.push(error.to_string());
        }
        self.scripts.take_audio_commands();
        sindri_scene::update_camera_behaviors(&mut self.world, delta);
        self.input.begin_frame(step);
        notes
    }

    /// Plugs a pad in.
    pub fn connect(&mut self, pad: u32) {
        self.input.apply(InputEvent::GamepadConnected(PadId(pad)));
    }

    /// Unplugs a pad.
    pub fn disconnect(&mut self, pad: u32) {
        self.input
            .apply(InputEvent::GamepadDisconnected(PadId(pad)));
    }

    /// Presses or lets go of a button on a pad.
    pub fn button(&mut self, pad: u32, button: GamepadButton, down: bool) {
        let pad = PadId(pad);
        self.input.apply(if down {
            InputEvent::GamepadPressed { pad, button }
        } else {
            InputEvent::GamepadReleased { pad, button }
        });
    }

    /// Pushes a stick or pulls a trigger, in screen axes.
    pub fn axis(&mut self, pad: u32, axis: GamepadAxis, value: f32) {
        self.input.apply(InputEvent::GamepadAxisMoved {
            pad: PadId(pad),
            axis,
            value,
        });
    }

    /// What a script left on the shared board.
    #[must_use]
    pub fn board(&self, name: &str) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        let value = self.scripts.blackboard().get(name, 0.0) as f32;
        value
    }

    /// Every live entity carrying a tag.
    #[must_use]
    pub fn tagged(&self, tag: &str) -> Vec<EntityId> {
        self.world
            .entities()
            .filter(|(entity, _)| {
                self.world.is_active(*entity)
                    && self
                        .components
                        .get::<TagsComponent>(&self.world, *entity)
                        .ok()
                        .flatten()
                        .is_some_and(|tags| tags.has(tag))
            })
            .map(|(entity, _)| entity)
            .collect()
    }

    /// The entity a scene gave this stable ID.
    #[must_use]
    pub fn entity(&self, id: &str) -> Option<EntityId> {
        self.world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|source| source.as_str() == id)
            })
            .map(|(entity, _)| entity)
    }

    /// Where an entity is, in the plane.
    #[must_use]
    pub fn position(&self, entity: EntityId) -> [f32; 2] {
        self.world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .map_or([0.0, 0.0], sindri_core::Transform3D::position_2d)
    }

    /// How big an entity is drawn, across.
    #[must_use]
    pub fn scale(&self, entity: EntityId) -> f32 {
        self.world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .map_or(0.0, |transform| transform.scale[0])
    }
}

/// Every file in a folder ending in `suffix`, by name, in name order.
fn files(folder: &Path, suffix: &str) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if name.ends_with(suffix) {
            let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
            found.push((name, text));
        }
    }
    found.sort();
    Ok(found)
}
