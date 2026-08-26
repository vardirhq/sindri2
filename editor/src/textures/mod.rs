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

mod poll;
mod request;

#[cfg(test)]
mod tests;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use sindri_assets::{
    AssetLoadQueueConfig, AssetLoader, AssetManifest, AssetWatch, FontAssetDecoder,
    MANIFEST_FILE_NAME, SpriteSheetAssetDecoder, TextureAsset, TextureAssetDecoder,
};
use sindri_core::AssetId;
use sindri_render::{Texture2D, TextureError, TextureRegistry};
use sindri_scene::TextureBindings;

/// How many texture loads run at once, and how many may be waiting.
///
/// Two workers because a scene's textures are large and few rather than small
/// and many, and sixty-four waiting because that is more than a scene the editor
/// can currently open will name. A scene that exceeds it says so rather than
/// dropping the overflow.
pub(super) const QUEUE: AssetLoadQueueConfig = AssetLoadQueueConfig::new(2, 64);

/// How often the files behind a scene's textures are examined.
///
/// A second is far below the time it takes to notice an edit did not appear,
/// and far above the cost of stating a scene's worth of paths. Polling rather
/// than subscribing is deliberate — see `AssetWatch`.
pub(super) const WATCH_INTERVAL: Duration = Duration::from_secs(1);

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
    /// Project font bytes used by `sindri.text` components.
    fonts: Option<AssetLoader<FontAssetDecoder>>,
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

/// The project's manifest, if it ships one.
///
/// A manifest that will not read or will not parse is treated as absent rather
/// than fatal: it describes the assets, and an editor that refused to open a
/// scene because a file beside it was malformed would be refusing to let anyone
/// fix it.
pub(super) fn manifest_beside(root: &Path) -> Option<AssetManifest> {
    let text = std::fs::read_to_string(root.join(MANIFEST_FILE_NAME)).ok()?;
    AssetManifest::from_json(&text).ok()
}

/// The directory a scene's references resolve against.
pub(super) fn root_of(scene: Option<&Path>) -> Option<PathBuf> {
    scene.and_then(Path::parent).map(Path::to_path_buf)
}

/// Puts a decoded texture on the GPU.
pub(super) fn upload(
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

impl SceneTextures {
    pub const fn registry(&self) -> &TextureRegistry {
        &self.registry
    }

    pub const fn bindings(&self) -> &TextureBindings {
        &self.bindings
    }

    /// Whether anything is still on its way.
    pub fn loading(&self) -> bool {
        self.loader
            .as_ref()
            .is_some_and(|loader| loader.outstanding() > 0)
            || self
                .fonts
                .as_ref()
                .is_some_and(|loader| loader.outstanding() > 0)
    }
}
