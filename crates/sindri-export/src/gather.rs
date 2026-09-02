//! Working out what a project is made of.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sindri_assets::AssetKind;
use sindri_core::{SceneDocument, World};
use sindri_scene::{SceneExtractor, referenced_fonts, referenced_textures};

use crate::write::ExportError;

/// A project's own description of itself.
#[derive(Debug, Deserialize)]
struct ProjectFile {
    project: ProjectSection,
    #[serde(default)]
    assets: AssetsSection,
}

/// What a project ships that its scene does not mention.
///
/// Almost everything is found by walking the scene, which is what makes an
/// export impossible to forget to update. This is the exception, and it exists
/// because a script can name a clip at run time — `Audio.play("pickup.wav")` is
/// a string inside a program, and no walk of a scene can see it. Guessing by
/// scanning script text for anything that looks like a path would ship whatever
/// a comment mentioned and miss whatever was built from a variable.
#[derive(Debug, Default, Deserialize)]
struct AssetsSection {
    #[serde(default)]
    include: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    name: String,
    main_scene: String,
}

/// One file the export will ship, and what it is.
#[derive(Clone, Debug)]
pub struct GatheredAsset {
    /// What the scene calls it, which is what a host asks for.
    pub id: String,
    pub kind: AssetKind,
    pub bytes: Vec<u8>,
}

/// Everything a project ships, worked out from its scene.
#[derive(Debug)]
pub struct ProjectExport {
    pub name: String,
    pub assets: Vec<GatheredAsset>,
}

impl ProjectExport {
    /// Reads a project directory and works out what it is made of.
    ///
    /// Nothing here is configured. A texture ships because a component names
    /// it, a font because a text element does, a script because an entity runs
    /// one — so an asset that stopped being used stops being carried, and one
    /// that started being used cannot be forgotten.
    pub fn gather(project: &Path) -> Result<Self, ExportError> {
        let text = std::fs::read_to_string(project.join("sindri.toml"))
            .map_err(|error| ExportError::unreadable(&project.join("sindri.toml"), &error))?;
        let file: ProjectFile =
            toml::from_str(&text).map_err(|error| ExportError::Project(error.to_string()))?;

        let scene_path = project.join(&file.project.main_scene);
        let scene_bytes = read(&scene_path)?;
        let document: SceneDocument = serde_json::from_slice(&scene_bytes).map_err(|error| {
            ExportError::Project(format!("the main scene does not read: {error}"))
        })?;

        let extractor = SceneExtractor::new().map_err(|error| {
            ExportError::Project(format!("the components do not register: {error}"))
        })?;
        // `sindri.script` is not a builtin: scripting is a layer above the
        // scene, and a host registers it. An export that did not register it
        // would carry a game with no code in it and look like it had worked.
        let mut components = extractor.components().clone();
        components
            .register::<sindri_decay::ScriptComponent>("Script")
            .map_err(|error| ExportError::Project(format!("sindri.script: {error}")))?;
        let world = World::from_scene(&document)
            .map_err(|error| {
                ExportError::Project(format!("the main scene does not load: {error}"))
            })?
            .world;

        let mut assets = vec![GatheredAsset {
            // A scene is named by its file, so two scenes in one project do not
            // collide, and so the host asks for the one the project names.
            id: leaf(&file.project.main_scene),
            kind: AssetKind::Scene,
            bytes: scene_bytes,
        }];

        // Ordered and de-duplicated, because two entities naming one texture is
        // one download.
        let mut wanted: BTreeMap<String, AssetKind> = BTreeMap::new();
        for reference in referenced_textures(&world) {
            wanted.insert(reference, AssetKind::Texture);
        }
        for font in referenced_fonts(&world) {
            wanted.insert(font, AssetKind::Font);
        }
        for source in sindri_decay::referenced_sources(&world, &components) {
            wanted.insert(source, AssetKind::Script);
        }

        // Audio is named by a component like anything else, and was the one
        // kind with no walker in the engine — a scene's music would have been
        // left behind by an export that looked complete.
        for (_, data) in world.entities() {
            if let Some(payload) = data.components.get("sindri.audio.source")
                && let Some(clip) = payload.get("clip").and_then(serde_json::Value::as_str)
            {
                wanted.insert(clip.to_owned(), AssetKind::Audio);
            }
        }

        // A sheet is not named by anything: it is found beside its texture, and
        // a texture cut into sprites that shipped without one would draw whole
        // images where frames should be.
        let sheets: Vec<String> = wanted
            .iter()
            .filter(|(_, kind)| **kind == AssetKind::Texture)
            .filter_map(|(id, _)| {
                sindri_core::AssetId::new(id)
                    .ok()
                    .and_then(|texture| sindri_core::sheet_id_for(&texture))
            })
            .map(|id| id.as_str().to_owned())
            .filter(|id| project.join("assets").join(id).exists())
            .collect();
        for id in sheets {
            wanted.insert(id, AssetKind::Sheet);
        }

        // What the scene cannot name.
        for id in &file.assets.include {
            wanted.insert(id.clone(), kind_of(id));
        }

        for (id, kind) in wanted {
            // A procedural texture is drawn by the engine and has no file. It
            // is named like an asset and is not one, so shipping it would mean
            // failing to find it.
            if id.starts_with("sindri:") {
                continue;
            }
            let path = project.join("assets").join(&id);
            let path = if path.exists() {
                path
            } else {
                project.join(&id)
            };
            assets.push(GatheredAsset {
                id: id.clone(),
                kind,
                bytes: read(&path)?,
            });
        }

        Ok(Self {
            name: file.project.name,
            assets,
        })
    }

    /// The scene's file name, which is what a host asks for first.
    #[must_use]
    pub fn scene_id(&self) -> Option<&str> {
        self.assets
            .iter()
            .find(|asset| asset.kind == AssetKind::Scene)
            .map(|asset| asset.id.as_str())
    }
}

/// What a file is, from its name.
///
/// Only used for assets a project listed by hand: everything found by walking
/// the scene already knows what it is, because the component that named it says
/// so. An extension nobody recognises ships as `Other`, which is honest — the
/// engine will not decode it, and refusing to carry it would be worse.
fn kind_of(id: &str) -> AssetKind {
    match id.rsplit_once('.').map(|(_, extension)| extension) {
        Some("wav" | "ogg" | "mp3" | "flac") => AssetKind::Audio,
        Some("png" | "jpg" | "jpeg") => AssetKind::Texture,
        Some("ttf" | "otf") => AssetKind::Font,
        Some("decay") => AssetKind::Script,
        _ if id.ends_with(".sheet.json") => AssetKind::Sheet,
        _ => AssetKind::Other,
    }
}

fn read(path: &Path) -> Result<Vec<u8>, ExportError> {
    std::fs::read(path).map_err(|error| ExportError::unreadable(path, &error))
}

/// The part of a path a host names an asset by.
fn leaf(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}
