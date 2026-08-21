//! The Decay scripts an open scene runs.
//!
//! The mirror of [`crate::textures`], and for the same reason: a scene's script
//! references are the statement of what it needs, the directory it lives in is
//! where they resolve, and `sindri-assets` is the one thing that knows how to
//! fetch a logical ID on either target. `sindri-decay` deliberately does no I/O,
//! so this is where a `.decay` file becomes text — and a text asset is all the
//! pipeline needs to know it is.
//!
//! Hot reload comes free of that arrangement. A script is watched once it has
//! loaded, a changed file is fetched again, and the next frame compiles the new
//! source because [`Scripts`] recompiles when the text it holds stops matching.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use sindri_assets::{
    AssetLoadOutcome, AssetLoadQueueConfig, AssetLoader, AssetWatch, FileSystemAssetSource,
    TextAssetDecoder,
};
use sindri_core::{AssetId, AssetStatus, ComponentSchemaRegistry, World};
use sindri_decay::{
    ScriptExport, ScriptFailure, ScriptReport, ScriptSources, Scripts, referenced_sources,
};
use sindri_platform::InputState;

/// Scripts are small and few, so one worker is enough and sixteen waiting is
/// more than a scene the editor can open will name.
const QUEUE: AssetLoadQueueConfig = AssetLoadQueueConfig::new(1, 16);

/// How often the files behind the loaded scripts are examined. The same second
/// textures use, for the same reason.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Something worth telling the author about a script.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptNote {
    Loaded(String),
    Reloaded(String),
    Failed(String),
}

/// Every script the open scene runs, and the sources behind them.
pub struct SceneScripts {
    /// `None` when there is no directory to resolve against — a scene that has
    /// never been saved, or one that failed to open.
    loader: Option<AssetLoader<TextAssetDecoder>>,
    watch: Option<AssetWatch>,
    last_examined: Instant,
    sources: ScriptSources,
    scripts: Scripts,
}

impl SceneScripts {
    pub fn for_scene(scene: Option<&Path>) -> Self {
        let root = root_of(scene);
        Self {
            loader: root.as_deref().and_then(|root| {
                AssetLoader::new(FileSystemAssetSource::new(root), QUEUE, TextAssetDecoder).ok()
            }),
            watch: root.map(AssetWatch::new),
            last_examined: Instant::now(),
            sources: ScriptSources::new(),
            scripts: Scripts::new(),
        }
    }

    /// Asks for every source the world's scripts name, and lets go of the rest.
    pub fn request(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Vec<ScriptNote> {
        let mut notes = Vec::new();
        let referenced = referenced_sources(world, components);
        let wanted: BTreeSet<AssetId> = referenced
            .iter()
            .filter_map(|reference| AssetId::new(reference.clone()).ok())
            .collect();

        let Self {
            loader: Some(loader),
            watch,
            sources,
            ..
        } = self
        else {
            for reference in referenced {
                notes.push(ScriptNote::Failed(format!(
                    "{reference}: the scene has no directory to load scripts from"
                )));
            }
            return notes;
        };

        // A reference an edit removed stops being held, and its source goes with
        // it — otherwise a renamed script would keep running from the text of
        // the file it used to be.
        for released in loader.retain(&wanted) {
            sources.remove(released.as_str());
        }
        if let Some(watch) = watch.as_mut() {
            watch.retain(&wanted);
        }
        for id in &wanted {
            if sources.get(id.as_str()).is_some() {
                continue;
            }
            if let Err(error) = loader.request(id.clone()) {
                notes.push(ScriptNote::Failed(format!("{id}: {error}")));
            }
        }
        // A reference that is not a valid asset ID will never resolve, and
        // saying so once is better than a script that silently never runs.
        for reference in &referenced {
            if AssetId::new(reference.clone()).is_err() {
                notes.push(ScriptNote::Failed(format!(
                    "{reference}: not a usable asset id"
                )));
            }
        }
        notes
    }

    /// Takes delivery of whatever finished. Called once a frame.
    pub fn poll(&mut self) -> Vec<ScriptNote> {
        let mut notes = self.examine_files();
        let Self {
            loader: Some(loader),
            watch,
            sources,
            ..
        } = self
        else {
            return notes;
        };
        for outcome in loader.poll() {
            match outcome {
                AssetLoadOutcome::Ready(id) => {
                    let Some(text) = loader.get(&id) else {
                        continue;
                    };
                    let again = sources.get(id.as_str()).is_some();
                    sources.insert(id.as_str(), text.clone());
                    notes.push(if again {
                        ScriptNote::Reloaded(format!("Reloaded {id}"))
                    } else {
                        ScriptNote::Loaded(format!("Loaded {id}"))
                    });
                    if let Some(watch) = watch.as_mut() {
                        watch.watch(&id);
                    }
                }
                AssetLoadOutcome::Failed(error) => notes.push(ScriptNote::Failed(format!(
                    "{}: {}",
                    error.id(),
                    error.message()
                ))),
            }
        }
        notes
    }

    /// Compiles what the world names, without running anything.
    ///
    /// Called every frame regardless of the transport, so a script that will
    /// not compile says so when the scene opens and the inspector can show what
    /// a script wants authored without anyone pressing Play.
    ///
    /// A source that has not arrived yet is not a failure, and saying it is
    /// puts one permanent error per scripted entity in the console for every
    /// cold open. Loading is asynchronous by design, so between the scene
    /// landing and its scripts arriving every scripted entity is briefly
    /// missing its source; the log keeps what it is told, so the count stayed
    /// up long after the scripts had compiled and run. Opening the companion
    /// game showed twelve errors against a game that was working.
    ///
    /// A source that will never arrive still reports, twice over: the loader
    /// says so when the request fails, and this says so once the asset is out
    /// of flight.
    pub fn compile(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Vec<ScriptFailure> {
        let mut failures = self.scripts.compile(world, components, &self.sources);
        failures.retain(|failure| match failure {
            ScriptFailure::MissingSource { asset, .. } => !self.is_in_flight(asset),
            _ => true,
        });
        failures
    }

    /// Whether the loader is still working on `asset`, so its absence is a
    /// moment rather than a fault.
    fn is_in_flight(&self, asset: &str) -> bool {
        let Some(loader) = self.loader.as_ref() else {
            return false;
        };
        let Ok(id) = AssetId::new(asset.to_owned()) else {
            return false;
        };
        matches!(
            loader.status(&id),
            Some(AssetStatus::Queued | AssetStatus::Loading)
        )
    }

    /// Moves every script in the world on by `delta_seconds`.
    pub fn advance(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        input: &InputState,
        delta_seconds: f32,
    ) -> ScriptReport {
        self.scripts
            .advance(world, components, &self.sources, input, delta_seconds)
    }

    /// What one script declares it wants authored, for the inspector to draw.
    ///
    /// `None` when the source has not compiled — still loading, or it will not
    /// compile at all — which the panel shows as "waiting" rather than as "no
    /// properties". Those are different, and confusing them would silently hide
    /// an author's fields.
    #[must_use]
    pub fn exports(&self, source: &str, script: &str) -> Option<Vec<ScriptExport>> {
        self.scripts.exports(source, script)
    }

    /// Forgets every running instance, keeping the loaded sources.
    ///
    /// An instance belongs to the world it was started against, so a world that
    /// was reloaded or restored needs new ones. The text does not change when
    /// the world does, and re-fetching it would be a load for nothing.
    pub fn restart(&mut self) {
        self.scripts.clear();
    }

    fn examine_files(&mut self) -> Vec<ScriptNote> {
        if self.last_examined.elapsed() < WATCH_INTERVAL {
            return Vec::new();
        }
        self.last_examined = Instant::now();
        let Self {
            loader: Some(loader),
            watch: Some(watch),
            ..
        } = self
        else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        for id in watch.changed() {
            if let Err(error) = loader.reload(&id) {
                notes.push(ScriptNote::Failed(format!("{id}: {error}")));
            }
        }
        notes
    }
}

fn root_of(scene: Option<&Path>) -> Option<PathBuf> {
    scene.and_then(Path::parent).map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, thread::sleep, time::Duration};

    use serde_json::json;
    use sindri_core::{ComponentSchemaRegistry, EntityData, SceneComponent, World};
    use sindri_decay::ScriptComponent;
    use tempfile::TempDir;

    use super::{SceneScripts, ScriptFailure};

    /// A world holding one entity that runs `source`, and a registry that knows
    /// what `sindri.script` is.
    fn scripted(source: &str) -> (World, ComponentSchemaRegistry) {
        let mut components = ComponentSchemaRegistry::default();
        components
            .register::<ScriptComponent>("Script")
            .expect("sindri.script registers once");
        let mut world = World::default();
        world.spawn(EntityData {
            name: Some("Thing".to_owned()),
            components: BTreeMap::from([(
                ScriptComponent::TYPE_NAME.to_owned(),
                json!({ "source": source, "script": "Thing" }),
            )]),
            ..EntityData::default()
        });
        (world, components)
    }

    /// A scene directory holding one script, and the scene path inside it.
    fn project(script: &str, text: &str) -> TempDir {
        let directory = TempDir::new().expect("a temporary directory");
        let scripts = directory.path().join("scripts");
        fs::create_dir_all(&scripts).expect("the scripts directory is creatable");
        fs::write(scripts.join(script), text).expect("the script is writable");
        directory
    }

    /// The bug this guards: a cold open reported one error per scripted entity
    /// for the moment between the scene landing and its scripts arriving, and
    /// the console keeps what it is told, so twelve phantom errors sat in the
    /// status bar of a game that was working.
    #[test]
    fn a_script_still_loading_is_not_an_error() {
        let directory = project("thing.decay", "script Thing {\n    fn update() {}\n}\n");
        let scene = directory.path().join("thing.scene.json");
        let (world, components) = scripted("scripts/thing.decay");

        let mut scripts = SceneScripts::for_scene(Some(&scene));
        scripts.request(&world, &components);

        // Before anything is polled the source cannot have arrived, which is
        // exactly the window the editor used to report.
        assert!(
            scripts.compile(&world, &components).is_empty(),
            "a source still in flight is not a compile failure"
        );

        // And once it lands it compiles, so the silence above was not the
        // failure being swallowed for good.
        for _ in 0..200 {
            scripts.poll();
            if scripts.compile(&world, &components).is_empty()
                && scripts.exports("scripts/thing.decay", "Thing").is_some()
            {
                return;
            }
            sleep(Duration::from_millis(10));
        }
        panic!("the script never arrived");
    }

    /// The other half: a source that will never arrive is still reported, so
    /// suppressing the in-flight case did not suppress a real typo.
    #[test]
    fn a_script_that_will_never_arrive_is_an_error() {
        let directory = project("thing.decay", "script Thing {\n    fn update() {}\n}\n");
        let scene = directory.path().join("thing.scene.json");
        let (world, components) = scripted("scripts/absent.decay");

        let mut scripts = SceneScripts::for_scene(Some(&scene));
        scripts.request(&world, &components);

        for _ in 0..200 {
            scripts.poll();
            let failures = scripts.compile(&world, &components);
            if failures
                .iter()
                .any(|failure| matches!(failure, ScriptFailure::MissingSource { .. }))
            {
                return;
            }
            sleep(Duration::from_millis(10));
        }
        panic!("a script that does not exist was never reported");
    }
}
