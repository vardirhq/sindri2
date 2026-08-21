//! The textures a scene draws with, and how they got here.
//!
//! Until this existed the editor bound exactly two: a generated checkerboard and
//! a badge, both handed to it by the cube example, both named by the demo scene.
//! Every other reference in every other scene drew the magenta missing checker,
//! and the console said so — truthfully, but about the wrong thing. Nothing had
//! failed to load, because nothing had been asked to load. The asset pipeline
//! that would have loaded it had no caller anywhere in the workspace.
//!
//! A scene's texture references are the statement of what it needs, and the
//! directory it lives in is where they resolve. What a reference *is* decides
//! how it is satisfied: one that parses as an `AssetId` is a file and goes
//! through `sindri-assets`; one that does not is a procedural texture the engine
//! generates. `procedural:` cannot parse as an ID because a colon is reserved in
//! one, so the two kinds cannot be confused and no rule has to be remembered.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use sindri_assets::{
    AssetLoadOutcome, AssetLoadQueueConfig, AssetLoader, AssetManifest, AssetWatch,
    FileSystemAssetSource, MANIFEST_FILE_NAME, SpriteSheetAssetDecoder, TextureAsset,
    TextureAssetDecoder,
};
use sindri_core::{AssetId, World, sheet_id_for};
use sindri_render::{Texture2D, TextureError, TextureRegistry};
use sindri_scene::{PROCEDURAL_TEXTURES, TextureBindings, referenced_sheets, referenced_textures};

/// How many texture loads run at once, and how many may be waiting.
///
/// Two workers because a scene's textures are large and few rather than small
/// and many, and sixty-four waiting because that is more than a scene the editor
/// can currently open will name. A scene that exceeds it says so rather than
/// dropping the overflow.
const QUEUE: AssetLoadQueueConfig = AssetLoadQueueConfig::new(2, 64);

/// How often the files behind a scene's textures are examined.
///
/// A second is far below the time it takes to notice an edit did not appear,
/// and far above the cost of stating a scene's worth of paths. Polling rather
/// than subscribing is deliberate — see `AssetWatch`.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Something worth telling the user about a texture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextureNote {
    /// It is on the GPU and bound.
    Loaded(String),
    /// It changed on disk and has been loaded again.
    Reloaded(String),
    /// It will not arrive, and this says why. The missing checker draws instead.
    Failed(String),
}

/// Every texture the open scene draws with.
///
/// Owns the registry and the bindings as well as the loader, because the three
/// only make sense together: a texture arrives, goes on the GPU, and becomes a
/// binding in one step. It is also what makes releasing them simple — a
/// different scene gets a new one of these, and the previous registry takes its
/// GPU textures with it when it drops.
pub struct SceneTextures {
    /// `None` when there is no directory to resolve references against — a
    /// scene that has never been saved, or one that failed to open. Procedural
    /// textures still work, because they do not come from anywhere.
    loader: Option<AssetLoader<TextureAssetDecoder>>,
    /// What the files behind the loaded textures looked like when last examined.
    ///
    /// Hot reload for native development, which is the point at which the
    /// editor stops being a thing you restart to see a texture you just saved.
    watch: Option<AssetWatch>,
    /// When the files were last examined, so the frame loop is not stating
    /// paths sixty times a second to learn nothing.
    last_examined: Instant,
    /// The sidecars saying how each sliced texture is cut.
    ///
    /// Its own loader rather than a second decoder on the first, because a
    /// sheet is a different kind of file with a different failure: a texture
    /// that will not decode is a picture problem, and a sheet that will not is
    /// a naming problem.
    sheets: Option<AssetLoader<SpriteSheetAssetDecoder>>,
    /// Which texture each sheet cuts, keyed by the sheet's own ID.
    sliced: BTreeMap<AssetId, String>,
    registry: TextureRegistry,
    bindings: TextureBindings,
}

impl SceneTextures {
    /// Builds the textures for a scene, with the engine's procedural ones
    /// already generated and bound.
    ///
    /// A scene that will not resolve — no path, or a queue that could not start
    /// its workers — still gets a registry, so the editor draws the missing
    /// checker rather than failing to open.
    pub fn for_scene(device: &wgpu::Device, queue: &wgpu::Queue, scene: Option<&Path>) -> Self {
        let mut registry = TextureRegistry::new(device, queue);
        let mut bindings = TextureBindings::new();
        for procedural in PROCEDURAL_TEXTURES {
            let texture = registry.insert(
                Texture2D::checkerboard(
                    device,
                    queue,
                    procedural.reference,
                    procedural.size,
                    procedural.cells,
                    procedural.colors,
                )
                .expect("built-in procedural texture dimensions are valid"),
            );
            bindings.bind(procedural.reference, texture);
        }
        let root = root_of(scene);
        Self {
            loader: root.as_deref().and_then(|root| {
                let loader =
                    AssetLoader::new(FileSystemAssetSource::new(root), QUEUE, TextureAssetDecoder)
                        .ok()?;
                // A project that ships a manifest gets its assets checked
                // against it. One that does not is not held to anything, which
                // is what makes the manifest a promise rather than a
                // requirement.
                Some(match manifest_beside(root) {
                    Some(manifest) => loader.with_manifest(manifest),
                    None => loader,
                })
            }),
            sheets: root.as_deref().and_then(|root| {
                AssetLoader::new(
                    FileSystemAssetSource::new(root),
                    QUEUE,
                    SpriteSheetAssetDecoder,
                )
                .ok()
            }),
            sliced: BTreeMap::new(),
            watch: root.map(AssetWatch::new),
            last_examined: Instant::now(),
            registry,
            bindings,
        }
    }

    pub const fn registry(&self) -> &TextureRegistry {
        &self.registry
    }

    pub const fn bindings(&self) -> &TextureBindings {
        &self.bindings
    }

    /// Asks for every texture the world draws with, and lets go of the rest.
    ///
    /// Called when the scene opens and again whenever an edit could have changed
    /// what it references, so pointing a mesh at another texture loads that
    /// texture rather than waiting for a reload. Asking twice for the same one
    /// costs nothing: the loader coalesces, which is what makes calling this on
    /// a whole world cheap.
    pub fn request(&mut self, world: &World) -> Vec<TextureNote> {
        let mut notes = Vec::new();
        let referenced = referenced_textures(world);
        let wanted: BTreeSet<AssetId> = referenced
            .iter()
            .filter_map(|reference| AssetId::new(reference.clone()).ok())
            .collect();

        let Self {
            loader: Some(loader),
            watch,
            registry,
            bindings,
            ..
        } = self
        else {
            // Nothing to resolve against, so anything the engine does not
            // generate is out of reach. Saying so once beats a magenta surface.
            for reference in referenced {
                if self.bindings.get(&reference).is_none() {
                    notes.push(TextureNote::Failed(format!(
                        "{reference}: the scene has no directory to load textures from"
                    )));
                }
            }
            return notes;
        };

        // Released first, so a reference an edit removed stops holding its
        // texture, and the binding goes back to resolving as missing rather
        // than to a handle nothing owns.
        for released in loader.retain(&wanted) {
            if let Some(texture) = bindings.unbind(released.as_str()) {
                registry.remove(texture);
            }
        }
        if let Some(watch) = watch.as_mut() {
            watch.retain(&wanted);
        }
        // Which texture each sheet cuts, so an arriving sheet knows what to
        // bind against and a released one knows what to unbind. Derived from
        // the world rather than remembered, because the world is what says
        // which textures are sliced at all.
        self.sliced = referenced
            .iter()
            .filter_map(|reference| {
                let id = AssetId::new(reference.clone()).ok()?;
                Some((sheet_id_for(&id)?, reference.clone()))
            })
            .collect();
        if let Some(sheets) = &mut self.sheets {
            let slices: BTreeSet<AssetId> = referenced_sheets(world)
                .iter()
                .filter_map(|reference| AssetId::new(reference.clone()).ok())
                .collect();
            let released = sheets.retain(&slices);
            for id in &slices {
                if let Err(error) = sheets.request(id.clone()) {
                    notes.push(TextureNote::Failed(format!("{id}: {error}")));
                }
            }
            for id in released {
                if let Some(texture) = self.sliced.get(&id) {
                    self.bindings.unbind_sheet(texture);
                }
            }
        }
        let Self {
            loader: Some(loader),
            bindings,
            ..
        } = self
        else {
            return notes;
        };

        for id in &wanted {
            if bindings.get(id.as_str()).is_some() {
                continue;
            }
            if let Err(error) = loader.request(id.clone()) {
                notes.push(TextureNote::Failed(format!("{id}: {error}")));
            }
        }
        // A reference that is neither a loadable ID nor something the engine
        // generates will never resolve, and the author is the only one who can
        // fix it.
        for reference in referenced {
            if AssetId::new(reference.clone()).is_err() && bindings.get(&reference).is_none() {
                notes.push(TextureNote::Failed(format!(
                    "{reference}: not a loadable asset reference, and nothing generates it"
                )));
            }
        }
        notes
    }

    /// Takes delivery of whatever finished, uploading and binding it.
    ///
    /// Called once a frame. The upload is here rather than in the loader
    /// because the device belongs to the host, and a loader that owned one could
    /// not be tested without one.
    pub fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<TextureNote> {
        let mut notes = self.examine_files();
        notes.extend(self.poll_sheets());
        let Self {
            loader: Some(loader),
            watch,
            registry,
            bindings,
            ..
        } = self
        else {
            return notes;
        };
        for outcome in loader.poll() {
            match outcome {
                AssetLoadOutcome::Ready(id) => {
                    let Some(asset) = loader.get(&id) else {
                        continue;
                    };
                    let asset_size = (asset.width(), asset.height());
                    match upload(device, queue, id.as_str(), asset) {
                        Ok(texture) => {
                            // Replaces whatever the reference resolved to
                            // before, which is what makes a reload visible: the
                            // old handle is simply no longer bound.
                            let again = bindings.bind(id.as_str(), registry.insert(texture));
                            // And the texture it replaced goes, or reloading one
                            // image fifty times would hold fifty copies of it.
                            if let Some(replaced) = again {
                                registry.remove(replaced);
                            }
                            let message = format!("{id} ({}x{})", asset_size.0, asset_size.1);
                            notes.push(if again.is_some() {
                                TextureNote::Reloaded(format!("Reloaded {message}"))
                            } else {
                                TextureNote::Loaded(format!("Loaded {message}"))
                            });
                            // Watched only once it has actually loaded, so the
                            // stamp records the file the picture is showing.
                            if let Some(watch) = watch.as_mut() {
                                watch.watch(&id);
                            }
                        }
                        Err(error) => notes.push(TextureNote::Failed(format!("{id}: {error}"))),
                    }
                }
                // The asset's own words, without the error's "while loading
                // asset" preamble repeating the name the line already starts
                // with. A console line has one dock's width to spend.
                AssetLoadOutcome::Failed(error) => notes.push(TextureNote::Failed(format!(
                    "{}: {}",
                    error.id(),
                    error.message()
                ))),
            }
        }
        notes
    }

    /// Looks at the files behind the loaded textures, at most once a second, and
    /// loads again whatever changed.
    ///
    /// This is the whole of hot reload from the editor's side. The binding is
    /// left pointing at the old texture until the new one arrives, so saving an
    /// image does not blink the scene through the missing checker on its way to
    /// showing the edit.
    /// Takes delivery of whatever sheets finished, binding each against the
    /// texture it slices.
    ///
    /// A sheet that will not load is reported and its texture stays unsliced,
    /// so every sprite naming a part of it goes unresolved and says so. That is
    /// louder than the alternative — quietly drawing the whole image, which is
    /// every sprite at once and the picture this whole change exists to stop.
    fn poll_sheets(&mut self) -> Vec<TextureNote> {
        let mut notes = Vec::new();
        let Some(sheets) = &mut self.sheets else {
            return notes;
        };
        for outcome in sheets.poll() {
            match outcome {
                AssetLoadOutcome::Ready(id) => {
                    let Some(texture) = self.sliced.get(&id).cloned() else {
                        continue;
                    };
                    let Some(sheet) = sheets.get(&id) else {
                        continue;
                    };
                    match self.bindings.bind_sheet(texture, sheet) {
                        Ok(()) => notes.push(TextureNote::Loaded(format!(
                            "{id} ({} sprites)",
                            sheet.rects().map(|rects| rects.len()).unwrap_or_default()
                        ))),
                        Err(error) => notes.push(TextureNote::Failed(error.to_string())),
                    }
                }
                AssetLoadOutcome::Failed(error) => notes.push(TextureNote::Failed(format!(
                    "{}: {}",
                    error.id(),
                    error.message()
                ))),
            }
        }
        notes
    }

    fn examine_files(&mut self) -> Vec<TextureNote> {
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
                notes.push(TextureNote::Failed(format!("{id}: {error}")));
            }
        }
        notes
    }

    /// Whether anything is still on its way.
    pub fn loading(&self) -> bool {
        self.loader
            .as_ref()
            .is_some_and(|loader| loader.outstanding() > 0)
    }
}

/// The project's manifest, if it ships one.
///
/// A manifest that will not read or will not parse is treated as absent rather
/// than fatal: it describes the assets, and an editor that refused to open a
/// scene because a file beside it was malformed would be refusing to let anyone
/// fix it.
fn manifest_beside(root: &Path) -> Option<AssetManifest> {
    let text = std::fs::read_to_string(root.join(MANIFEST_FILE_NAME)).ok()?;
    AssetManifest::from_json(&text).ok()
}

/// The directory a scene's references resolve against.
fn root_of(scene: Option<&Path>) -> Option<PathBuf> {
    scene.and_then(Path::parent).map(Path::to_path_buf)
}

/// Puts a decoded texture on the GPU.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    asset: &TextureAsset,
) -> Result<Texture2D, TextureError> {
    Texture2D::from_rgba8(
        device,
        queue,
        label,
        asset.width(),
        asset.height(),
        asset.rgba8(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule that keeps the two kinds of texture reference apart without
    /// anybody having to remember it.
    #[test]
    fn a_procedural_reference_cannot_be_mistaken_for_a_file() {
        for procedural in PROCEDURAL_TEXTURES {
            assert!(
                AssetId::new(procedural.reference).is_err(),
                "{} would be asked of the filesystem",
                procedural.reference
            );
        }
        assert!(AssetId::new("textures/badge.png").is_ok());
    }

    /// A scene's directory is where its references resolve, which is what makes
    /// the same scene file work from wherever it is checked out.
    #[test]
    fn references_resolve_against_the_scene_s_own_directory() {
        assert_eq!(
            root_of(Some(Path::new("game/levels/one.scene.json"))),
            Some(PathBuf::from("game/levels"))
        );
        assert_eq!(root_of(None), None);
    }
}
