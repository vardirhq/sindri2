//! Reusable authored data that is not an entity.
//!
//! A profile is Sindri's equivalent of a Unity `ScriptableObject`: a project
//! asset with an identity and values, referenced by scenes and scripts without
//! being placed in the world. The payload deliberately stays JSON-shaped so a
//! game can define its own vocabulary without adding engine components.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// The version this runtime writes and understands.
pub const PROFILE_FORMAT_VERSION: u32 = 1;

/// The suffix that identifies a profile asset.
pub const PROFILE_SUFFIX: &str = ".profile.json";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProfileDocument {
    pub format_version: u32,
    /// What the editor calls this asset. Gameplay should use the asset ID.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// An optional game-defined category such as `module` or `enemy_stats`.
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub profile_type: String,
    /// Game-owned data. Objects and arrays may be nested freely.
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

impl Default for ProfileDocument {
    fn default() -> Self {
        Self {
            format_version: PROFILE_FORMAT_VERSION,
            name: "New Profile".to_owned(),
            profile_type: String::new(),
            values: BTreeMap::new(),
        }
    }
}

impl ProfileDocument {
    pub fn from_json(json: &str) -> Result<Self, ProfileError> {
        let document: Self = serde_json::from_str(json).map_err(|error| ProfileError::Json {
            message: error.to_string(),
        })?;
        document.validate()?;
        Ok(document)
    }

    pub fn to_canonical_json(&self) -> Result<String, ProfileError> {
        self.validate()?;
        serde_json::to_string_pretty(self)
            .map(|json| format!("{json}\n"))
            .map_err(|error| ProfileError::Json {
                message: error.to_string(),
            })
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.format_version != PROFILE_FORMAT_VERSION {
            return Err(ProfileError::UnsupportedVersion {
                found: self.format_version,
                supported: PROFILE_FORMAT_VERSION,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    #[must_use]
    pub fn value_at(&self, collection: &str, index: usize, key: &str) -> Option<&Value> {
        self.values
            .get(collection)?
            .as_array()?
            .get(index)?
            .as_object()?
            .get(key)
    }

    #[must_use]
    pub fn count(&self, collection: &str) -> usize {
        self.values
            .get(collection)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProfileError {
    #[error("profile JSON is invalid: {message}")]
    Json { message: String },
    #[error("profile format {found} is not supported; this runtime supports {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_round_trips_nested_game_data() {
        let source = r#"{
          "format_version": 1,
          "name": "Hot Core",
          "type": "module",
          "values": {"weight": 2.0, "effects": [{"key": "damage", "value": 1.28}]}
        }"#;
        let profile = ProfileDocument::from_json(source).expect("the profile parses");
        assert_eq!(profile.count("effects"), 1);
        assert_eq!(profile.value("weight").and_then(Value::as_f64), Some(2.0));
        assert_eq!(
            profile
                .value_at("effects", 0, "key")
                .and_then(Value::as_str),
            Some("damage")
        );
        let written = profile.to_canonical_json().expect("the profile writes");
        assert_eq!(ProfileDocument::from_json(&written).unwrap(), profile);
    }
}
