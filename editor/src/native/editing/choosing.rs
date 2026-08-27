//! What the editor is pointing at, and what changes when that moves.
//!
//! Separate from the verbs that write to the world, because choosing is not
//! editing: nothing here goes through the command layer or the undo history.
//! What it does instead is keep the rest of the editor honest about the move —
//! a half-typed field belongs to the entity it was being typed for, a tool
//! holding a brush belongs to the entity it was aimed at, and an inspector
//! showing a file is not also showing an entity.

use std::path::Path;

use sindri_core::EntityId;

use crate::audition;
use crate::preview::{self, TextPreview};
use crate::selection::Pick;
use crate::slicer::Slicer;
use crate::typeface;

use super::super::{EditorApp, Focus};
use super::is_sliceable;

impl EditorApp {
    /// Makes this the whole selection, or empties it with `None`.
    pub(crate) fn select(&mut self, entity: Option<EntityId>) {
        self.pick(entity, Pick::Only, &[]);
    }

    /// Makes exactly these entities the selection, the last one primary.
    ///
    /// What a verb that produces entities uses — duplicate, and anything else
    /// that leaves you pointing at what it just made.
    pub(crate) fn select_many(&mut self, entities: Vec<EntityId>) {
        self.select(None);
        self.selection = entities.into_iter().collect();
    }

    /// Changes the selection the way a click with these modifiers means to.
    ///
    /// `order` is the rows as the panel is drawing them, which is what a range
    /// runs along: a range in a tree means the rows between two rows, so a
    /// collapsed subtree is not in it and neither is anything the filter hid.
    pub(crate) fn pick(&mut self, entity: Option<EntityId>, how: Pick, order: &[EntityId]) {
        self.focus = Focus::Hierarchy;
        if entity.is_some() {
            // One inspector, one subject. Selecting an entity puts the file
            // away rather than leaving it behind a panel showing something
            // else.
            self.show_nothing();
        }
        let moved = match (entity, how) {
            (Some(entity), Pick::Also) => self.selection.toggle(entity),
            (Some(entity), Pick::Through) => self.selection.extend_through(entity, order),
            (entity, _) => self.selection.replace(entity),
        };
        if moved {
            self.history.break_merge_run();
            self.gizmo_drag = None;
            self.tilemap_tool.reset();
            self.animation_tool.reset();
            // A half-typed stable ID belongs to the entity it was being typed
            // for. Carried over, it would appear in the next entity's field
            // and be written to it on the way out.
            self.id_edit = None;
        }
    }

    /// Puts away whatever the inspector was showing about a file.
    pub(crate) fn show_nothing(&mut self) {
        self.slicer = None;
        self.preview = None;
        self.heard = None;
        // The font stays registered with egui until the panel stops showing
        // one, so a project font does not outlive the row that asked for it.
        self.shown_font = None;
    }

    /// Whether the inspector is already showing this exact file.
    ///
    /// Asked before anything is put away, so clicking the selected row again
    /// does not reload a file, restart a font, or reset a slicer someone is
    /// halfway through.
    fn already_showing(&self, path: &Path) -> bool {
        self.slicer.as_ref().is_some_and(|open| open.path() == path)
            || self
                .preview
                .as_ref()
                .is_some_and(|open| open.path() == path)
            || self.heard.as_deref() == Some(path)
            || self.shown_font.as_deref() == Some(path)
    }

    /// Marks an asset in the browser, and shows it if there is anything to
    /// show.
    ///
    /// The browser used to mark only the open scene, so selecting a file
    /// changed nothing visible and there was no such thing as "the asset I am
    /// pointing at" for anything else to act on. Now four kinds have something
    /// to show, and the rest are marked and nothing more.
    pub(crate) fn select_asset(&mut self, path: &Path) {
        self.focus = Focus::Project;
        self.browser.selected = Some(path.to_owned());
        if !path.is_file() {
            return;
        }
        // Four things the inspector can show about a file: an image slices, a
        // text file is read, a clip plays and a font draws. All four take the
        // panel over, so choosing one puts the others away rather than leaving
        // it showing the file before last.
        if self.already_showing(path) {
            return;
        }
        self.show_nothing();
        if is_sliceable(path) {
            self.slicer = Some(Slicer::open(path));
        } else if preview::is_readable(path) {
            self.preview = Some(TextPreview::open(path));
        } else if audition::is_audible(path) {
            self.heard = Some(path.to_owned());
        } else if typeface::is_a_typeface(path) {
            self.shown_font = Some(path.to_owned());
        } else {
            return;
        }
        self.selection.clear();
        self.tilemap_tool.reset();
        self.animation_tool.reset();
    }
}
