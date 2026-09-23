//! Pictures of the project's textures for panels to draw, from the copies the
//! scene already has on the GPU.
//!
//! Nothing is read from disk here. A texture the scene draws with is already
//! uploaded, and egui can sample that upload directly; the only work is
//! registering it once and remembering which part of it each reference names.
//! A texture the scene has not loaded has no picture, and a panel draws its
//! place plainly instead.

use std::collections::BTreeMap;

use eframe::egui::{self, pos2, vec2};
use sindri_core::SpriteRef;

use sindri_scene::{PROCEDURAL_TEXTURES, TextureBindings};

use crate::project::ProjectTree;
use crate::textures::SceneTextures;
use crate::ui::widgets::cube::Picture;

/// A picture for each texture reference that has one.
pub(crate) type Pictures = BTreeMap<String, Picture>;

/// For a caller with no project to draw pictures from, such as a test.
#[cfg(test)]
pub(crate) static NO_PICTURES: Pictures = BTreeMap::new();

#[derive(Default)]
pub(super) struct Thumbnails {
    /// The bindings' generation and how many references were asked for, when
    /// the pictures were last worked out. Either changing means a texture
    /// arrived, left or changed, or the project gained a file.
    seen: Option<(u64, usize)>,
    registered: BTreeMap<sindri_render::TextureId, egui::TextureId>,
    pictures: Pictures,
}

impl Thumbnails {
    /// Works the pictures out again if anything they depend on moved.
    pub(super) fn refresh(
        &mut self,
        state: &eframe::egui_wgpu::RenderState,
        textures: &SceneTextures,
        references: &[String],
    ) {
        let bindings = textures.bindings();
        let key = (bindings.generation(), references.len());
        if self.seen == Some(key) {
            return;
        }
        self.seen = Some(key);
        let mut renderer = state.renderer.write();
        for (_, registered) in std::mem::take(&mut self.registered) {
            renderer.free_texture(&registered);
        }
        self.pictures.clear();
        for reference in references {
            let Ok(sprite) = SpriteRef::parse(reference) else {
                continue;
            };
            if bindings.get(sprite.texture()).is_none() {
                continue;
            }
            let (texture, uv) = bindings.resolve_sprite(&sprite);
            let id = *self.registered.entry(texture).or_insert_with(|| {
                renderer.register_native_texture(
                    &state.device,
                    textures.registry().get(texture).view(),
                    wgpu::FilterMode::Nearest,
                )
            });
            self.pictures.insert(
                reference.clone(),
                Picture {
                    texture: id,
                    uv: egui::Rect::from_min_size(
                        pos2(uv.x(), uv.y()),
                        vec2(uv.width(), uv.height()),
                    ),
                },
            );
        }
    }

    pub(super) const fn pictures(&self) -> &Pictures {
        &self.pictures
    }
}

/// Every texture reference the engine can actually draw.
///
/// The project's own files, plus the handful the engine generates. A procedural
/// reference is deliberately not parseable as an asset path, so a picker built
/// from the directory alone both refused to offer `procedural:checkerboard` and
/// marked the fixture's own cube as naming a texture that does not exist.
///
/// The sprites cut from the project's sheets are references too: a voxel face
/// or a sprite naming `blocks.png#stone-0` names something that draws, and a
/// list without them marked every such reference as missing.
pub(crate) fn drawable_textures(project: &ProjectTree, bindings: &TextureBindings) -> Vec<String> {
    let mut textures: Vec<String> = PROCEDURAL_TEXTURES
        .iter()
        .map(|texture| texture.reference.to_owned())
        .collect();
    textures.extend(project.textures());
    textures.extend(bindings.sprite_references());
    textures
}
