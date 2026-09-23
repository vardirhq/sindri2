//! The engine's own assets, bound before anything a scene names is asked for.
//!
//! Beside the procedural textures and for the same reason: a `builtin:`
//! reference is not a file, so no loader will fetch it, and a host that does
//! not bind it draws the missing checker. Every scene gets them, whether or
//! not it has a directory, because a brand-new scene is exactly the one that
//! most needs blocks it did not have to draw.

use sindri_assets::{builtin_textures, builtin_tile_sets};
use sindri_render::TextureRegistry;
use sindri_scene::{TextureBindings, TileSetBindings};

use super::upload;

pub(super) fn bind_builtins(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    registry: &mut TextureRegistry,
    bindings: &mut TextureBindings,
    tile_sets: &mut TileSetBindings,
) {
    // Nothing here can fail except a broken build, which sindri-assets' own
    // tests refuse. Skipping what would not bind keeps the editor opening
    // even then, with the missing checker where the art should be.
    for texture in builtin_textures() {
        let Ok(asset) = texture.decode() else {
            continue;
        };
        let Ok(uploaded) = upload(device, queue, texture.reference, &asset) else {
            continue;
        };
        bindings.bind(texture.reference, registry.insert(uploaded));
        if let Some(Ok(sheet)) = texture.sheet() {
            let _ = bindings.bind_sheet(texture.reference, &sheet);
        }
    }
    for (reference, tile_set) in builtin_tile_sets().flatten() {
        let _ = tile_sets.bind(reference, tile_set);
    }
}
