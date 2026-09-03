//! What a project ships, and what each of its assets is supposed to be.
//!
//! A scene names `textures/badge.png`. On a developer's machine that resolves to
//! a file, and a wrong one fails loudly. On static web hosting it resolves to a
//! URL, and the ways it can be wrong are quieter: a truncated response, a stale
//! entry in a CDN, a deploy that replaced half the files. The bytes arrive, they
//! decode, and the picture is last week's.
//!
//! A manifest is the project saying, once and in advance, what each asset is.
//! Two things follow from that. A build knows what to publish without walking a
//! directory at deploy time, and a load can check what arrived against what was
//! promised rather than trusting whatever came back.
//!
//! The hash is SHA-256 of the bytes as they are stored, before any decoding. It
//! is not a security boundary — anyone who can replace an asset can replace the
//! manifest beside it — but it is the same digest the browser's subresource
//! integrity uses, so the day this feeds a `<link integrity>` the numbers are
//! already the right ones.

use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use sindri_core::{AssetId, AssetLoadError, AssetLoadErrorKind};
use thiserror::Error;

#[cfg(test)]
mod tests;

/// The manifest format this build writes and understands.
///
/// Versioned for the same reason a scene is: a manifest outlives the build that
/// wrote it, and a reader that guesses is a reader that is wrong once.
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// The digest algorithm, named in every hash so the format can gain another
/// without the version having to say which is which.
const ALGORITHM: &str = "sha256";

/// A SHA-256 of an asset's stored bytes.
///
/// Written as `sha256:` and sixty-four lowercase hex characters. Hex rather than
/// base64 because a manifest is a file people read in review, and the twenty
/// bytes base64 would save are not worth a reviewer squinting at it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// The hash of some bytes.
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(ALGORITHM)?;
        formatter.write_str(":")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ContentHash {
    type Err = ManifestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value
            .strip_prefix(ALGORITHM)
            .and_then(|rest| rest.strip_prefix(':'))
            .ok_or_else(|| ManifestError::UnknownAlgorithm(value.to_owned()))?;
        if digits.len() != 64 {
            return Err(ManifestError::MalformedHash(value.to_owned()));
        }
        let mut bytes = [0_u8; 32];
        for (byte, pair) in bytes.iter_mut().zip(digits.as_bytes().chunks_exact(2)) {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| ManifestError::MalformedHash(value.to_owned()))?;
            *byte = u8::from_str_radix(pair, 16)
                .map_err(|_| ManifestError::MalformedHash(value.to_owned()))?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// What an asset is, so a host can load it without being told in advance.
///
/// The manifest carries this because the alternative is a list of asset IDs
/// compiled into the host, one per kind, maintained by hand — which is exactly
/// what stops a project being exported without someone editing Rust. A host
/// that reads kinds from the manifest can open any project.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// The scene document itself.
    Scene,
    /// A Decay source file.
    Script,
    /// A reusable entity definition a script can spawn.
    ///
    /// Separate from `Scene` because a host does different things with the
    /// two: a scene is opened, and a prefab is held so that a script asking
    /// for one gets it. A build that carried prefabs as scenes would open
    /// several worlds; one that carried them as `Other` could not tell which
    /// bytes to parse.
    Prefab,
    Texture,
    /// A sprite sheet describing how a texture is cut up.
    Sheet,
    Font,
    Audio,
    /// Something the engine does not decode: read as bytes, or not at all.
    ///
    /// A project may ship a file for its own reasons, and a manifest that could
    /// not describe one would force it to be lied about as some other kind.
    Other,
}

impl AssetKind {
    /// Every kind, in the order a host should load them.
    ///
    /// The scene first, because everything else is referenced from it.
    pub const ALL: [Self; 8] = [
        Self::Scene,
        Self::Prefab,
        Self::Sheet,
        Self::Script,
        Self::Texture,
        Self::Font,
        Self::Audio,
        Self::Other,
    ];

    /// What a file is, from its name.
    ///
    /// The last resort, for the two callers that have nothing else to go on: a
    /// directory scan, and an asset a project listed by hand. Everything found
    /// by walking a scene already knows what it is, because the component that
    /// named it says so — and that is the better answer wherever it is
    /// available, since a texture called `notes.txt` is still a texture if a
    /// sprite names it.
    ///
    /// An extension nobody recognises is `Other`, which is honest: the engine
    /// will not decode it, and refusing to carry it would be worse.
    #[must_use]
    pub fn for_id(id: &str) -> Self {
        match id.rsplit_once('.').map(|(_, extension)| extension) {
            Some("wav" | "ogg" | "mp3" | "flac") => Self::Audio,
            Some("png" | "jpg" | "jpeg") => Self::Texture,
            Some("ttf" | "otf") => Self::Font,
            Some("decay") => Self::Script,
            // Both end in `.json`, so the longer name is tested first.
            _ if id.ends_with(sindri_core::PREFAB_SUFFIX) => Self::Prefab,
            _ if id.ends_with(".sheet.json") => Self::Sheet,
            _ if id.ends_with(".scene.json") => Self::Scene,
            _ => Self::Other,
        }
    }

    /// The name this kind is stored under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Script => "script",
            Self::Prefab => "prefab",
            Self::Texture => "texture",
            Self::Sheet => "sheet",
            Self::Font => "font",
            Self::Audio => "audio",
            Self::Other => "other",
        }
    }
}

/// One asset, as the manifest records it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManifestEntry {
    /// What kind of thing this is.
    ///
    /// Defaulted, and left out when it is `Other`, because `Other` is the
    /// absence of a statement rather than one: a manifest written before kinds
    /// existed says nothing about them, and one that has nothing to say should
    /// read the same as it always did.
    #[serde(default = "other_kind", skip_serializing_if = "is_other")]
    pub kind: AssetKind,
    /// How many bytes the asset is.
    ///
    /// Redundant against the hash, and worth carrying anyway: it is what a build
    /// reports as a download size, and a length that disagrees identifies a
    /// truncated response before anything hashes a megabyte to say the same.
    pub bytes: u64,
    pub hash: ContentHash,
}

const fn other_kind() -> AssetKind {
    AssetKind::Other
}

// `serde` hands a reference to `skip_serializing_if`, and clippy would rather a
// one-byte enum were copied. Both are satisfied by taking the reference and
// immediately not keeping it.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_other(kind: &AssetKind) -> bool {
    matches!(*kind, AssetKind::Other)
}

/// Every asset a project ships.
///
/// Ordered by ID, because the file is reviewed and diffed like any other source:
/// a manifest whose lines moved when nothing changed would make every asset
/// change unreadable.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssetManifest {
    format_version: u32,
    /// The directory the assets live in, relative to wherever the manifest is.
    ///
    /// Empty for a project whose assets sit beside it, which is what a
    /// hand-written manifest and every existing one means. An export names a
    /// directory here, and names it after what the assets hash to — so the
    /// manifest is the one file that must never be cached and everything it
    /// points at can be cached for ever.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    content_root: String,
    assets: BTreeMap<AssetId, ManifestEntry>,
}

impl Default for AssetManifest {
    fn default() -> Self {
        Self {
            format_version: MANIFEST_FORMAT_VERSION,
            content_root: String::new(),
            assets: BTreeMap::new(),
        }
    }
}

impl AssetManifest {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Where the assets live, relative to the manifest.
    pub fn content_root(&self) -> &str {
        &self.content_root
    }

    /// Says where the assets live.
    pub fn set_content_root(&mut self, root: impl Into<String>) {
        self.content_root = root.into();
    }

    /// Records what an asset is, returning what it was recorded as before.
    /// Records an asset, taking it for whatever its name says it is.
    ///
    /// It used to record every asset as `Other`, which was harmless while
    /// nothing read kinds and became a trap the moment a host asked for its
    /// assets by kind: a manifest of nothing but `Other` describes a project
    /// with no scene, and the host that read one loaded nothing at all.
    pub fn insert(&mut self, id: AssetId, bytes: &[u8]) -> Option<ManifestEntry> {
        let kind = AssetKind::for_id(id.as_str());
        self.insert_as(id, kind, bytes)
    }

    /// Records what an asset is, and what kind of thing it is.
    pub fn insert_as(
        &mut self,
        id: AssetId,
        kind: AssetKind,
        bytes: &[u8],
    ) -> Option<ManifestEntry> {
        self.assets.insert(
            id,
            ManifestEntry {
                kind,
                bytes: bytes.len() as u64,
                hash: ContentHash::of(bytes),
            },
        )
    }

    pub fn get(&self, id: &AssetId) -> Option<&ManifestEntry> {
        self.assets.get(id)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn assets(&self) -> impl ExactSizeIterator<Item = (&AssetId, &ManifestEntry)> {
        self.assets.iter()
    }

    /// Every asset of one kind, in the manifest's own order.
    ///
    /// This is what lets a host open a project it was not compiled against: it
    /// asks the manifest what textures there are rather than being handed a
    /// list by whoever wrote the game.
    pub fn ids_of(&self, kind: AssetKind) -> impl Iterator<Item = &AssetId> {
        self.assets
            .iter()
            .filter(move |(_, entry)| entry.kind == kind)
            .map(|(id, _)| id)
    }

    /// Checks bytes that arrived against what was promised.
    ///
    /// An asset the manifest does not mention passes. A manifest is a statement
    /// about what it lists, not a claim that nothing else exists — a project
    /// that loads something generated at runtime should not have to describe it
    /// in advance to be allowed to load it.
    ///
    /// The length is checked first because it is free and it is what a truncated
    /// response fails on, so the common failure names itself without hashing a
    /// megabyte to reach the same conclusion.
    pub fn verify(&self, id: &AssetId, bytes: &[u8]) -> Result<(), AssetLoadError> {
        let Some(entry) = self.assets.get(id) else {
            return Ok(());
        };
        if entry.bytes != bytes.len() as u64 {
            return Err(AssetLoadError::new(
                id.clone(),
                AssetLoadErrorKind::InvalidData,
                format!(
                    "the manifest expects {} bytes and {} arrived",
                    entry.bytes,
                    bytes.len()
                ),
            ));
        }
        let hash = ContentHash::of(bytes);
        if hash != entry.hash {
            return Err(AssetLoadError::new(
                id.clone(),
                AssetLoadErrorKind::InvalidData,
                format!(
                    "the manifest expects {} and the bytes are {hash}",
                    entry.hash
                ),
            ));
        }
        Ok(())
    }

    /// The manifest as the file it is stored as.
    ///
    /// Canonical without needing a canonicaliser: the structure is flat and the
    /// assets are in a sorted map, so pretty-printing it is already stable. The
    /// trailing newline is there because every other text file has one.
    pub fn to_canonical_json(&self) -> Result<String, ManifestError> {
        let mut text = serde_json::to_string_pretty(self)
            .map_err(|error| ManifestError::Json(error.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    /// Reads a manifest, rejecting a version this build does not understand.
    pub fn from_json(text: &str) -> Result<Self, ManifestError> {
        let manifest: Self =
            serde_json::from_str(text).map_err(|error| ManifestError::Json(error.to_string()))?;
        if manifest.format_version != MANIFEST_FORMAT_VERSION {
            return Err(ManifestError::UnsupportedVersion(manifest.format_version));
        }
        Ok(manifest)
    }

    /// Builds a manifest by reading every asset under `root`.
    ///
    /// The IDs are the paths below the root, which is exactly what an asset
    /// source resolves against it, so a manifest built here describes the same
    /// names a scene writes. Dot files are the tooling's rather than the
    /// project's and are skipped, and so is the manifest itself: a file cannot
    /// contain its own hash.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn of_directory(root: &std::path::Path) -> Result<Self, ManifestError> {
        let mut manifest = Self::new();
        collect(root, root, &mut manifest)?;
        Ok(manifest)
    }
}

/// What a manifest is normally called, so a build and a loader agree without
/// being told.
pub const MANIFEST_FILE_NAME: &str = "sindri.manifest.json";

#[cfg(not(target_arch = "wasm32"))]
fn collect(
    root: &std::path::Path,
    directory: &std::path::Path,
    manifest: &mut AssetManifest,
) -> Result<(), ManifestError> {
    let listing = std::fs::read_dir(directory)
        .map_err(|error| ManifestError::Read(format!("{}: {error}", directory.display())))?;
    let mut entries: Vec<std::path::PathBuf> = listing
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| ManifestError::Read(format!("{}: {error}", directory.display())))
        })
        .collect::<Result<_, _>>()?;
    entries.sort();

    for path in entries {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') || name == MANIFEST_FILE_NAME {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, manifest)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| ManifestError::Read(format!("{} is not under the root", path.display())))?
            .to_string_lossy()
            .replace('\\', "/");
        let id = AssetId::new(relative.clone())
            .map_err(|error| ManifestError::AssetId(format!("{relative}: {error}")))?;
        let bytes = std::fs::read(&path)
            .map_err(|error| ManifestError::Read(format!("{}: {error}", path.display())))?;
        manifest.insert(id, &bytes);
    }
    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest format version {0} is not supported")]
    UnsupportedVersion(u32),
    #[error("'{0}' does not name a digest this build understands")]
    UnknownAlgorithm(String),
    #[error("'{0}' is not a well-formed digest")]
    MalformedHash(String),
    #[error("could not read the project: {0}")]
    Read(String),
    #[error("a file does not name a valid asset: {0}")]
    AssetId(String),
    #[error("manifest JSON: {0}")]
    Json(String),
}
