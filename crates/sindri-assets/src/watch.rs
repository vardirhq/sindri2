//! Noticing that the file behind an asset has changed.
//!
//! Hot reload for native development, which is the point at which an editor
//! stops being a thing you restart. Save a texture in an image editor and the
//! scene showing it updates.
//!
//! This polls modification times rather than subscribing to filesystem events.
//! A watcher crate would be more efficient and would bring a background thread,
//! a platform-specific event model, and a set of coalescing rules to get wrong;
//! the set of files being watched here is a scene's assets, which is tens, and
//! stating tens of paths once a second costs nothing measurable. The cheaper
//! thing is not always the smaller thing.
//!
//! Native only. A browser has no modification time to read, and no editor to
//! reload into.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use sindri_core::AssetId;

/// What a file looked like when it was last examined.
///
/// Both fields, because either alone misses edits. A filesystem that records
/// modification times to the second cannot distinguish two saves within one
/// second, and a file rewritten with different bytes usually changes length; a
/// file whose length is unchanged usually has a newer time. Together they catch
/// everything except a same-second edit that preserves the length exactly, which
/// is documented rather than pretended away.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl Stamp {
    /// Reads a path, or `None` when there is nothing there.
    ///
    /// A file that does not exist is a state like any other: an asset deleted
    /// and put back is a change, and so is one that appears where a load
    /// previously failed.
    fn of(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        Some(Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        })
    }
}

/// Watches the files behind a set of assets under one root.
#[derive(Clone, Debug)]
pub struct AssetWatch {
    root: PathBuf,
    seen: BTreeMap<AssetId, Option<Stamp>>,
}

impl AssetWatch {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            seen: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where an asset's bytes are read from.
    ///
    /// The same join `FileSystemAssetSource` makes, and it cannot escape the
    /// root: an `AssetId` is validated to be relative with no `..` segments, so
    /// the type has already ruled out the traversal a path would need checking
    /// for.
    pub fn path_of(&self, id: &AssetId) -> PathBuf {
        self.root.join(id.as_str())
    }

    /// Starts watching an asset, taking the file as it is now.
    ///
    /// Recording the current state rather than nothing, so the first poll after
    /// a load does not report the file that was just read as having changed.
    /// The narrow race — the file changing between the read and this call —
    /// costs one missed reload of a file that is being written to repeatedly,
    /// and the next save reports it.
    pub fn watch(&mut self, id: &AssetId) {
        let stamp = Stamp::of(&self.path_of(id));
        self.seen.insert(id.clone(), stamp);
    }

    pub fn forget(&mut self, id: &AssetId) {
        self.seen.remove(id);
    }

    /// Stops watching everything not named.
    pub fn retain(&mut self, keep: &BTreeSet<AssetId>) {
        self.seen.retain(|id, _| keep.contains(id));
    }

    pub fn watching(&self) -> impl ExactSizeIterator<Item = &AssetId> {
        self.seen.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Every watched asset whose file differs from when it was last examined.
    ///
    /// Reporting updates what is remembered, so one change is reported once. A
    /// caller that ignored the answer would not be told again, which is the
    /// right trade: the alternative is a watcher that repeats itself forever
    /// because something downstream declined to act.
    pub fn changed(&mut self) -> Vec<AssetId> {
        let mut changed = Vec::new();
        for (id, seen) in &mut self.seen {
            let now = Stamp::of(&self.root.join(id.as_str()));
            if now != *seen {
                *seen = now;
                changed.push(id.clone());
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use super::*;

    fn id(value: &str) -> AssetId {
        AssetId::new(value).expect("test asset IDs are valid")
    }

    /// Writes a file, waiting first when the content is the same length as what
    /// was there, so a filesystem recording whole seconds still records a
    /// different time.
    fn write(root: &Path, name: &str, bytes: &[u8]) {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if fs::metadata(&path).is_ok_and(|previous| previous.len() == bytes.len() as u64) {
            sleep(Duration::from_millis(1100));
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn watching_an_existing_file_reports_nothing_until_it_changes() {
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), "textures/badge.png", b"first");
        let mut watch = AssetWatch::new(directory.path());
        watch.watch(&id("textures/badge.png"));

        assert!(
            watch.changed().is_empty(),
            "the file it was just told about has not changed"
        );

        write(directory.path(), "textures/badge.png", b"second version");
        assert_eq!(watch.changed(), [id("textures/badge.png")]);
        assert!(
            watch.changed().is_empty(),
            "and one change is reported once"
        );
    }

    /// A rewrite that keeps the length is the case a modification time alone
    /// catches, and the case a length alone misses. Both are read for this.
    #[test]
    fn a_rewrite_of_the_same_length_is_still_a_change() {
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), "art.png", b"aaaa");
        let mut watch = AssetWatch::new(directory.path());
        watch.watch(&id("art.png"));

        write(directory.path(), "art.png", b"bbbb");
        assert_eq!(watch.changed(), [id("art.png")]);
    }

    /// Existing and not existing are both states, so an asset appearing where
    /// one was missing is news — which is how a failed load becomes a working
    /// one without restarting the editor.
    #[test]
    fn a_file_appearing_or_disappearing_is_a_change() {
        let directory = tempfile::tempdir().unwrap();
        let mut watch = AssetWatch::new(directory.path());
        watch.watch(&id("later.png"));
        assert!(watch.changed().is_empty());

        write(directory.path(), "later.png", b"here now");
        assert_eq!(watch.changed(), [id("later.png")], "it appeared");

        fs::remove_file(directory.path().join("later.png")).unwrap();
        assert_eq!(watch.changed(), [id("later.png")], "and it went away");
    }

    /// A scene that stops referencing a texture stops watching it, or every
    /// scene opened in a session would keep polling the last one's files.
    #[test]
    fn only_what_is_still_wanted_is_watched() {
        let directory = tempfile::tempdir().unwrap();
        write(directory.path(), "kept.png", b"kept");
        write(directory.path(), "dropped.png", b"dropped");
        let mut watch = AssetWatch::new(directory.path());
        watch.watch(&id("kept.png"));
        watch.watch(&id("dropped.png"));

        watch.retain(&BTreeSet::from([id("kept.png")]));
        assert_eq!(watch.watching().collect::<Vec<_>>(), [&id("kept.png")]);

        write(directory.path(), "dropped.png", b"changed while unwatched");
        assert!(watch.changed().is_empty());
    }

    /// An asset ID cannot escape the root, so the path is a plain join.
    #[test]
    fn an_asset_resolves_under_the_root() {
        let watch = AssetWatch::new("/project/assets");
        assert_eq!(
            watch.path_of(&id("textures/badge.png")),
            Path::new("/project/assets/textures/badge.png")
        );
        assert!(
            AssetId::new("../escape.png").is_err(),
            "the type rules out what a path check would look for"
        );
    }
}
