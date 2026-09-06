//! Authored profile assets available to a running Decay world.

use std::collections::BTreeMap;

use sindri_core::ProfileDocument;

#[derive(Clone, Debug, Default)]
pub struct ProfileSources {
    profiles: BTreeMap<String, ProfileDocument>,
}

impl ProfileSources {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn none() -> &'static Self {
        static NONE: std::sync::OnceLock<ProfileSources> = std::sync::OnceLock::new();
        NONE.get_or_init(Self::new)
    }

    pub fn insert(&mut self, id: impl Into<String>, profile: ProfileDocument) {
        self.profiles.insert(id.into(), profile);
    }

    pub fn remove(&mut self, id: &str) -> Option<ProfileDocument> {
        self.profiles.remove(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ProfileDocument> {
        self.profiles.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ProfileDocument> {
        self.profiles.get_mut(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.profiles.keys().map(String::as_str)
    }
}
