//! A block set opened for editing: the blocks a voxel world or a tile volume
//! is built from, each a name, the art on its faces and what it does.
//!
//! A block set is a `.tileset.json`. Editing one by hand meant writing six
//! sprite references and a size per face for every block; the editor offers
//! the blocks as cubes and each face as the texture picker every other field
//! uses.

use std::path::{Path, PathBuf};

use sindri_core::{TileDefinition, TileFaceVisual, TileFaces, TileSetDocument};

/// What a new block looks like until it is given art: the engine's checker,
/// which is plainly unfinished rather than plausibly wrong.
const PLACEHOLDER: &str = "procedural:checkerboard";

pub struct BlockSetEditor {
    path: PathBuf,
    pub document: Option<TileSetDocument>,
    pub error: Option<String>,
    pub dirty: bool,
    /// The block the panel is showing, by name.
    pub selected: Option<String>,
}

impl BlockSetEditor {
    pub fn open(path: &Path) -> Self {
        let read = std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|json| TileSetDocument::from_json(&json).map_err(|error| error.to_string()));
        let (document, error) = match read {
            Ok(document) => (Some(document), None),
            Err(error) => (None, Some(error)),
        };
        let selected = document
            .as_ref()
            .and_then(|document| document.tiles.keys().next().cloned());
        Self {
            path: path.to_owned(),
            document,
            error,
            dirty: false,
            selected,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn adopt(&mut self, path: &Path) {
        path.clone_into(&mut self.path);
    }

    /// Writes the set back, refusing one the engine would refuse to load.
    pub fn save(&mut self) -> Result<(), String> {
        let document = self
            .document
            .as_ref()
            .ok_or_else(|| self.error.clone().unwrap_or_else(|| "unreadable".into()))?;
        document.validate().map_err(|error| error.to_string())?;
        std::fs::write(&self.path, block_set_json(document)?).map_err(|error| error.to_string())?;
        self.dirty = false;
        Ok(())
    }

    /// Adds a block with placeholder art, named so it collides with nothing,
    /// and selects it.
    pub fn add_block(&mut self) {
        let block = TileDefinition {
            faces: TileFaces {
                top: Some(face(PLACEHOLDER)),
                south: Some(face(PLACEHOLDER)),
                east: Some(face(PLACEHOLDER)),
                ..TileFaces::default()
            },
            ..blank_block()
        };
        self.insert_as("block", block);
    }

    /// Copies the selected block under a new name, and selects the copy.
    pub fn duplicate_selected(&mut self) {
        let Some((name, block)) = self
            .selected_block()
            .map(|(name, block)| (name.to_owned(), block.clone()))
        else {
            return;
        };
        self.insert_as(&name, block);
    }

    /// Removes the selected block, selecting its neighbour.
    pub fn remove_selected(&mut self) {
        let (Some(document), Some(name)) = (self.document.as_mut(), self.selected.clone()) else {
            return;
        };
        if document.tiles.remove(&name).is_some() {
            self.selected = document
                .tiles
                .range(name..)
                .next()
                .or_else(|| document.tiles.iter().next_back())
                .map(|(name, _)| name.clone());
            self.dirty = true;
        }
    }

    /// Renames the selected block. Refused, returning false, when the new name
    /// is empty or another block already has it.
    pub fn rename_selected(&mut self, to: &str) -> bool {
        let to = to.trim();
        let (Some(document), Some(from)) = (self.document.as_mut(), self.selected.clone()) else {
            return false;
        };
        if to.is_empty() || to == from || document.tiles.contains_key(to) {
            return to == from;
        }
        let Some(block) = document.tiles.remove(&from) else {
            return false;
        };
        document.tiles.insert(to.to_owned(), block);
        self.selected = Some(to.to_owned());
        self.dirty = true;
        true
    }

    pub fn selected_block(&self) -> Option<(&str, &TileDefinition)> {
        let name = self.selected.as_deref()?;
        let (name, block) = self.document.as_ref()?.tiles.get_key_value(name)?;
        Some((name.as_str(), block))
    }

    pub fn selected_block_mut(&mut self) -> Option<&mut TileDefinition> {
        let name = self.selected.clone()?;
        self.document.as_mut()?.tiles.get_mut(&name)
    }

    fn insert_as(&mut self, stem: &str, block: TileDefinition) {
        let Some(document) = self.document.as_mut() else {
            return;
        };
        // One more candidate than there are blocks, so one is always free.
        let name = (1..=document.tiles.len() + 1)
            .map(|n| {
                if n == 1 && stem != "block" {
                    format!("{stem}-copy")
                } else {
                    format!("{stem}-{n}")
                }
            })
            .find(|name| !document.tiles.contains_key(name))
            .expect("some name is free");
        document.tiles.insert(name.clone(), block);
        self.selected = Some(name);
        self.dirty = true;
    }
}

/// A block set as written to disk: indented, one field per line, so a change
/// to one block is a change to a few lines of a diff.
pub fn block_set_json(document: &TileSetDocument) -> Result<String, String> {
    serde_json::to_string_pretty(document)
        .map(|json| json + "\n")
        .map_err(|error| error.to_string())
}

/// One face showing all of `sprite`, a whole cell in size.
pub fn face(sprite: &str) -> TileFaceVisual {
    TileFaceVisual {
        sprite: sprite.to_owned(),
        size: [1.0, 1.0],
        offset: [0.0, 0.0],
    }
}

/// A block with every field at the value a document leaving it out means.
fn blank_block() -> TileDefinition {
    serde_json::from_value(serde_json::json!({ "faces": {} }))
        .expect("a block with no faces decodes to every default")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_with(names: &[&str]) -> (tempfile::TempDir, BlockSetEditor) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("terrain.tileset.json");
        let mut document = TileSetDocument {
            format_version: sindri_core::TILESET_FORMAT_VERSION,
            tiles: std::collections::BTreeMap::new(),
        };
        for name in names {
            document.tiles.insert(
                (*name).to_owned(),
                TileDefinition {
                    faces: TileFaces {
                        top: Some(face("grass.png")),
                        ..TileFaces::default()
                    },
                    ..blank_block()
                },
            );
        }
        std::fs::write(&path, block_set_json(&document).unwrap()).unwrap();
        let editor = BlockSetEditor::open(&path);
        (directory, editor)
    }

    #[test]
    fn a_block_set_opens_on_its_first_block_and_saves_back() {
        let (_directory, mut editor) = set_with(&["grass", "stone"]);
        assert_eq!(editor.selected.as_deref(), Some("grass"));
        editor.add_block();
        assert_eq!(editor.selected.as_deref(), Some("block-1"));
        editor.save().expect("the set saves");
        let reopened = BlockSetEditor::open(editor.path());
        let names: Vec<_> = reopened.document.unwrap().tiles.into_keys().collect();
        assert_eq!(names, ["block-1", "grass", "stone"]);
    }

    #[test]
    fn a_duplicate_and_a_rename_never_collide() {
        let (_directory, mut editor) = set_with(&["grass", "stone"]);
        editor.duplicate_selected();
        assert_eq!(editor.selected.as_deref(), Some("grass-copy"));
        assert!(!editor.rename_selected("stone"), "stone is taken");
        assert!(editor.rename_selected("meadow"));
        assert!(
            editor
                .selected_block()
                .is_some_and(|(name, _)| name == "meadow")
        );
        editor.remove_selected();
        assert_eq!(editor.selected.as_deref(), Some("stone"));
    }
}
