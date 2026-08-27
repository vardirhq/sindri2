//! What the browser is looking at, and which rows that leaves showing.
//!
//! The tree itself is read from disk once and never changes between refreshes
//! (`crate::project::ProjectTree`). What changes is what the *browser* is doing
//! with it: which folder it is scoped to, which folders are folded away, and
//! which asset is selected. Kept apart from the drawing so that "which rows
//! does this state show, at what depth" is a question a test can ask without a
//! window.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::project::ProjectEntry;

/// Where the project browser is looking.
#[derive(Clone, Debug, Default)]
pub(crate) struct BrowserState {
    /// The asset the browser has selected.
    ///
    /// Not the same thing as the open scene, which the browser also marks. It
    /// showed only the latter, so the scene file wore the selection band
    /// permanently and clicking anything else marked nothing at all.
    pub(crate) selected: Option<PathBuf>,
    /// The folder the listing is scoped to, or the whole project.
    pub(crate) folder: Option<PathBuf>,
    /// Folders folded closed. Absent means open, so a project opens showing
    /// everything, as it always did.
    collapsed: BTreeSet<PathBuf>,
    /// Which sliced images are showing their sprites.
    ///
    /// Collapsed until asked for, because a sheet is as likely to hold
    /// sixty-four frames as four.
    pub(crate) expanded_sheets: BTreeSet<PathBuf>,
}

impl BrowserState {
    pub(crate) fn is_folded(&self, path: &Path) -> bool {
        self.collapsed.contains(path)
    }

    pub(crate) fn toggle_fold(&mut self, path: &Path) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_owned());
        }
    }

    /// Scopes the listing to one folder, and opens it so its contents show.
    ///
    /// Scoping into a folder that was folded and then seeing nothing is the
    /// sort of small betrayal that makes a panel feel broken.
    pub(crate) fn look_in(&mut self, folder: Option<&Path>) {
        if let Some(folder) = folder {
            self.collapsed.remove(folder);
        }
        self.folder = folder.map(Path::to_path_buf);
    }

    /// Whether this entry is inside whatever the browser is scoped to.
    fn in_scope(&self, entry: &ProjectEntry) -> bool {
        self.folder
            .as_ref()
            .is_none_or(|folder| entry.path.starts_with(folder) && entry.path != *folder)
    }

    /// Whether a folded folder between here and the scope hides this entry.
    fn folded_away(&self, entry: &ProjectEntry) -> bool {
        self.collapsed.iter().any(|folded| {
            entry.path.starts_with(folded) && entry.path != *folded && self.in_folder(folded)
        })
    }

    fn in_folder(&self, path: &Path) -> bool {
        self.folder
            .as_ref()
            .is_none_or(|folder| path.starts_with(folder) && path != folder)
    }

    /// The rows a listing shows, each with the depth it is drawn at.
    ///
    /// Depth is relative to whatever the browser is scoped to, so looking
    /// inside a folder starts its contents at the left edge rather than three
    /// indents in. A search is a flat list of matches, because an indentation
    /// under a parent the search removed points at nothing.
    pub(crate) fn rows<'a>(
        &self,
        entries: &[&'a ProjectEntry],
        searching: bool,
    ) -> Vec<(&'a ProjectEntry, usize)> {
        // Measured from the scope's own row rather than from the path, so a
        // folder's contents start at the left edge however deep the folder is.
        let base = self.folder.as_ref().and_then(|folder| {
            entries
                .iter()
                .find(|entry| entry.path == *folder)
                .map(|entry| entry.depth + 1)
        });
        entries
            .iter()
            .filter(|entry| self.in_scope(entry))
            .filter(|entry| searching || !self.folded_away(entry))
            .map(|entry| {
                let depth = if searching {
                    0
                } else {
                    entry.depth.saturating_sub(base.unwrap_or(0))
                };
                (*entry, depth)
            })
            .collect()
    }

    /// Whether the browser is looking inside a folder rather than at the whole
    /// project.
    pub(crate) const fn is_scoped(&self) -> bool {
        self.folder.is_some()
    }

    /// What the listing is of, for a header that has one line to spend.
    pub(crate) fn label_within(&self, project: &crate::project::ProjectTree) -> String {
        self.folder.as_ref().map_or_else(
            || project.label(),
            |folder| {
                folder.file_name().map_or_else(
                    || project.label(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BrowserState, ProjectEntry};
    use crate::project::AssetKind;

    fn entry(relative: &str, kind: AssetKind) -> ProjectEntry {
        let path = PathBuf::from("/project").join(relative);
        ProjectEntry {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            relative: relative.to_owned(),
            kind,
            depth: relative.matches('/').count(),
            path,
            sprites: Vec::new(),
        }
    }

    fn tree() -> Vec<ProjectEntry> {
        vec![
            entry("level.scene.json", AssetKind::Scene),
            entry("scripts", AssetKind::Folder),
            entry("scripts/spin.decay", AssetKind::Script),
            entry("textures", AssetKind::Folder),
            entry("textures/tiles.png", AssetKind::Texture),
        ]
    }

    fn shown(state: &BrowserState, tree: &[ProjectEntry]) -> Vec<(String, usize)> {
        let entries: Vec<&ProjectEntry> = tree.iter().collect();
        state
            .rows(&entries, false)
            .into_iter()
            .map(|(entry, depth)| (entry.relative.clone(), depth))
            .collect()
    }

    /// The whole tree, indented, which is what the browser always showed —
    /// and, until folding existed, the only thing it could show.
    #[test]
    fn an_untouched_browser_shows_everything() {
        let tree = tree();
        assert_eq!(
            shown(&BrowserState::default(), &tree),
            vec![
                ("level.scene.json".to_owned(), 0),
                ("scripts".to_owned(), 0),
                ("scripts/spin.decay".to_owned(), 1),
                ("textures".to_owned(), 0),
                ("textures/tiles.png".to_owned(), 1),
            ]
        );
    }

    /// A folded folder takes its contents with it and stays visible itself,
    /// which is the difference between folding a folder and hiding one.
    #[test]
    fn folding_a_folder_hides_what_is_under_it() {
        let tree = tree();
        let mut state = BrowserState::default();
        state.toggle_fold(&PathBuf::from("/project/scripts"));
        assert_eq!(
            shown(&state, &tree),
            vec![
                ("level.scene.json".to_owned(), 0),
                ("scripts".to_owned(), 0),
                ("textures".to_owned(), 0),
                ("textures/tiles.png".to_owned(), 1),
            ]
        );

        state.toggle_fold(&PathBuf::from("/project/scripts"));
        assert_eq!(shown(&state, &tree).len(), 5, "and folding again unfolds");
    }

    /// Looking inside a folder lists that folder and nothing else, starting at
    /// the left edge rather than one indent in for every level above it.
    #[test]
    fn looking_in_a_folder_scopes_the_listing() {
        let tree = tree();
        let mut state = BrowserState::default();
        state.look_in(Some(&PathBuf::from("/project/textures")));
        assert_eq!(
            shown(&state, &tree),
            vec![("textures/tiles.png".to_owned(), 0)]
        );
        assert!(state.is_scoped());

        state.look_in(None);
        assert_eq!(shown(&state, &tree).len(), 5, "and the project comes back");
        assert!(!state.is_scoped());
    }

    /// Scoping into a folded folder shows its contents rather than an empty
    /// panel, which is what folding it earlier would otherwise have caused.
    #[test]
    fn looking_in_a_folded_folder_opens_it() {
        let tree = tree();
        let mut state = BrowserState::default();
        let scripts = PathBuf::from("/project/scripts");
        state.toggle_fold(&scripts);
        state.look_in(Some(&scripts));
        assert_eq!(
            shown(&state, &tree),
            vec![("scripts/spin.decay".to_owned(), 0)]
        );
    }

    /// A search is a flat list of matches wherever they are, because an
    /// indentation under a parent the search removed points at nothing.
    #[test]
    fn a_search_flattens_and_ignores_folds() {
        let tree = tree();
        let mut state = BrowserState::default();
        state.toggle_fold(&PathBuf::from("/project/scripts"));
        let entries: Vec<&ProjectEntry> = tree.iter().collect();
        let rows = state.rows(&entries, true);
        assert_eq!(rows.len(), 5);
        assert!(rows.iter().all(|(_, depth)| *depth == 0));
    }
}
