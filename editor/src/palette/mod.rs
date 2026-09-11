//! One field that finds anything, and does it.
//!
//! The canvas arrangement hides more than the docked one did: a panel someone
//! closed is reachable only through a menu, and the answer to "where is the
//! console" should not be "learn the View menu". So there is one place to type
//! what you want — a panel, an entity, a file, a scene, a thing to do — and the
//! editor works out which of those it was.
//!
//! What the palette is worth depends entirely on ranking, so the matching lives
//! in [`score`] where it can be tested against the orderings a person would
//! expect, and the rest of this file is only about what the candidates are.

mod score;

pub use score::{Score, score, score_terms};

use sindri_core::EntityId;

use crate::dock::{Panel, Preset};

/// How many results are offered at once.
///
/// A palette that lists everything is a file listing with a text field on top.
/// Ten is enough to show that a better match exists further down and few enough
/// that the right one is usually visible without arrowing.
pub const VISIBLE: usize = 10;

/// What choosing a result does.
///
/// Deliberately a description rather than a closure: an action that is data can
/// be matched on by the editor that owns the state it needs, tested without a
/// window, and — when the time comes — proposed by something other than a
/// person typing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Show a panel, moving it nowhere if it is already open.
    ShowPanel(Panel),
    /// Replace the arrangement.
    UsePreset(Preset),
    /// Select an entity and frame it.
    GoToEntity(EntityId),
    /// Choose a file in the project browser.
    SelectAsset(String),
    /// Open a scene, asking about unsaved work first.
    OpenScene(String),
    /// One of the editor's verbs.
    Run(Verb),
}

/// A thing the editor can be told to do that is not about one object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verb {
    Save,
    SaveAs,
    ReloadFromDisk,
    NewScene,
    OpenScene,
    OpenProject,
    Welcome,
    Undo,
    Redo,
    TogglePlay,
    TogglePause,
    Step,
    DiscardChanges,
}

impl Verb {
    /// Every verb, with what it is called and what kind of thing it is.
    ///
    /// One table rather than three parallel matches: a verb added here shows up
    /// in the palette, with its label and its grouping, or it does not compile.
    pub const ALL: [(Self, &'static str, Kind); 13] = [
        (Self::Save, "Save scene", Kind::File),
        (Self::SaveAs, "Save scene as…", Kind::File),
        (Self::ReloadFromDisk, "Reload scene from disk", Kind::File),
        (Self::NewScene, "New scene…", Kind::File),
        (Self::OpenScene, "Open scene…", Kind::File),
        (Self::OpenProject, "Open project…", Kind::File),
        (Self::Welcome, "Welcome…", Kind::File),
        (Self::Undo, "Undo", Kind::Edit),
        (Self::Redo, "Redo", Kind::Edit),
        (Self::TogglePlay, "Play or stop", Kind::Run),
        (Self::TogglePause, "Pause or resume", Kind::Run),
        (Self::Step, "Run one fixed step", Kind::Run),
        (Self::DiscardChanges, "Discard changes", Kind::Edit),
    ];
}

/// What sort of thing a result is, which is what the row says on its right.
///
/// Worth showing because the palette mixes kinds deliberately: without it,
/// "Console" the panel and "console.decay" the file are two rows that look the
/// same and do very different things.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Panel,
    Arrangement,
    Entity,
    Asset,
    Scene,
    File,
    Edit,
    Run,
}

impl Kind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Panel => "Panel",
            Self::Arrangement => "Arrangement",
            Self::Entity => "Entity",
            Self::Asset => "Asset",
            Self::Scene => "Scene",
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Run => "Run",
        }
    }
}

/// One thing the palette is offering.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub label: String,
    /// What is shown under the label: a path, a parent, a shortcut.
    pub note: String,
    pub kind: Kind,
    pub action: Action,
}

impl Candidate {
    pub fn new(label: impl Into<String>, kind: Kind, action: Action) -> Self {
        Self {
            label: label.into(),
            note: String::new(),
            kind,
            action,
        }
    }

    #[must_use]
    pub fn noted(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// The text a query is matched against.
    ///
    /// Label and note together, so a file is found by its folder as well as by
    /// its name — typing `prefabs drift` should work, and it is the note that
    /// carries the folder.
    fn haystack(&self) -> String {
        if self.note.is_empty() {
            self.label.clone()
        } else {
            format!("{} {}", self.label, self.note)
        }
    }
}

/// What is typed, what was found, and which of it is chosen.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    open: bool,
    pub query: String,
    /// The index into the last set of results, clamped on read.
    chosen: usize,
    /// Set the frame the palette opens, so the field can take focus once.
    just_opened: bool,
}

impl Palette {
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the palette, always on an empty query.
    ///
    /// Keeping the last search would mean the first keystroke of the next one
    /// lands in the middle of it, and there is no use anyone has for reopening
    /// a search they have already acted on.
    pub fn open(&mut self) {
        self.open = true;
        self.just_opened = true;
        self.query.clear();
        self.chosen = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.chosen = 0;
    }

    /// Whether the text field should take focus this frame, consuming the flag.
    pub const fn take_focus(&mut self) -> bool {
        let first = self.just_opened;
        self.just_opened = false;
        first
    }

    /// Moves the selection, wrapping at both ends.
    ///
    /// Wrapping because the list is short and reaching the last entry by
    /// pressing up once is worth more than the purity of stopping at the top.
    pub fn step(&mut self, by: isize, results: usize) {
        if results == 0 {
            self.chosen = 0;
            return;
        }
        let count = isize::try_from(results).unwrap_or(isize::MAX);
        let at = isize::try_from(self.chosen.min(results - 1)).unwrap_or(0);
        self.chosen = usize::try_from((at + by).rem_euclid(count)).unwrap_or(0);
    }

    pub fn chosen(&self, results: usize) -> usize {
        self.chosen.min(results.saturating_sub(1))
    }

    pub fn choose(&mut self, index: usize) {
        self.chosen = index;
    }

    /// Ranks the candidates against what has been typed.
    ///
    /// Ties are broken by the order they were offered in, which is why the
    /// caller builds them in the order it does: panels and verbs before the
    /// project's files, so an empty query opens on the things there are few of
    /// rather than on the first ten assets in alphabetical order.
    pub fn rank(&self, candidates: Vec<Candidate>) -> Vec<Candidate> {
        let mut scored: Vec<(Score, usize, Candidate)> = candidates
            .into_iter()
            .enumerate()
            .filter_map(|(order, candidate)| {
                score_terms(&candidate.haystack(), &self.query)
                    .map(|score| (score, order, candidate))
            })
            .collect();
        scored
            .sort_by(|(one, first, _), (other, second, _)| other.cmp(one).then(first.cmp(second)));
        scored
            .into_iter()
            .take(VISIBLE)
            .map(|(_, _, candidate)| candidate)
            .collect()
    }
}

#[cfg(test)]
mod tests;
