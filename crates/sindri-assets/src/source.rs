use std::{future::Future, pin::Pin};

use sindri_core::{AssetId, AssetLoadError, AssetLoadErrorKind};
use thiserror::Error;

pub type AssetSourceFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AssetBytes, AssetSourceError>> + 'a>>;

/// A source capable of resolving logical asset IDs to undecoded bytes.
///
/// The boxed future keeps this contract object-safe and permits browser
/// futures that are not `Send`. Native runtime hosts must poll filesystem
/// sources on their I/O workers rather than on a frame thread.
pub trait AssetSource {
    fn name(&self) -> &'static str;

    fn load<'a>(&'a self, id: &'a AssetId) -> AssetSourceFuture<'a>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetBytes {
    id: AssetId,
    bytes: Vec<u8>,
    content_type: Option<String>,
}

impl AssetBytes {
    pub fn new(id: AssetId, bytes: Vec<u8>) -> Self {
        Self {
            id,
            bytes,
            content_type: None,
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{kind} in asset source '{source_name}' for '{id}': {message}")]
pub struct AssetSourceError {
    id: AssetId,
    source_name: &'static str,
    kind: AssetLoadErrorKind,
    message: String,
    status_code: Option<u16>,
}

impl AssetSourceError {
    pub fn new(
        id: AssetId,
        source_name: &'static str,
        kind: AssetLoadErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            source_name,
            kind,
            message: message.into(),
            status_code: None,
        }
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub const fn source_name(&self) -> &'static str {
        self.source_name
    }

    pub const fn kind(&self) -> AssetLoadErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn status_code(&self) -> Option<u16> {
        self.status_code
    }
}

impl From<AssetSourceError> for AssetLoadError {
    fn from(error: AssetSourceError) -> Self {
        Self::new(
            error.id,
            error.kind,
            format!("source '{}': {}", error.source_name, error.message),
        )
    }
}
