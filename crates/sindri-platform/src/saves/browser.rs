//! A save kept in the page's own storage.
//!
//! `localStorage` rather than anything larger: a save is a handful of numbers
//! and truths, it has to survive a tab closing, and every alternative in a
//! browser is asynchronous. An asynchronous save would mean a game asking
//! whether its progress had landed yet, which is a question no gameplay script
//! should have to think about.

use sindri_core::{SaveDocument, SaveReadError};
use wasm_bindgen::JsValue;

use super::{SaveBackend, SaveWriteError};

/// A save kept under one key in the page's storage.
#[derive(Clone, Debug)]
pub struct BrowserSaves {
    key: String,
}

impl BrowserSaves {
    /// A save under a key the host chose.
    ///
    /// Named by the host because a page may carry more than one thing, and two
    /// games sharing an origin must not share a save.
    #[must_use]
    pub fn under(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }

    /// The page's storage, or why it cannot be reached.
    ///
    /// It genuinely can be missing: private browsing, storage disabled, an
    /// origin that is not allowed one. That is a reportable condition rather
    /// than a panic, because a game that cannot save should say so and keep
    /// playing.
    fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .ok_or_else(|| "there is no window".to_owned())?
            .local_storage()
            .map_err(|error| describe(&error))?
            .ok_or_else(|| "this page is not allowed local storage".to_owned())
    }
}

impl SaveBackend for BrowserSaves {
    fn read(&mut self) -> Result<Option<SaveDocument>, SaveReadError> {
        let storage = Self::storage().map_err(SaveReadError::Unreadable)?;
        match storage.get_item(&self.key) {
            Ok(Some(text)) => Ok(Some(serde_json::from_str(&text)?)),
            // Nothing stored yet is a first run, not a failure.
            Ok(None) => Ok(None),
            Err(error) => Err(SaveReadError::Unreadable(describe(&error))),
        }
    }

    fn write(&mut self, document: &SaveDocument) -> Result<(), SaveWriteError> {
        let text = serde_json::to_string(document)?;
        let storage = Self::storage().map_err(SaveWriteError::Unwritable)?;
        // One key, replaced whole. `localStorage` sets a key atomically, so
        // there is no half-written save to guard against the way there is on a
        // filesystem.
        storage
            .set_item(&self.key, &text)
            .map_err(|error| SaveWriteError::Unwritable(describe(&error)))
    }
}

/// A browser error as something a log can carry.
fn describe(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
