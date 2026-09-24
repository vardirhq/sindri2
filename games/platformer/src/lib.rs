//! Playing the platformer without a window.
//!
//! There is no game code here. What the game does is in `assets/`, the scene
//! and its Decay, and this module assembles the public pieces a host
//! assembles, in the order a host runs them, so a test can play it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sindri_core::{ComponentSchemaRegistry, EntityId, SceneDocument, World};
use sindri_decay::{Physics2d, ScriptComponent, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::{InputEvent, InputState, Key};
use sindri_scene::{SceneExtractor, ScenePhysics2d, SpriteAnimations};

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
    /// No gravity of its own: the scene's Physics 2D World says which way is
    /// down, which is what this proves the scene can do.
    pub physics: ScenePhysics2d,
    pub animations: SpriteAnimations,
    pub input: InputState,
}

impl Run {
    /// Opens the project: its scene and every script in it.
    ///
    /// # Errors
    /// If the project will not read, will not parse, or will not load.
    pub fn open() -> Result<Self, String> {
        let root = project().join("assets");
        let text = std::fs::read_to_string(root.join("platformer.scene.json"))
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
        for entry in std::fs::read_dir(root.join("scripts")).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "decay")
            {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
                sources.insert(format!("scripts/{name}"), text);
            }
        }

        Ok(Self {
            world,
            components,
            scripts: Scripts::new(),
            sources,
            physics: ScenePhysics2d::top_down().map_err(|error| error.to_string())?,
            animations: SpriteAnimations::new(),
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
        let (physics, events) = self.physics.for_scripts();
        let report = self.scripts.advance(
            &mut self.world,
            &self.components,
            ScriptFrame::new(&self.sources, &self.input, delta)
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
        sindri_scene::update_camera_behaviors(&mut self.world, delta);
        self.input.begin_frame(step);
        notes
    }

    /// Holds or lets go of a key, as the window would.
    pub fn key(&mut self, key: Key, down: bool) {
        self.input.apply(if down {
            InputEvent::KeyPressed(key)
        } else {
            InputEvent::KeyReleased(key)
        });
    }

    /// What a script left on the shared board.
    #[must_use]
    pub fn board(&self, name: &str) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        let value = self.scripts.blackboard().get(name, 0.0) as f32;
        value
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
}
