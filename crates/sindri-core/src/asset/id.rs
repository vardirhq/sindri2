//! What names an asset, and what a valid name is.

use std::{fmt, hash::Hash, str::FromStr};

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
