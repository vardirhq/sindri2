use std::{
    collections::BTreeMap,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    str::FromStr,
    sync::{Arc, Weak},
};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// A stable, portable identifier for an authored asset.
///
/// Asset IDs are relative, slash-separated logical paths. They deliberately do
/// not identify a filesystem location; an asset source resolves them for the
/// active platform later in the loading pipeline.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AssetId(String);

impl AssetId {
    pub fn new(value: impl Into<String>) -> Result<Self, AssetIdError> {
        let value = value.into();
        validate_asset_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AssetId {
    type Err = AssetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for AssetId {
    type Error = AssetIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

fn validate_asset_id(value: &str) -> Result<(), AssetIdError> {
    if value.is_empty() {
        return Err(AssetIdError::Empty);
    }
    if value.starts_with('/') {
        return Err(AssetIdError::Absolute);
    }
    if value.contains('\\') {
        return Err(AssetIdError::Backslash);
    }
    if let Some(delimiter) = [':', '?', '#']
        .into_iter()
        .find(|delimiter| value.contains(*delimiter))
    {
        return Err(AssetIdError::ReservedDelimiter(delimiter));
    }
    if value.chars().any(char::is_control) {
        return Err(AssetIdError::ControlCharacter);
    }
    for segment in value.split('/') {
        if segment.is_empty() {
            return Err(AssetIdError::EmptySegment);
        }
        if matches!(segment, "." | "..") {
            return Err(AssetIdError::DotSegment);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AssetIdError {
    #[error("asset IDs cannot be empty")]
    Empty,
    #[error("asset IDs must be relative")]
    Absolute,
    #[error("asset IDs use '/' separators, not backslashes")]
    Backslash,
    #[error("asset IDs cannot contain the reserved delimiter '{0}'")]
    ReservedDelimiter(char),
    #[error("asset IDs cannot contain control characters")]
    ControlCharacter,
    #[error("asset IDs cannot contain empty path segments")]
    EmptySegment,
    #[error("asset IDs cannot contain '.' or '..' path segments")]
    DotSegment,
}

#[derive(Debug)]
struct AssetLease;

/// A strong, type-safe reference to one logical asset request.
///
/// Cloning a handle keeps the asset live. The store may reclaim the loaded
/// value only after every strong handle for that request has been dropped.
pub struct AssetHandle<T> {
    id: AssetId,
    generation: u64,
    lease: Arc<AssetLease>,
    marker: PhantomData<fn() -> T>,
}

impl<T> AssetHandle<T> {
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn downgrade(&self) -> WeakAssetHandle<T> {
        WeakAssetHandle {
            id: self.id.clone(),
            generation: self.generation,
            lease: Arc::downgrade(&self.lease),
            marker: PhantomData,
        }
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.lease)
    }
}

impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            generation: self.generation,
            lease: Arc::clone(&self.lease),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for AssetHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AssetHandle")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && Arc::ptr_eq(&self.lease, &other.lease)
    }
}

impl<T> Eq for AssetHandle<T> {}

impl<T> Hash for AssetHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.generation.hash(state);
        Arc::as_ptr(&self.lease).hash(state);
    }
}

/// A non-owning typed asset reference.
///
/// Upgrading succeeds only while at least one strong handle from the same
/// generation remains alive.
pub struct WeakAssetHandle<T> {
    id: AssetId,
    generation: u64,
    lease: Weak<AssetLease>,
    marker: PhantomData<fn() -> T>,
}

impl<T> WeakAssetHandle<T> {
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn upgrade(&self) -> Option<AssetHandle<T>> {
        self.lease.upgrade().map(|lease| AssetHandle {
            id: self.id.clone(),
            generation: self.generation,
            lease,
            marker: PhantomData,
        })
    }
}

impl<T> Clone for WeakAssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            generation: self.generation,
            lease: self.lease.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for WeakAssetHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeakAssetHandle")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .field("alive", &(self.lease.strong_count() > 0))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetStatus {
    Queued,
    Loading,
    Ready,
    Failed,
}

impl fmt::Display for AssetStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Queued => "queued",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Failed => "failed",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetLoadErrorKind {
    NotFound,
    AccessDenied,
    UnsupportedFormat,
    InvalidData,
    Network,
    Io,
    Cancelled,
    Other,
}

impl fmt::Display for AssetLoadErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "not found",
            Self::AccessDenied => "access denied",
            Self::UnsupportedFormat => "unsupported format",
            Self::InvalidData => "invalid data",
            Self::Network => "network error",
            Self::Io => "I/O error",
            Self::Cancelled => "cancelled",
            Self::Other => "load error",
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} while loading asset '{id}': {message}")]
pub struct AssetLoadError {
    id: AssetId,
    kind: AssetLoadErrorKind,
    message: String,
}

impl AssetLoadError {
    pub fn new(id: AssetId, kind: AssetLoadErrorKind, message: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            message: message.into(),
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn kind(&self) -> AssetLoadErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug)]
enum StoredAsset<T> {
    Queued,
    Loading,
    Ready(T),
    Failed(AssetLoadError),
}

impl<T> StoredAsset<T> {
    const fn status(&self) -> AssetStatus {
        match self {
            Self::Queued => AssetStatus::Queued,
            Self::Loading => AssetStatus::Loading,
            Self::Ready(_) => AssetStatus::Ready,
            Self::Failed(_) => AssetStatus::Failed,
        }
    }
}

#[derive(Debug)]
struct AssetEntry<T> {
    generation: u64,
    lease: Weak<AssetLease>,
    state: StoredAsset<T>,
}

/// Renderer- and executor-independent storage for one asset type.
///
/// The store owns loaded values while handles own liveness. Requests for an
/// already-live ID coalesce onto the same generation. Call [`Self::collect_unused`]
/// at an explicit maintenance point to release entries without strong handles.
#[derive(Debug)]
pub struct AssetStore<T> {
    entries: BTreeMap<AssetId, AssetEntry<T>>,
    next_generation: u64,
}

impl<T> Default for AssetStore<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_generation: 0,
        }
    }
}

impl<T> AssetStore<T> {
    pub fn request(&mut self, id: AssetId) -> AssetHandle<T> {
        if let Some(entry) = self.entries.get(&id)
            && let Some(lease) = entry.lease.upgrade()
        {
            return AssetHandle {
                id,
                generation: entry.generation,
                lease,
                marker: PhantomData,
            };
        }

        let generation = self.allocate_generation();
        let lease = Arc::new(AssetLease);
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.generation = generation;
            entry.lease = Arc::downgrade(&lease);
        } else {
            self.entries.insert(
                id.clone(),
                AssetEntry {
                    generation,
                    lease: Arc::downgrade(&lease),
                    state: StoredAsset::Queued,
                },
            );
        }

        AssetHandle {
            id,
            generation,
            lease,
            marker: PhantomData,
        }
    }

    pub fn status(&self, handle: &AssetHandle<T>) -> Result<AssetStatus, AssetStoreError> {
        Ok(self.entry(handle)?.state.status())
    }

    pub fn status_by_id(&self, id: &AssetId) -> Option<AssetStatus> {
        self.entries.get(id).map(|entry| entry.state.status())
    }

    pub fn begin_loading(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Queued, StoredAsset::Loading)
    }

    pub fn complete(&mut self, handle: &AssetHandle<T>, value: T) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Loading, StoredAsset::Ready(value))
    }

    pub fn fail(
        &mut self,
        handle: &AssetHandle<T>,
        kind: AssetLoadErrorKind,
        message: impl Into<String>,
    ) -> Result<(), AssetStoreError> {
        let error = AssetLoadError::new(handle.id.clone(), kind, message);
        self.transition(handle, AssetStatus::Loading, StoredAsset::Failed(error))
    }

    pub fn retry(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Failed, StoredAsset::Queued)
    }

    /// Marks a ready asset as needing loading again.
    ///
    /// What hot reload does: the bytes on disk changed, so the value held is
    /// out of date, and the fact that it loaded successfully is exactly why it
    /// must load again. Separate from [`Self::retry`] because the two start
    /// from opposite states and mean opposite things — one is "that did not
    /// work", the other is "that worked and is now stale".
    pub fn reload(&mut self, handle: &AssetHandle<T>) -> Result<(), AssetStoreError> {
        self.transition(handle, AssetStatus::Ready, StoredAsset::Queued)
    }

    pub fn get(&self, handle: &AssetHandle<T>) -> Result<Option<&T>, AssetStoreError> {
        Ok(match &self.entry(handle)?.state {
            StoredAsset::Ready(value) => Some(value),
            _ => None,
        })
    }

    pub fn error(
        &self,
        handle: &AssetHandle<T>,
    ) -> Result<Option<&AssetLoadError>, AssetStoreError> {
        Ok(match &self.entry(handle)?.state {
            StoredAsset::Failed(error) => Some(error),
            _ => None,
        })
    }

    pub fn collect_unused(&mut self) -> Vec<AssetId> {
        let unused = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.lease.strong_count() == 0)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in &unused {
            self.entries.remove(id);
        }
        unused
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("asset handle generation exhausted u64::MAX");
        generation
    }

    fn transition(
        &mut self,
        handle: &AssetHandle<T>,
        expected: AssetStatus,
        next: StoredAsset<T>,
    ) -> Result<(), AssetStoreError> {
        let entry = self.entry_mut(handle)?;
        let actual = entry.state.status();
        if actual != expected {
            return Err(AssetStoreError::InvalidTransition {
                id: handle.id.clone(),
                from: actual,
                to: next.status(),
            });
        }
        entry.state = next;
        Ok(())
    }

    fn entry(&self, handle: &AssetHandle<T>) -> Result<&AssetEntry<T>, AssetStoreError> {
        self.entries
            .get(&handle.id)
            .filter(|entry| entry.matches(handle))
            .ok_or_else(|| AssetStoreError::InvalidHandle(handle.id.clone()))
    }

    fn entry_mut(
        &mut self,
        handle: &AssetHandle<T>,
    ) -> Result<&mut AssetEntry<T>, AssetStoreError> {
        self.entries
            .get_mut(&handle.id)
            .filter(|entry| entry.matches(handle))
            .ok_or_else(|| AssetStoreError::InvalidHandle(handle.id.clone()))
    }
}

impl<T> AssetEntry<T> {
    fn matches(&self, handle: &AssetHandle<T>) -> bool {
        self.generation == handle.generation
            && self
                .lease
                .upgrade()
                .is_some_and(|lease| Arc::ptr_eq(&lease, &handle.lease))
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AssetStoreError {
    #[error("invalid or stale handle for asset '{0}'")]
    InvalidHandle(AssetId),
    #[error("asset '{id}' cannot transition from {from} to {to}")]
    InvalidTransition {
        id: AssetId,
        from: AssetStatus,
        to: AssetStatus,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Texture(&'static str);

    fn texture_id() -> AssetId {
        AssetId::new("textures/player.png").unwrap()
    }

    #[test]
    fn asset_ids_are_portable_and_serde_validated() {
        let id = texture_id();
        assert_eq!(id.as_str(), "textures/player.png");
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            r#""textures/player.png""#
        );
        assert_eq!(
            serde_json::from_str::<AssetId>(r#""textures/player.png""#).unwrap(),
            id
        );

        for invalid in [
            "",
            "/player.png",
            "textures\\player.png",
            "textures//player.png",
            "textures/../player.png",
            "C:/textures/player.png",
            "textures/player.png?version=1",
            "textures/player.png#fragment",
        ] {
            assert!(AssetId::new(invalid).is_err(), "accepted '{invalid}'");
        }
    }

    #[test]
    fn duplicate_requests_share_one_live_generation() {
        let mut assets = AssetStore::<Texture>::default();
        let first = assets.request(texture_id());
        let second = assets.request(texture_id());

        assert_eq!(first, second);
        assert_eq!(first.strong_count(), 2);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets.status(&first).unwrap(), AssetStatus::Queued);
    }

    #[test]
    fn load_state_is_explicit_and_checked() {
        let mut assets = AssetStore::<Texture>::default();
        let handle = assets.request(texture_id());

        assets.begin_loading(&handle).unwrap();
        assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Loading);
        assert!(matches!(
            assets.retry(&handle),
            Err(AssetStoreError::InvalidTransition { .. })
        ));
        assets.complete(&handle, Texture("rgba pixels")).unwrap();
        assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Ready);
        assert_eq!(assets.get(&handle).unwrap(), Some(&Texture("rgba pixels")));
    }

    #[test]
    fn failures_keep_actionable_asset_context_and_can_retry() {
        let mut assets = AssetStore::<Texture>::default();
        let handle = assets.request(texture_id());
        assets.begin_loading(&handle).unwrap();
        assets
            .fail(
                &handle,
                AssetLoadErrorKind::NotFound,
                "no source contained this logical ID",
            )
            .unwrap();

        let error = assets.error(&handle).unwrap().unwrap();
        assert_eq!(error.id(), handle.id());
        assert_eq!(error.kind(), AssetLoadErrorKind::NotFound);
        assert!(error.to_string().contains("textures/player.png"));

        assets.retry(&handle).unwrap();
        assert_eq!(assets.status(&handle).unwrap(), AssetStatus::Queued);
        assert!(assets.error(&handle).unwrap().is_none());
    }

    #[test]
    fn values_live_until_the_last_strong_handle_is_collected() {
        let mut assets = AssetStore::<Texture>::default();
        let first = assets.request(texture_id());
        let second = first.clone();
        let weak = first.downgrade();

        drop(first);
        assert!(assets.collect_unused().is_empty());
        assert!(weak.upgrade().is_some());

        drop(second);
        assert_eq!(assets.collect_unused(), vec![texture_id()]);
        assert!(assets.is_empty());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn a_new_request_cannot_revive_an_expired_weak_handle() {
        let mut assets = AssetStore::<Texture>::default();
        let first = assets.request(texture_id());
        let weak = first.downgrade();
        let first_generation = first.generation();
        drop(first);

        let replacement = assets.request(texture_id());
        assert_ne!(replacement.generation(), first_generation);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn handles_are_bound_to_the_store_that_created_them() {
        let mut first_store = AssetStore::<Texture>::default();
        let first = first_store.request(texture_id());
        let mut second_store = AssetStore::<Texture>::default();
        let second = second_store.request(texture_id());

        assert_ne!(first, second);
        assert!(matches!(
            second_store.status(&first),
            Err(AssetStoreError::InvalidHandle(_))
        ));
    }
}

/// A texture, and optionally which named part of it to draw.
///
/// Written as `textures/tiles.png#floor`: the path before the `#`, the sprite's
/// name after it. Without a fragment it names the whole image.
///
/// `#` is a *rejected* character in [`AssetId`], and that is the argument for
/// using it here rather than against. It is reserved precisely so a fragment
/// cannot leak into a path that becomes a URL, so splitting it off at the
/// boundary — exactly as a URL does — leaves the asset ID a pure path and gives
/// the fragment somewhere to live. Nothing that resolves an asset ever sees it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpriteRef {
    texture: String,
    sprite: Option<String>,
}

impl SpriteRef {
    /// Parses `textures/tiles.png#floor`, or a plain path for the whole image.
    pub fn parse(reference: &str) -> Result<Self, SpriteRefError> {
        let (path, sprite) = match reference.split_once('#') {
            Some((path, name)) => {
                if name.is_empty() {
                    return Err(SpriteRefError::EmptySprite);
                }
                if name.contains('#') {
                    return Err(SpriteRefError::SecondFragment);
                }
                (path, Some(name.to_owned()))
            }
            None => (reference, None),
        };
        if path.is_empty() {
            return Err(SpriteRefError::EmptyTexture);
        }
        Ok(Self {
            texture: path.to_owned(),
            sprite,
        })
    }

    /// The reference as a whole names its texture, which is what a host binds
    /// and what the renderer is asked for.
    ///
    /// A string rather than an [`AssetId`], because not every texture is a
    /// file: `procedural:checkerboard` is generated, and the colon that makes
    /// it un-parseable as an asset ID is exactly what marks it as generated.
    #[must_use]
    pub fn texture(&self) -> &str {
        &self.texture
    }

    /// The asset behind the texture, or `None` when nothing loads it.
    pub fn asset(&self) -> Option<AssetId> {
        AssetId::new(self.texture.clone()).ok()
    }

    /// Which part of the image, or `None` for all of it.
    #[must_use]
    pub fn sprite(&self) -> Option<&str> {
        self.sprite.as_deref()
    }

    /// The sheet this reference needs loaded, which is only ever the one its
    /// own fragment names a sprite in.
    ///
    /// A reference with no fragment needs no sheet, so an unsliced texture is
    /// not asked for a sidecar that does not exist. That is what keeps a
    /// missing sheet an error worth reporting rather than the ordinary case.
    #[must_use]
    pub fn sheet(&self) -> Option<AssetId> {
        self.sprite.as_ref()?;
        crate::sheet_id_for(&self.asset()?)
    }
}

impl fmt::Display for SpriteRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.sprite {
            Some(sprite) => write!(formatter, "{}#{sprite}", self.texture),
            None => formatter.write_str(self.texture.as_str()),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SpriteRefError {
    #[error("a sprite reference must name a texture")]
    EmptyTexture,
    #[error("a sprite reference's `#` must be followed by a name")]
    EmptySprite,
    #[error("a sprite reference names one sprite, so it holds one `#`")]
    SecondFragment,
}
