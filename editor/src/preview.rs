//! Looking at a file the browser lists.
//!
//! The project browser listed the language's own source files and could do
//! nothing with any of them: a `.decay` script was a row and a name, in an
//! engine whose headline capability is scripting. Selecting one shows it now,
//! the same way selecting an image opens the slicer.
//!
//! Read-only, deliberately. An editor that opens a script in a text box is
//! promising to be a code editor — syntax, errors at the line they are on,
//! find, undo of its own — and half of that is worse than none. What this
//! answers is the question the browser could not: *what is in this file*.

use std::path::{Path, PathBuf};

use crate::project::AssetKind;

/// How much of a file is read.
///
/// A preview is for reading, and nobody reads a megabyte in a dock. A file
/// longer than this is shown to its cut and says so, which beats a panel that
/// stalls the frame it is opened on.
const MAX_BYTES: usize = 64 * 1024;

/// A file the inspector is showing the contents of.
pub struct TextPreview {
    path: PathBuf,
    /// The text as read, or the reason it could not be.
    body: Result<String, String>,
    /// Whether the file went on past what was read.
    truncated: bool,
}

impl TextPreview {
    /// Reads a file to look at, cut to what is worth showing.
    pub fn open(path: &Path) -> Self {
        let (body, truncated) = match std::fs::read(path) {
            Err(error) => (Err(error.to_string()), false),
            Ok(bytes) => {
                let truncated = bytes.len() > MAX_BYTES;
                let kept = &bytes[..bytes.len().min(MAX_BYTES)];
                // Lossy rather than refusing: a source file with one stray
                // byte in it is still a source file, and the point of looking
                // at it may be to find that byte.
                (Ok(String::from_utf8_lossy(kept).into_owned()), truncated)
            }
        };
        Self {
            path: path.to_path_buf(),
            body,
            truncated,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What the file is called, which is the panel's heading.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }

    /// The text, or the reason there is none.
    pub fn body(&self) -> Result<&str, &str> {
        self.body.as_deref().map_err(String::as_str)
    }

    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// How many lines it holds, for a panel that has one line to say so.
    pub fn lines(&self) -> usize {
        self.body.as_ref().map_or(0, |text| text.lines().count())
    }
}

/// Whether the editor can show this file as text.
///
/// By kind rather than by sniffing the bytes: the browser already decides what
/// a file is from its name, and a preview that opened a `.png` because the
/// first bytes happened to decode would be showing the wrong thing convincingly.
///
/// Fonts, images and audio are absent because reading them as text says
/// nothing. What they need is a preview of their own — a rendered sample, a
/// picture, a play button — and offering a wall of mojibake instead would be
/// worse than the row that at least admitted it could do nothing.
pub fn is_readable(path: &Path) -> bool {
    matches!(
        AssetKind::of_path(path),
        AssetKind::Script | AssetKind::Scene | AssetKind::Sheet | AssetKind::Other
    ) && !looks_binary(path)
}

/// The extensions `AssetKind::Other` covers that are not text.
///
/// `Other` is the browser's "something else", which is mostly text — a README,
/// a `.toml`, a licence — and occasionally not.
fn looks_binary(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase());
    matches!(
        extension.as_deref(),
        Some("zip" | "gz" | "tar" | "bin" | "exe" | "dll" | "so" | "dylib" | "pdf" | "wasm" | "db")
    )
}

#[cfg(test)]
mod tests;
