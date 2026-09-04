//! What kind of thing a file in the project is, judged by its name.

use super::sheet::SHEET_SUFFIX;

/// What kind of thing an entry is, as far as the browser can tell.
///
/// From the extension, because that is all a file offers before something opens
/// it. `Other` is deliberate: an unrecognised file is still listed, since a
/// browser that hides what it does not understand is a browser you cannot trust
/// to be showing you the directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKind {
    Folder,
    Scene,
    Texture,
    /// One named part of a sliced texture. Not a file of its own: it is a row
    /// under the image it was cut from, which is where a person looks for it.
    Sprite,
    /// The sidecar that slices a texture. Listed as its own kind rather than as
    /// "File", because it is the thing a slicer edits.
    Sheet,
    Mesh,
    /// A fragment of a scene that scripts spawn. Its own kind rather than
    /// "File", because it is a thing the engine loads and the browser can say
    /// so: listed as a plain file, the acceptance project's every enemy looked
    /// like an unrecognised blob sitting in a folder.
    Prefab,
    Script,
    Font,
    Audio,
    Other,
}

impl AssetKind {
    /// What the browser calls this kind in its right-hand column.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Scene => "Scene",
            Self::Texture => "Texture",
            Self::Sprite => "Sprite",
            Self::Sheet => "Sheet",
            Self::Mesh => "Mesh",
            Self::Prefab => "Prefab",
            Self::Script => "Script",
            Self::Font => "Font",
            Self::Audio => "Audio",
            Self::Other => "File",
        }
    }

    /// What the file at this path is, judged by its name.
    #[must_use]
    pub fn of_path(path: &std::path::Path) -> Self {
        if path.is_dir() {
            return Self::Folder;
        }
        Self::of_file(&path.to_string_lossy())
    }

    /// What a file of this name is, judged by its extension.
    ///
    /// A scene is `*.scene.json` rather than any JSON, because the editor can
    /// open one and not the other, and a row that offers to open a settings
    /// file as a scene is the same class of lie this module exists to remove.
    pub(crate) fn of_file(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.ends_with(".scene.json") {
            return Self::Scene;
        }
        if lower.ends_with(SHEET_SUFFIX) {
            return Self::Sheet;
        }
        if lower.ends_with(sindri_core::PREFAB_SUFFIX) {
            return Self::Prefab;
        }
        match lower.rsplit_once('.').map(|(_, extension)| extension) {
            Some("png" | "jpg" | "jpeg" | "webp" | "bmp" | "ktx2" | "dds") => Self::Texture,
            Some("gltf" | "glb" | "obj" | "fbx") => Self::Mesh,
            // `decay` first because it is the engine's own: a `.decay` file is
            // a script the editor can actually run, and the rest are scripts
            // only in the sense that they are code sitting in a project.
            Some("decay" | "rs" | "ts" | "js" | "wgsl") => Self::Script,
            Some("ttf" | "otf" | "woff" | "woff2") => Self::Font,
            Some("wav" | "ogg" | "mp3") => Self::Audio,
            _ => Self::Other,
        }
    }
}
