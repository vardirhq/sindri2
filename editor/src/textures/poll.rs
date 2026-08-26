//! Taking delivery of what was asked for.
//!
//! One function per kind, because what arrives is different in each
//! case: a texture is uploaded, a font is registered with the text
//! renderer, and a sheet renames the sprites that read it.

use sindri_assets::AssetLoadOutcome;
use sindri_render::TextRenderer;

use super::{SceneTextures, TextureNote, upload};

impl SceneTextures {
    /// Takes delivery of whatever finished, uploading and binding it.
    ///
    /// Called once a frame. The upload is here rather than in the loader
    /// because the device belongs to the host, and a loader that owned one could
    /// not be tested without one.
    pub fn poll(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: &mut TextRenderer,
    ) -> Vec<TextureNote> {
        let mut notes = self.examine_files();
        notes.extend(self.poll_sheets());
        notes.extend(self.poll_fonts(text));
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

    pub(super) fn poll_fonts(&mut self, renderer: &mut TextRenderer) -> Vec<TextureNote> {
        let mut notes = Vec::new();
        let Some(fonts) = &mut self.fonts else {
            return notes;
        };
        for outcome in fonts.poll() {
            match outcome {
                AssetLoadOutcome::Ready(id) => {
                    let Some(font) = fonts.get(&id) else {
                        continue;
                    };
                    let again =
                        renderer.bind_font(id.as_str(), font.family(), font.bytes().to_vec());
                    let message = format!("{id} ({})", font.family());
                    notes.push(if again.is_some() {
                        TextureNote::Reloaded(format!("Reloaded {message}"))
                    } else {
                        TextureNote::Loaded(format!("Loaded {message}"))
                    });
                    if let Some(watch) = self.watch.as_mut() {
                        watch.watch(&id);
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
    pub(super) fn poll_sheets(&mut self) -> Vec<TextureNote> {
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
}
