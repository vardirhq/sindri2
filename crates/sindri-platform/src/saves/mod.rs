//! Where a game's saved state actually goes.
//!
//! The store in `sindri-core` is what a game reads and writes; a backend here is
//! what puts it somewhere. Splitting them is what lets the same gameplay run
//! against a file, against a browser, and against a test that touches neither —
//! and it keeps the decision of *where* with the host, which is the only part of
//! the stack that knows.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod file;

#[cfg(target_arch = "wasm32")]
pub use browser::BrowserSaves;
#[cfg(not(target_arch = "wasm32"))]
pub use file::FileSaves;

use sindri_core::{SaveDocument, SaveReadError};

/// Somewhere a game's saved state can be kept.
///
/// Both halves may fail, and a failure is reported rather than swallowed: a
/// save that silently did not happen is the worst outcome available.
pub trait SaveBackend {
    /// Reads what was stored, or `None` when nothing has been.
    ///
    /// The difference matters: nothing stored is a first run, and something
    /// stored that will not parse is a save worth telling someone about.
    fn read(&mut self) -> Result<Option<SaveDocument>, SaveReadError>;

    /// Replaces what is stored.
    ///
    /// Must be all-or-nothing where the platform can manage it. A save half
    /// written is a save destroyed, and it is destroyed at the exact moment
    /// someone's machine lost power mid-run — which is when they least deserve
    /// it.
    fn write(&mut self, document: &SaveDocument) -> Result<(), SaveWriteError>;
}

/// Why saved state could not be stored.
#[derive(Debug, thiserror::Error)]
pub enum SaveWriteError {
    #[error("the save could not be encoded: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the save could not be written: {0}")]
    Unwritable(String),
}

/// A backend that keeps a save in memory and nowhere else.
///
/// The default, so a headless run and a test have somewhere to write without
/// choosing a path or touching a disk. Everything it holds goes when it does,
/// which is the honest behaviour for a host that never said where to put
/// anything.
#[derive(Debug, Default)]
pub struct MemorySaves {
    stored: Option<SaveDocument>,
    /// How many times a document has been written, which a test can assert on
    /// to prove a game is not writing every frame.
    writes: usize,
}

impl MemorySaves {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A backend that already holds this, for a test about loading.
    #[must_use]
    pub fn holding(document: SaveDocument) -> Self {
        Self {
            stored: Some(document),
            writes: 0,
        }
    }

    /// How many times this has been written to.
    #[must_use]
    pub const fn writes(&self) -> usize {
        self.writes
    }
}

impl SaveBackend for MemorySaves {
    fn read(&mut self) -> Result<Option<SaveDocument>, SaveReadError> {
        Ok(self.stored.clone())
    }

    fn write(&mut self, document: &SaveDocument) -> Result<(), SaveWriteError> {
        self.stored = Some(document.clone());
        self.writes += 1;
        Ok(())
    }
}

/// A backend that reports every save as unreadable.
///
/// For proving that a game handles a damaged save, which is a path no test can
/// otherwise reach without corrupting a real file.
#[derive(Debug, Default)]
pub struct DamagedSaves;

impl SaveBackend for DamagedSaves {
    fn read(&mut self) -> Result<Option<SaveDocument>, SaveReadError> {
        Err(SaveReadError::Unreadable(
            "this backend always is".to_owned(),
        ))
    }

    fn write(&mut self, _document: &SaveDocument) -> Result<(), SaveWriteError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DamagedSaves, MemorySaves, SaveBackend};
    use sindri_core::{SaveState, SaveStore, SaveValue};

    #[test]
    fn a_fresh_backend_holds_nothing() {
        assert!(MemorySaves::new().read().expect("readable").is_none());
    }

    #[test]
    fn what_was_written_reads_back() {
        let mut backend = MemorySaves::new();
        let mut store = SaveStore::default();
        store.set("score", SaveValue::Number(5.0));
        backend.write(&store.to_document()).expect("writable");

        let reopened = SaveStore::opened(backend.read());
        assert!((reopened.number("score", 0.0) - 5.0).abs() < 1.0e-9);
        assert_eq!(backend.writes(), 1);
    }

    #[test]
    fn a_damaged_backend_gives_a_store_that_says_so() {
        let store = SaveStore::opened(DamagedSaves.read());
        assert_eq!(store.state(), SaveState::Damaged);
    }
}
