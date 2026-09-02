//! A save kept in a file, replaced all at once.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sindri_core::{SaveDocument, SaveReadError};

use super::{SaveBackend, SaveWriteError};

/// A save kept at a path the host chose.
///
/// The engine does not pick the path. Where a game's save belongs is a question
/// about the platform and the person using it — an application data directory,
/// a portable folder beside the executable, a location a launcher passed in —
/// and the host is the only part of the stack that knows the answer.
#[derive(Clone, Debug)]
pub struct FileSaves {
    path: PathBuf,
}

impl FileSaves {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file this reads and writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where the replacement is built before it takes over.
    fn staging(&self) -> PathBuf {
        let mut staging = self.path.clone();
        // An extra extension rather than a hidden name, so a leftover is
        // obviously ours and obviously incomplete if anyone ever finds one.
        let name = staging.file_name().map_or_else(
            || "save.writing".to_owned(),
            |name| format!("{}.writing", name.to_string_lossy()),
        );
        staging.set_file_name(name);
        staging
    }
}

impl SaveBackend for FileSaves {
    fn read(&mut self) -> Result<Option<SaveDocument>, SaveReadError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
            // Nothing stored yet is a first run, not a failure.
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(SaveReadError::Unreadable(error.to_string())),
        }
    }

    fn write(&mut self, document: &SaveDocument) -> Result<(), SaveWriteError> {
        let text = serde_json::to_string_pretty(document)?;
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| unwritable(parent, &error))?;
        }
        // Written beside the real file and then renamed over it. A save half
        // written is a save destroyed, and it is destroyed at the exact moment
        // someone's machine lost power mid-run. A rename on the same
        // filesystem is the one operation that cannot leave a half of either.
        let staging = self.staging();
        fs::write(&staging, text).map_err(|error| unwritable(&staging, &error))?;
        fs::rename(&staging, &self.path).map_err(|error| {
            // The replacement did not take, so it is litter rather than a save.
            let _ = fs::remove_file(&staging);
            unwritable(&self.path, &error)
        })
    }
}

fn unwritable(path: &Path, error: &std::io::Error) -> SaveWriteError {
    SaveWriteError::Unwritable(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{FileSaves, SaveBackend};
    use sindri_core::{SaveState, SaveStore, SaveValue};

    /// A directory that cleans up after itself, without a dependency.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("sindri-save-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        fn file(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_that_is_not_there_yet_is_a_first_run() {
        let scratch = Scratch::new("absent");
        let mut backend = FileSaves::at(scratch.file("save.json"));
        assert!(backend.read().expect("readable").is_none());
        assert_eq!(SaveStore::opened(backend.read()).state(), SaveState::New);
    }

    #[test]
    fn what_was_written_reads_back_from_disk() {
        let scratch = Scratch::new("round-trip");
        let mut backend = FileSaves::at(scratch.file("save.json"));
        let mut store = SaveStore::default();
        store.set("best_wave", SaveValue::Number(12.0));
        store.set("seen_intro", SaveValue::Flag(true));
        backend.write(&store.to_document()).expect("writable");

        let reopened = SaveStore::opened(backend.read());
        assert_eq!(reopened.state(), SaveState::Loaded);
        assert!((reopened.number("best_wave", 0.0) - 12.0).abs() < 1.0e-9);
        assert!(reopened.flag("seen_intro", false));
    }

    /// The directory a host names may not exist on a first run.
    #[test]
    fn a_missing_directory_is_made_rather_than_refused() {
        let scratch = Scratch::new("nested");
        let mut backend = FileSaves::at(scratch.file("deeper/still/save.json"));
        backend
            .write(&SaveStore::default().to_document())
            .expect("writable");
        assert!(backend.path().exists());
    }

    /// Something there that will not parse is worth telling someone about,
    /// rather than being mistaken for a first run.
    #[test]
    fn a_torn_file_is_reported_rather_than_ignored() {
        let scratch = Scratch::new("torn");
        let path = scratch.file("save.json");
        std::fs::write(&path, "{ this is not json").expect("a torn file");
        let store = SaveStore::opened(FileSaves::at(&path).read());
        assert_eq!(store.state(), SaveState::Damaged);
    }

    /// A rename leaves no half of either file.
    #[test]
    fn writing_leaves_no_staging_file_behind() {
        let scratch = Scratch::new("staging");
        let path = scratch.file("save.json");
        let mut backend = FileSaves::at(&path);
        backend
            .write(&SaveStore::default().to_document())
            .expect("writable");
        let leftovers: Vec<_> = std::fs::read_dir(&scratch.0)
            .expect("a directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".writing"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// The second write replaces the first rather than joining it.
    #[test]
    fn a_second_write_replaces_the_first() {
        let scratch = Scratch::new("replace");
        let mut backend = FileSaves::at(scratch.file("save.json"));
        let mut store = SaveStore::default();
        store.set("score", SaveValue::Number(1.0));
        backend.write(&store.to_document()).expect("writable");
        store.set("score", SaveValue::Number(2.0));
        backend.write(&store.to_document()).expect("writable");

        let reopened = SaveStore::opened(backend.read());
        assert!((reopened.number("score", 0.0) - 2.0).abs() < 1.0e-9);
    }
}
