//! What a game remembers between runs.
//!
//! A save is a **flat, versioned key/value document** rather than a tree. That
//! is not a shortcut: Decay holds numbers, truths and text and nothing else, so
//! a structure a script could not build is a structure nothing could write. A
//! game that wants `progress.best_wave` and `settings.volume` writes those keys,
//! and the format stays something a person can read and repair.
//!
//! Editor preferences are a different thing entirely and stay where they are.
//! A save belongs to the *game* — it is the player's, it ships with the build,
//! and it must round-trip identically in a browser and on a desktop. Editor
//! state belongs to whoever is running the editor and never leaves their
//! machine.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The version this build writes.
///
/// Bumped only when a change would make an older reader wrong. Adding a key
/// does not: a reader that does not know a key ignores it, and a reader that
/// wants one it cannot find uses the fallback the caller gave.
pub const SAVE_FORMAT_VERSION: u32 = 1;

/// One value a game remembers.
///
/// The three things a script can hold, and no others. A save that could carry a
/// shape Decay cannot express would be a save only Rust could write.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SaveValue {
    Flag(bool),
    Number(f64),
    Text(String),
}

/// Everything a game remembers, and the version that wrote it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SaveDocument {
    pub version: u32,
    /// Ordered, so the same saved state is the same bytes.
    ///
    /// A file that changes when nothing changed is one nobody can diff, and a
    /// save that cannot be diffed is one nobody can debug.
    #[serde(default)]
    pub values: BTreeMap<String, SaveValue>,
}

impl Default for SaveDocument {
    fn default() -> Self {
        Self {
            version: SAVE_FORMAT_VERSION,
            values: BTreeMap::new(),
        }
    }
}

/// How a game's stored state was found.
///
/// Every outcome is one gameplay can act on, which is the point: a title screen
/// that says "your progress could not be read" is a better game than one that
/// silently starts a new save over the top of the old one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SaveState {
    /// Nothing has been stored yet. A first run.
    #[default]
    New,
    /// Read, and written by this version or an older one.
    Loaded,
    /// Something was there and could not be understood.
    ///
    /// Kept separate from `New` because they call for different things: a first
    /// run starts cheerfully, and a damaged save is worth telling someone about
    /// before their progress is written over.
    Damaged,
    /// Written by a newer build than this one.
    ///
    /// The values are not loaded, because a reader that guessed at a format it
    /// does not know would corrupt the newer save the moment it wrote back.
    FromNewer { found: u32 },
}

impl SaveState {
    /// Whether values from storage are in hand.
    #[must_use]
    pub const fn has_values(self) -> bool {
        matches!(self, Self::Loaded)
    }
}

/// A game's stored state, in memory, and whether it needs writing out.
///
/// The store is the thing scripts read and write; a backend is what puts it
/// somewhere. Splitting them is what lets the same gameplay run against a file,
/// a browser, and a test that touches neither.
#[derive(Clone, Debug, Default)]
pub struct SaveStore {
    document: SaveDocument,
    state: SaveState,
    dirty: bool,
}

impl SaveStore {
    /// A store holding what was read, or saying why it holds nothing.
    #[must_use]
    pub fn opened(loaded: Result<Option<SaveDocument>, SaveReadError>) -> Self {
        match loaded {
            Ok(None) => Self::default(),
            Ok(Some(document)) if document.version > SAVE_FORMAT_VERSION => Self {
                document: SaveDocument::default(),
                state: SaveState::FromNewer {
                    found: document.version,
                },
                dirty: false,
            },
            Ok(Some(document)) => Self {
                document,
                state: SaveState::Loaded,
                dirty: false,
            },
            Err(_) => Self {
                document: SaveDocument::default(),
                state: SaveState::Damaged,
                dirty: false,
            },
        }
    }

    /// How this store's contents were found.
    #[must_use]
    pub const fn state(&self) -> SaveState {
        self.state
    }

    /// Whether anything has changed since it was last written out.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Whether a key has a value.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.document.values.contains_key(key)
    }

    /// A stored number, or `fallback` when there is none.
    ///
    /// A fallback rather than an optional, because every caller has one — a
    /// starting score, a default volume — and a save is mostly read on a first
    /// run when nothing is there.
    #[must_use]
    pub fn number(&self, key: &str, fallback: f64) -> f64 {
        match self.document.values.get(key) {
            Some(SaveValue::Number(value)) => *value,
            // A flag read as a number is a script asking the wrong question,
            // and answering 1 would let the mistake run for a while.
            _ => fallback,
        }
    }

    /// A stored truth, or `fallback` when there is none.
    #[must_use]
    pub fn flag(&self, key: &str, fallback: bool) -> bool {
        match self.document.values.get(key) {
            Some(SaveValue::Flag(value)) => *value,
            _ => fallback,
        }
    }

    /// A stored string, or `fallback` when there is none.
    #[must_use]
    pub fn text<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        match self.document.values.get(key) {
            Some(SaveValue::Text(value)) => value,
            _ => fallback,
        }
    }

    /// Stores a value, marking the store as needing to be written out.
    pub fn set(&mut self, key: impl Into<String>, value: SaveValue) {
        let key = key.into();
        // Writing the same value again is not a change. A game that stores its
        // volume every frame should not make the disk busy.
        if self.document.values.get(&key) == Some(&value) {
            return;
        }
        self.document.values.insert(key, value);
        self.dirty = true;
    }

    /// Forgets everything, which is what "reset my progress" means.
    ///
    /// Dirty afterwards even though nothing is left, because an empty save that
    /// was never written out is a reset that did not happen.
    pub fn clear(&mut self) {
        if !self.document.values.is_empty() {
            self.document.values.clear();
        }
        self.document.version = SAVE_FORMAT_VERSION;
        self.dirty = true;
    }

    /// What should be written out, stamped with this build's version.
    #[must_use]
    pub fn to_document(&self) -> SaveDocument {
        SaveDocument {
            version: SAVE_FORMAT_VERSION,
            values: self.document.values.clone(),
        }
    }

    /// Marks the store as written out.
    ///
    /// Called by whoever did the writing, because only they know it succeeded:
    /// clearing the flag before the bytes landed would lose the next write too.
    pub const fn mark_written(&mut self) {
        self.dirty = false;
        // A store that has been written has been stored, whatever it was
        // before: a damaged save that a game has replaced is no longer damaged.
        self.state = SaveState::Loaded;
    }
}

/// Why stored state could not be read.
#[derive(Debug, thiserror::Error)]
pub enum SaveReadError {
    #[error("the stored save is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the stored save could not be read: {0}")]
    Unreadable(String),
}

#[cfg(test)]
mod tests {
    use super::{
        SAVE_FORMAT_VERSION, SaveDocument, SaveReadError, SaveState, SaveStore, SaveValue,
    };

    #[test]
    fn a_first_run_finds_nothing_and_says_so() {
        let store = SaveStore::opened(Ok(None));
        assert_eq!(store.state(), SaveState::New);
        assert!(!store.is_dirty());
        assert!((store.number("score", 7.0) - 7.0).abs() < 1.0e-9);
    }

    #[test]
    fn what_was_stored_comes_back() {
        let mut store = SaveStore::default();
        store.set("best_wave", SaveValue::Number(12.0));
        store.set("seen_intro", SaveValue::Flag(true));
        let written = store.to_document();

        let reopened = SaveStore::opened(Ok(Some(written)));
        assert_eq!(reopened.state(), SaveState::Loaded);
        assert!((reopened.number("best_wave", 0.0) - 12.0).abs() < 1.0e-9);
        assert!(reopened.flag("seen_intro", false));
    }

    #[test]
    fn a_missing_key_is_the_fallback() {
        let store = SaveStore::default();
        assert!((store.number("absent", 3.0) - 3.0).abs() < 1.0e-9);
        assert!(store.flag("absent", true));
        assert_eq!(store.text("absent", "none"), "none");
        assert!(!store.has("absent"));
    }

    /// A script asking the wrong question gets its fallback rather than a
    /// plausible-looking answer that lets the mistake run.
    #[test]
    fn a_value_of_the_wrong_kind_is_the_fallback_too() {
        let mut store = SaveStore::default();
        store.set("volume", SaveValue::Flag(true));
        assert!((store.number("volume", 0.5) - 0.5).abs() < 1.0e-9);
    }

    /// A title screen that says "your progress could not be read" is a better
    /// game than one that silently writes over it.
    #[test]
    fn something_unreadable_is_not_mistaken_for_a_first_run() {
        let store = SaveStore::opened(Err(SaveReadError::Unreadable("torn".to_owned())));
        assert_eq!(store.state(), SaveState::Damaged);
        assert_ne!(store.state(), SaveState::New);
        assert!(!store.state().has_values());
    }

    /// Guessing at a format this build does not know would corrupt the newer
    /// save the moment it wrote back.
    #[test]
    fn a_newer_save_is_reported_and_not_read() {
        let store = SaveStore::opened(Ok(Some(SaveDocument {
            version: SAVE_FORMAT_VERSION + 5,
            values: [("best_wave".to_owned(), SaveValue::Number(99.0))]
                .into_iter()
                .collect(),
        })));
        assert_eq!(
            store.state(),
            SaveState::FromNewer {
                found: SAVE_FORMAT_VERSION + 5
            }
        );
        assert!(
            (store.number("best_wave", 0.0)).abs() < 1.0e-9,
            "it was read"
        );
    }

    /// A reader that does not know a key ignores it, which is what lets a build
    /// add one without a version bump.
    #[test]
    fn an_unknown_key_from_an_older_save_is_kept_rather_than_dropped() {
        let document = SaveDocument {
            version: SAVE_FORMAT_VERSION,
            values: [("from_the_future".to_owned(), SaveValue::Number(1.0))]
                .into_iter()
                .collect(),
        };
        let store = SaveStore::opened(Ok(Some(document)));
        assert!(store.has("from_the_future"));
        assert!(store.to_document().values.contains_key("from_the_future"));
    }

    #[test]
    fn storing_marks_the_store_as_needing_writing() {
        let mut store = SaveStore::default();
        assert!(!store.is_dirty());
        store.set("score", SaveValue::Number(1.0));
        assert!(store.is_dirty());
        store.mark_written();
        assert!(!store.is_dirty());
    }

    /// A game that stores its volume every frame should not make the disk busy.
    #[test]
    fn storing_the_same_value_again_is_not_a_change() {
        let mut store = SaveStore::default();
        store.set("volume", SaveValue::Number(0.5));
        store.mark_written();
        store.set("volume", SaveValue::Number(0.5));
        assert!(!store.is_dirty());
        store.set("volume", SaveValue::Number(0.6));
        assert!(store.is_dirty());
    }

    /// An empty save that was never written out is a reset that did not happen.
    #[test]
    fn clearing_needs_writing_out() {
        let mut store = SaveStore::default();
        store.set("score", SaveValue::Number(5.0));
        store.mark_written();
        store.clear();
        assert!(store.is_dirty());
        assert!(!store.has("score"));
    }

    /// A damaged save that a game has replaced is no longer damaged.
    #[test]
    fn writing_over_a_damaged_save_settles_it() {
        let mut store = SaveStore::opened(Err(SaveReadError::Unreadable("torn".to_owned())));
        store.set("score", SaveValue::Number(1.0));
        store.mark_written();
        assert_eq!(store.state(), SaveState::Loaded);
    }

    /// A file that changes when nothing changed is one nobody can diff.
    #[test]
    fn the_same_state_is_the_same_bytes() {
        let mut first = SaveStore::default();
        let mut second = SaveStore::default();
        for key in ["c", "a", "b"] {
            first.set(key, SaveValue::Number(1.0));
        }
        for key in ["b", "c", "a"] {
            second.set(key, SaveValue::Number(1.0));
        }
        let render = |store: &SaveStore| {
            serde_json::to_string(&store.to_document()).expect("a document serializes")
        };
        assert_eq!(render(&first), render(&second));
    }

    #[test]
    fn a_document_round_trips_through_json() {
        let mut store = SaveStore::default();
        store.set("score", SaveValue::Number(1200.0));
        store.set("unlocked", SaveValue::Flag(true));
        store.set("name", SaveValue::Text("Ada".to_owned()));
        let json = serde_json::to_string(&store.to_document()).expect("serializes");
        let back: SaveDocument = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, store.to_document());
    }
}
