use std::{collections::BTreeSet, fs, path::Path};

use serde_json::Value;

#[derive(Default)]
pub(crate) struct ProjectIndex {
    pub(crate) entity_names: BTreeSet<String>,
    pub(crate) audio_assets: BTreeSet<String>,
}

impl ProjectIndex {
    pub(crate) fn scan(root: Option<&Path>) -> Self {
        let mut index = Self::default();
        if let Some(root) = root {
            index.scan_dir(root, root);
        }
        index
    }

    fn scan_dir(&mut self, root: &Path, directory: &Path) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some(".git" | "target" | "node_modules")
                ) {
                    self.scan_dir(root, &path);
                }
                continue;
            }

            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            if matches!(extension.as_deref(), Some("wav" | "ogg" | "mp3" | "flac")) {
                self.audio_assets.insert(relative.clone());
            }
            if !relative.ends_with(".scene.json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            collect_entity_names(&value, &mut self.entity_names);
        }
    }
}

fn collect_entity_names(scene: &Value, into: &mut BTreeSet<String>) {
    let Some(entities) = scene.get("entities").and_then(Value::as_array) else {
        return;
    };
    for entity in entities {
        if let Some(name) = entity.get("name").and_then(Value::as_str) {
            into.insert(name.to_owned());
        }
    }
}
