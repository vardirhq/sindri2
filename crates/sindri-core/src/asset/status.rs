//! Where an asset is in loading, and what went wrong when it failed.

use std::fmt;

use thiserror::Error;

use super::id::AssetId;

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
