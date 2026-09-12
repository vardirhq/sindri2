//! Asking for what a scene names, once each.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

use sindri_assets::{
    AssetLoader, AssetWatch, FileSystemAssetSource, FontAssetDecoder, SpriteSheetAssetDecoder,
    TextureAssetDecoder, TileSetAssetDecoder,
};
use sindri_core::{AssetId, World, sheet_id_for};
use sindri_render::{TextRenderer, Texture2D, TextureRegistry};
use sindri_scene::{
    PROCEDURAL_TEXTURES, TextureBindings, TileSetBindings, referenced_fonts, referenced_sheets,
    referenced_textures, referenced_tile_sets, tile_set_textures,
};

use super::{QUEUE, SceneTextures, TextureNote, WATCH_INTERVAL, manifest_beside, root_of};

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
            fonts: root.as_deref().and_then(|root| {
                let loader =
                    AssetLoader::new(FileSystemAssetSource::new(root), QUEUE, FontAssetDecoder)
                        .ok()?;
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
            tile_sets: root.as_deref().and_then(|root| {
                AssetLoader::new(FileSystemAssetSource::new(root), QUEUE, TileSetAssetDecoder).ok()
            }),
            sliced: BTreeMap::new(),
            watch: root.map(AssetWatch::new),
            last_examined: Instant::now(),
            registry,
            bindings,
            tile_set_bindings: TileSetBindings::new(),
        }
    }

    pub(super) fn request_fonts(
        &mut self,
        world: &World,
        renderer: &mut TextRenderer,
    ) -> (BTreeSet<AssetId>, Vec<TextureNote>) {
        let references = referenced_fonts(world);
        let wanted: BTreeSet<AssetId> = references
            .iter()
            .filter_map(|reference| AssetId::new(reference.clone()).ok())
            .collect();
        let mut notes = Vec::new();
        let Some(fonts) = &mut self.fonts else {
            for reference in references {
                notes.push(TextureNote::Failed(format!(
                    "{reference}: the scene has no directory to load fonts from"
                )));
            }
            return (wanted, notes);
        };
        for released in fonts.retain(&wanted) {
            renderer.unbind_font(released.as_str());
        }
        for id in &wanted {
            if renderer.has_font(id.as_str()) {
                continue;
            }
            if let Err(error) = fonts.request(id.clone()) {
                notes.push(TextureNote::Failed(format!("{id}: {error}")));
            }
        }
        for reference in references {
            if AssetId::new(reference.clone()).is_err() {
                notes.push(TextureNote::Failed(format!(
                    "{reference}: not a loadable font asset reference"
                )));
            }
        }
        (wanted, notes)
    }

    /// Asks for every texture the world draws with, and lets go of the rest.
    ///
    /// Called when the scene opens and again whenever an edit could have changed
    /// what it references, so pointing a mesh at another texture loads that
    /// texture rather than waiting for a reload. Asking twice for the same one
    /// costs nothing: the loader coalesces, which is what makes calling this on
    /// a whole world cheap.
    pub fn request(&mut self, world: &World, text: &mut TextRenderer) -> Vec<TextureNote> {
        let (wanted_fonts, mut notes) = self.request_fonts(world, text);
        let (wanted_tile_sets, tile_notes) = self.request_tile_sets(world);
        notes.extend(tile_notes);
        let mut referenced = referenced_textures(world);
        for id in &wanted_tile_sets {
            if let Some(tile_set) = self.tile_set_bindings.get(id.as_str()) {
                referenced.extend(tile_set_textures(tile_set));
            }
        }
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
            let watched: BTreeSet<AssetId> = wanted
                .union(&wanted_fonts)
                .cloned()
                .chain(wanted_tile_sets.iter().cloned())
                .collect();
            watch.retain(&watched);
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

    fn request_tile_sets(&mut self, world: &World) -> (BTreeSet<AssetId>, Vec<TextureNote>) {
        let wanted: BTreeSet<AssetId> = referenced_tile_sets(world)
            .iter()
            .filter_map(|reference| AssetId::new(reference.clone()).ok())
            .collect();
        let mut notes = Vec::new();
        if let Some(tile_sets) = &mut self.tile_sets {
            for released in tile_sets.retain(&wanted) {
                self.tile_set_bindings.unbind(released.as_str());
            }
            for id in &wanted {
                if self.tile_set_bindings.get(id.as_str()).is_none()
                    && let Err(error) = tile_sets.request(id.clone())
                {
                    notes.push(TextureNote::Failed(format!("{id}: {error}")));
                }
            }
        }
        (wanted, notes)
    }

    pub(super) fn examine_files(&mut self) -> Vec<TextureNote> {
        if self.last_examined.elapsed() < WATCH_INTERVAL {
            return Vec::new();
        }
        self.last_examined = Instant::now();
        let Self {
            loader: Some(loader),
            fonts,
            tile_sets,
            watch: Some(watch),
            ..
        } = self
        else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        for id in watch.changed() {
            let result =
                if let Some(fonts) = fonts.as_mut().filter(|fonts| fonts.get(&id).is_some()) {
                    fonts.reload(&id)
                } else if let Some(tile_sets) = tile_sets
                    .as_mut()
                    .filter(|tile_sets| tile_sets.get(&id).is_some())
                {
                    tile_sets.reload(&id)
                } else {
                    loader.reload(&id)
                };
            if let Err(error) = result {
                notes.push(TextureNote::Failed(format!("{id}: {error}")));
            }
        }
        notes
    }
}