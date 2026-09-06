//! A reusable profile opened for structured authoring.

use std::path::{Path, PathBuf};

use sindri_core::ProfileDocument;

pub struct ProfileEditor {
    path: PathBuf,
    pub document: Option<ProfileDocument>,
    pub error: Option<String>,
    pub dirty: bool,
}

impl ProfileEditor {
    pub fn open(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(json) => match ProfileDocument::from_json(&json) {
                Ok(document) => Self {
                    path: path.to_owned(),
                    document: Some(document),
                    error: None,
                    dirty: false,
                },
                Err(error) => Self {
                    path: path.to_owned(),
                    document: None,
                    error: Some(error.to_string()),
                    dirty: false,
                },
            },
            Err(error) => Self {
                path: path.to_owned(),
                document: None,
                error: Some(error.to_string()),
                dirty: false,
            },
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn adopt(&mut self, path: &Path) {
        self.path = path.to_owned();
    }

    pub fn save(&mut self) -> Result<(), String> {
        let document = self.document.as_ref().ok_or_else(|| {
            self.error
                .clone()
                .unwrap_or_else(|| "profile is unreadable".into())
        })?;
        let json = document
            .to_canonical_json()
            .map_err(|error| error.to_string())?;
        std::fs::write(&self.path, json).map_err(|error| error.to_string())?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_edit_saves_back_to_its_own_asset() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ship.profile.json");
        std::fs::write(
            &path,
            ProfileDocument::default().to_canonical_json().unwrap(),
        )
        .unwrap();
        let mut editor = ProfileEditor::open(&path);
        editor.document.as_mut().unwrap().name = "Strider".into();
        editor.dirty = true;
        editor.save().unwrap();
        assert_eq!(ProfileEditor::open(&path).document.unwrap().name, "Strider");
    }
}
