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

use crate::ordering;
use crate::project::AssetKind;
use crate::selection;

pub(super) mod choosing;
pub(super) mod duplicate;

use duplicate::duplicate_into;

use super::hierarchy::row::entity_name;
use super::hierarchy::rows::{hierarchy_preference_key, hierarchy_rows};
use super::inspector_panel::draft::{ProjectDefaults, component_default};
use super::runtime::initialized_lifecycle;
use super::scene_io::load_world;
use super::{EditorApp, UI_IMAGE_COMPONENT};

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
pub(super) fn is_sliceable(path: &Path) -> bool {
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
        self.delete_entities(&[entity]);
    }

    /// Deletes everything selected.
    ///
    /// What the header's button and the Delete key mean, now that "the
    /// selection" can be five things.
    pub(super) fn delete_selection(&mut self) {
        let selected = self.selection.clone();
        self.delete_entities(selected.all());
    }

    /// Copies everything selected.
    pub(super) fn duplicate_selection(&mut self) {
        let selected = self.selection.clone();
        self.duplicate_entities(selected.all());
    }

    /// Deletes every entity named, in one transaction.
    ///
    /// One step rather than one per entity, because a selection of five pips
    /// deleted by mistake should come back with one Ctrl+Z. The set is folded
    /// first: a despawn already takes the subtree, so a parent and its child
    /// both named would despawn the child's handle twice and the second one
    /// would fail the whole transaction.
    pub(super) fn delete_entities(&mut self, entities: &[EntityId]) {
        let roots = selection::topmost(&self.world, entities);
        if roots.is_empty() {
            return;
        }
        let mut buffer = CommandBuffer::new();
        for entity in roots {
            buffer.push(WorldCommand::Despawn { entity });
        }
        self.history.break_merge_run();
        let label = if entities.len() == 1 {
            "Delete entity".to_owned()
        } else {
            format!("Delete {} entities", entities.len())
        };
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction(label), &mut self.world)
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
        self.duplicate_entities(&[entity]);
    }

    /// Copies every entity named, in one transaction, and selects the copies.
    ///
    /// Folded like a delete, for the same reason turned around: duplicating a
    /// parent already copies its child, so naming both would land two copies
    /// of the child. Each copy is rehearsed against the world the one before it
    /// left behind, so five copies earn five different stable IDs rather than
    /// five collisions.
    pub(super) fn duplicate_entities(&mut self, entities: &[EntityId]) {
        let roots = selection::topmost(&self.world, entities);
        let mut buffer = CommandBuffer::new();
        let mut copies = Vec::new();
        // One rehearsal for the lot, so each copy is told the handles and the
        // stable IDs the copies before it took.
        let mut rehearsal = self.world.clone();
        for entity in roots {
            copies.extend(duplicate_into(
                &mut rehearsal,
                &self.world,
                entity,
                &mut buffer,
            ));
        }
        if buffer.is_empty() {
            return;
        }
        self.history.break_merge_run();
        let label = if copies.len() == 1 {
            "Duplicate entity".to_owned()
        } else {
            format!("Duplicate {} entities", copies.len())
        };
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction(label), &mut self.world)
        {
            self.report(error.to_string());
            return;
        }
        self.select_many(copies);
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

    /// Moves an entity `offset` places among its siblings.
    ///
    /// Its own transaction, and the label counts what it actually changed:
    /// moving one row rewrites every sibling's recorded place, and an undo step
    /// called "Move 4 entities" would be describing bookkeeping rather than
    /// what was asked for.
    pub(super) fn move_among_siblings(&mut self, entity: EntityId, offset: isize) {
        let buffer = ordering::move_by(&self.world, entity, offset);
        if buffer.is_empty() {
            return;
        }
        self.history.break_merge_run();
        let label = if offset < 0 { "Move up" } else { "Move down" };
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction(label), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    /// Moves an entity under a new parent, or out to the root with `None`.
    pub(super) fn reparent(&mut self, entity: EntityId, parent: Option<EntityId>) {
        self.reparent_all(&[entity], parent);
    }

    /// Moves a dragged row, or the whole selection it belongs to.
    ///
    /// Dragging one of five banded rows moves the five, because that is what
    /// the band promised; dragging a row outside the selection moves that row
    /// alone, because a drag is not a way of selecting. The set is folded, so a
    /// parent and its child both selected do not fight over where the child
    /// goes, and a move the world refuses is reported and the rest still
    /// happen — the transaction is per entity, because "reparent five things"
    /// failing wholesale on the one illegal drop is worse than moving four.
    pub(super) fn reparent_dragged(&mut self, entity: EntityId, parent: Option<EntityId>) {
        if self.selection.contains(entity) && self.selection.len() > 1 {
            let moving = self.selection.clone();
            self.reparent_all(moving.all(), parent);
        } else {
            self.reparent_all(&[entity], parent);
        }
    }

    /// Moves every entity named under a new parent, or out to the root.
    ///
    /// Its own transaction rather than part of the inspector draft: a parent
    /// change is one discrete choice, and merging it into a transform drag
    /// would make one undo step that both moved and reparented.
    ///
    /// The move is offered only where [`World::check_set_parent`] allows it, so
    /// reaching the error here means the world changed under the open menu, or
    /// that one entity of several cannot go where the rest can. It is reported
    /// rather than ignored, because silently doing nothing is how an interface
    /// teaches people it is unreliable.
    fn reparent_all(&mut self, entities: &[EntityId], parent: Option<EntityId>) {
        let moving = selection::topmost(&self.world, entities);
        let mut buffer = CommandBuffer::new();
        let mut moved = 0_usize;
        for entity in moving {
            if Some(entity) == parent {
                continue;
            }
            buffer.push(WorldCommand::SetParent { entity, parent });
            moved += 1;
            // The place it held was a place among its old siblings, and it
            // means nothing among its new ones. Forgotten rather than
            // reinterpreted, so the entity lands at the bottom of the list it
            // was dropped into — which is where a thing you have just put
            // somewhere belongs, and where the eye is already looking.
            if self.world.get(entity).and_then(ordering::rank).is_some() {
                buffer.push(WorldCommand::SetEditorEntry {
                    entity,
                    key: ordering::ORDER_KEY.to_owned(),
                    value: None,
                });
            }
        }
        if buffer.is_empty() {
            return;
        }
        let label = if moved == 1 {
            "Reparent entity".to_owned()
        } else {
            format!("Reparent {moved} entities")
        };
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction(label), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    pub(super) fn undo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.undo(&mut self.world) {
            self.report(error.to_string());
        }
        // Undoing a Spawn despawns it, and a handle the world no longer holds
        // must not stay in the selection: the next verb would aim a command at
        // it, and the inspector would draw a panel about nothing.
        self.selection.retain_live(&self.world);
    }

    pub(super) fn redo(&mut self) {
        self.history.break_merge_run();
        if let Err(error) = self.history.redo(&mut self.world) {
            self.report(error.to_string());
        }
        self.selection.retain_live(&self.world);
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
                self.selection.clear();
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
