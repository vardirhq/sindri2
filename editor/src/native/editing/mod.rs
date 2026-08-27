//! Selecting entities and the checked commands that change them.
//!
//! Every mutation here goes through `CommandBuffer` and the history, never a
//! direct world write, so undo is a property of the shell rather than
//! something each caller remembers to arrange.

use std::path::Path;

use sindri_core::{
    CommandBuffer, EntityData, EntityId, SceneEntityId, Transform3D, World, WorldCommand,
};
use sindri_scene::SpriteAnimations;

use crate::preview::{self, TextPreview};
use crate::project::AssetKind;
use crate::slicer::Slicer;

pub(super) mod duplicate;

use duplicate::duplicate_commands;

use super::hierarchy::row::entity_name;
use super::hierarchy::rows::{hierarchy_preference_key, hierarchy_rows};
use super::inspector_panel::draft::{ProjectDefaults, component_default};
use super::runtime::initialized_lifecycle;
use super::scene_io::load_world;
use super::{EditorApp, Focus, UI_IMAGE_COMPONENT};

/// What the hierarchy's create menu was asked for.
///
/// A UI element is its own entry rather than "make an empty and then find the
/// right component", because which space a thing is in is the first thing an
/// author knows about it and the last thing they should have to discover.
#[derive(Clone, Copy)]
pub(super) enum CreateGameObject {
    Empty { parent: Option<EntityId> },
    UiImage,
}

/// The parents `entity` may legally be moved under, in the order the hierarchy
/// lists them.
///
/// Legality is asked of the world rather than decided here, so the menu cannot
/// offer a move the command layer would then refuse. The root is not in this
/// list because it is not an entity; it is the separate "World" choice.
pub(super) fn reparent_choices(world: &World, entity: EntityId) -> Vec<(EntityId, String)> {
    hierarchy_rows(world)
        .into_iter()
        .filter(|(candidate, _)| world.check_set_parent(entity, Some(*candidate)).is_ok())
        .filter_map(|(candidate, _)| {
            world
                .get(candidate)
                .map(|data| (candidate, entity_name(data)))
        })
        .collect()
}

/// Whether the editor opens the slicer for this file.
fn is_sliceable(path: &Path) -> bool {
    AssetKind::of_path(path) == AssetKind::Texture
}

/// A stable ID is assigned before the spawn enters history so save, undo, and
/// redo all agree on the identity of a newly authored `GameObject`.
pub(super) fn next_game_object_id(world: &World) -> SceneEntityId {
    let mut suffix = 1_u32;
    loop {
        let candidate = SceneEntityId::new(format!("game-object-{suffix}"))
            .expect("the generated GameObject ID is valid");
        if world
            .entities()
            .all(|(_, data)| data.source_id.as_ref() != Some(&candidate))
        {
            return candidate;
        }
        suffix += 1;
    }
}

/// Only tests look entities up by their authored ID; the editor works in
/// runtime handles.
#[cfg(test)]
pub(super) fn find_by_source_id(world: &World, source_id: &str) -> Option<EntityId> {
    world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == source_id)
        })
        .map(|(entity, _)| entity)
}

impl EditorApp {
    /// What to call an entity in a console line.
    pub(super) fn entity_label(&self, entity: EntityId) -> String {
        self.world
            .get(entity)
            .and_then(|data| {
                data.name
                    .clone()
                    .or_else(|| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
            })
            .unwrap_or_else(|| format!("{entity:?}"))
    }

    pub(super) fn select(&mut self, entity: Option<EntityId>) {
        self.focus = Focus::Hierarchy;
        if entity.is_some() {
            // One inspector, one subject. Selecting an entity puts the file
            // away rather than leaving it behind a panel showing something
            // else.
            self.slicer = None;
            self.preview = None;
        }
        if self.selection != entity {
            self.history.break_merge_run();
            self.gizmo_drag = None;
            self.tilemap_tool.reset();
            self.animation_tool.reset();
            // A half-typed stable ID belongs to the entity it was being typed
            // for. Carried over, it would appear in the next entity's field
            // and be written to it on the way out.
            self.id_edit = None;
            self.selection = entity;
        }
    }

    /// Marks an asset in the browser, and shows it if there is anything to
    /// show.
    ///
    /// Marking is the whole of it for most kinds. The browser used to mark only
    /// the open scene, so selecting a file changed nothing visible and there
    /// was no such thing as "the asset I am pointing at" for anything else to
    /// act on. A texture also opens the slicer, which is the one asset the
    /// editor can currently do something with.
    pub(super) fn select_asset(&mut self, path: &Path) {
        self.focus = Focus::Project;
        self.browser.selected = Some(path.to_owned());
        if !path.is_file() {
            return;
        }
        // An image slices and a text file is read. Both take the inspector
        // over, so choosing one puts the other away rather than leaving a
        // panel showing the file before last.
        if is_sliceable(path) {
            self.preview = None;
            if self.slicer.as_ref().is_some_and(|open| open.path() == path) {
                return;
            }
            self.slicer = Some(Slicer::open(path));
        } else if preview::is_readable(path) {
            self.slicer = None;
            if self
                .preview
                .as_ref()
                .is_some_and(|open| open.path() == path)
            {
                return;
            }
            self.preview = Some(TextPreview::open(path));
        } else {
            return;
        }
        self.selection = None;
        self.tilemap_tool.reset();
        self.animation_tool.reset();
    }

    /// Creates what the menu asked for, and selects it.
    pub(super) fn create_game_object(&mut self, create: CreateGameObject) {
        match create {
            CreateGameObject::Empty { parent } => self.create_entity(parent),
            CreateGameObject::UiImage => self.create_ui_image(),
        }
    }

    /// Creates a UI element at the middle of the viewport.
    ///
    /// It arrives carrying `sindri.ui.image`, which is what puts it in the UI
    /// group and what makes it visible: an empty entity called "UI something"
    /// would be in neither space and would draw nothing.
    fn create_ui_image(&mut self) {
        // A UI image has an honest blank in the registry, so nothing from the
        // project is needed to complete it.
        let Some(payload) = component_default(
            self.scene.components(),
            UI_IMAGE_COMPONENT,
            ProjectDefaults::default(),
        ) else {
            return;
        };
        let entity = self.world.next_handle();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Spawn {
            entity,
            data: Box::new(EntityData {
                source_id: Some(next_game_object_id(&self.world)),
                name: Some("UI Image".to_owned()),
                transform_3d: Some(Transform3D::default()),
                components: [(UI_IMAGE_COMPONENT.to_owned(), payload)]
                    .into_iter()
                    .collect(),
                ..EntityData::default()
            }),
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Create UI Image"), &mut self.world)
        {
            self.report(error.to_string());
            return;
        }
        self.select(Some(entity));
        self.refresh_textures();
    }

    /// Creates an empty `GameObject`, optionally under another, and selects it.
    ///
    /// The handle is taken from the world *before* the command runs, so the
    /// command can be redone onto the same handle, and so there is something to
    /// select without asking the world what just appeared.
    pub(super) fn create_entity(&mut self, parent: Option<EntityId>) {
        let entity = self.world.next_handle();
        let source_id = next_game_object_id(&self.world);
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Spawn {
            entity,
            data: Box::new(EntityData {
                source_id: Some(source_id),
                name: Some("GameObject".to_owned()),
                parent,
                transform_3d: Some(Transform3D::default()),
                ..EntityData::default()
            }),
        });
        self.history.break_merge_run();
        if let Err(error) = self.history.apply(
            buffer.into_transaction(if parent.is_some() {
                "Create child"
            } else {
                "Create GameObject"
            }),
            &mut self.world,
        ) {
            self.report(error.to_string());
            return;
        }
        // Selected, because making something and then having to find it is the
        // kind of small friction that makes a tool tiring to use.
        self.select(Some(entity));
        if let Some(parent) = parent
            && let Some(key) = hierarchy_preference_key(self.file.path(), &self.world, parent)
        {
            self.preferences.collapsed_hierarchy.remove(&key);
        }
    }

    /// Deletes an entity and everything under it.
    ///
    /// The selection is cleared rather than moved to the parent: after a
    /// delete, nothing is what is selected, and guessing otherwise risks an
    /// edit meant for the deleted thing landing somewhere else.
    ///
    /// Undo brings it back at the same handle, so this is not the one-way door
    /// a delete usually is — see [`sindri_core::World::spawn_at`].
    pub(super) fn delete_entity(&mut self, entity: EntityId) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::Despawn { entity });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Delete entity"), &mut self.world)
        {
            self.report(error.to_string());
            return;
        }
        self.select(None);
        self.refresh_textures();
    }

    /// Copies an entity and everything under it, beside it, and selects the
    /// copy.
    ///
    /// One transaction, so a duplicated subtree undoes in one step rather than
    /// leaving half a copy behind. The copy keeps the original's parent, so it
    /// appears as a sibling — which is what "duplicate" means everywhere else,
    /// and is what makes building five pips from one bearable.
    pub(super) fn duplicate_entity(&mut self, entity: EntityId) {
        let (buffer, root) = duplicate_commands(&self.world, entity);
        if buffer.is_empty() {
            return;
        }
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Duplicate entity"), &mut self.world)
        {
            self.report(error.to_string());
            return;
        }
        self.select(root);
        self.refresh_textures();
    }

    /// Renames an entity, through the same command path the inspector uses.
    ///
    /// A blank name is a request to have no name rather than to be called
    /// nothing: the hierarchy falls back to the stable ID, which is what an
    /// entity that never had a name shows.
    pub(super) fn rename_entity(&mut self, entity: EntityId, name: &str) {
        let name = name.trim();
        let current = self.world.get(entity).and_then(|data| data.name.clone());
        let wanted = (!name.is_empty()).then(|| name.to_owned());
        if current == wanted {
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetName {
            entity,
            name: wanted,
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Rename entity"), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    /// Moves an entity under a new parent, or out to the root with `None`.
    ///
    /// Its own transaction rather than part of the inspector draft: a parent
    /// change is one discrete choice, and merging it into a transform drag
    /// would make one undo step that both moved and reparented.
    ///
    /// The move is offered only where [`World::check_set_parent`] allows it, so
    /// reaching the error here means the world changed under the open menu. It
    /// is reported rather than ignored, because silently doing nothing is how
    /// an interface teaches people it is unreliable.
    pub(super) fn reparent(&mut self, entity: EntityId, parent: Option<EntityId>) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetParent { entity, parent });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Reparent entity"), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    pub(super) fn undo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.undo(&mut self.world) {
            self.report(error.to_string());
        }
    }

    pub(super) fn redo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.redo(&mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Rebuilds the runtime scene from the authored document.
    ///
    /// Every runtime handle is replaced, so recorded history is discarded
    /// rather than left pointing at entities that no longer exist.
    pub(super) fn reset_to_authored(&mut self) {
        match load_world(&self.scene, self.file.document()) {
            Ok(world) => {
                self.world = world;
                self.history.clear();
                self.saved_revision = self.history.revision();
                self.selection = None;
                self.tilemap_tool.reset();
                self.animation_tool.reset();
                self.lifecycle = initialized_lifecycle();
                // A cursor belongs to the world it was advanced against, and
                // a freshly loaded world reuses entity slots from the start.
                self.animations = SpriteAnimations::new();
                self.play_snapshot = None;
                self.notice = None;
                self.announce_scene();
                self.reload_textures();
                self.reload_scripts();
            }
            Err(error) => self.report(error),
        }
    }
}
