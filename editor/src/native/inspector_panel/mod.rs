//! Editing what the selected entity holds.
//!
//! An inspector edit is never written to the world directly. A panel edits a
//! draft or a component payload, and what changed becomes checked commands on
//! the way out, so every edit undoes in one step and an edit the schema refuses
//! is refused rather than written.
//!
//! `header` is what every entity has, `section/` is the typed editor a
//! component type gets, `rows` is what a value falls back to when no section
//! claims it, and `draft` turns the whole of it into commands.

pub(super) mod draft;
pub(super) mod header;
pub(super) mod rows;
pub(super) mod section;

use std::{collections::BTreeMap, path::Path};

use eframe::egui::{self, Stroke};
use serde_json::Value;
use sindri_core::{CommandBuffer, ComponentMetadata, EntityId, SpriteRef, WorldCommand};

use self::draft::{
    EntityDraft, addable_components, component_commands, component_default, draft_commands,
};
use self::header::{ParentChoice, inspector_identity, inspector_parent, transform_3d_section};
use self::rows::add_component_button;
use self::section::components_sections;
use self::section::grid::grid_choices;
use crate::{animation::AnimationTool, scripts::SceneScripts, tilemap::TilemapTool};

use super::editing::reparent_choices;
use super::hierarchy::row::entity_icon;
use super::{BORDER, EditorApp, PANEL_BG, SPRITE_COMPONENT, UI_IMAGE_COMPONENT, panel_title};

/// Stateful authoring surfaces shared across component sections.
pub(super) struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// What the inspector reads about the project it is editing inside.
///
/// Grouped rather than passed one by one because every component section wants
/// some subset of it, and the list only grows: each new component that names a
/// project asset would otherwise add another parameter to one signature.
pub(super) struct InspectorProject<'a> {
    scripts: &'a SceneScripts,
    root: Option<&'a Path>,
    fonts: &'a [String],
    animation_texture: Option<&'a str>,
    grids: &'a [(String, String)],
}

impl EditorApp {
    /// Turns the difference between the drawn draft and the world into one
    /// transaction, so inspector edits are undoable and reach the viewport.
    pub(super) fn commit_draft(
        &mut self,
        entity: EntityId,
        original: &EntityDraft,
        draft: &EntityDraft,
    ) {
        let buffer = draft_commands(entity, original, draft);
        if buffer.is_empty() {
            return;
        }

        // One merge key per entity: a continuous drag stays a single undo step
        // until the pointer is released or the selection changes.
        let transaction = buffer
            .into_transaction("Edit entity")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// The components this entity does not have and the registry can create.
    ///
    /// A type with no default payload is missing from the list rather than
    /// offered and refused: a button that adds a component the engine will
    /// then reject is worse than no button, which is why the old Add Component
    /// was removed instead of left drawn.
    pub(super) fn addable_components(
        &self,
        present: &BTreeMap<String, Value>,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) -> Vec<ComponentMetadata> {
        addable_components(
            self.scene.components(),
            present,
            first_font,
            first_sprite,
            first_grid,
        )
    }

    /// Turns every changed component payload into a command.
    ///
    /// Each is checked against its own schema first. A payload is written back
    /// exactly as stored, so an edit that stopped it decoding would produce a
    /// scene the engine refuses to open — and the author would find out on the
    /// next launch rather than at the field they were editing.
    pub(super) fn commit_components(
        &mut self,
        entity: EntityId,
        original: &BTreeMap<String, Value>,
        draft: &BTreeMap<String, Value>,
    ) {
        let (buffer, refused) =
            component_commands(entity, original, draft, self.scene.components());
        for message in refused {
            self.console.warning(message);
        }
        if buffer.is_empty() {
            return;
        }
        // The same merge key the rest of the inspector uses, so dragging a tint
        // is one undo step rather than one per frame of the drag.
        let transaction = buffer
            .into_transaction("Edit components")
            .merging(format!("inspector:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }

    /// Adds a component with the payload its schema says a fresh one starts as.
    pub(super) fn add_component(
        &mut self,
        entity: EntityId,
        type_name: &str,
        first_font: Option<&str>,
        first_sprite: Option<&str>,
        first_grid: Option<&str>,
    ) {
        let Some(payload) = component_default(
            self.scene.components(),
            type_name,
            first_font,
            first_sprite,
            first_grid,
        ) else {
            return;
        };
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.to_owned(),
            payload,
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Add component"), &mut self.world)
        {
            self.report(error.to_string());
        }
        self.refresh_textures();
    }

    pub(super) fn remove_component(&mut self, entity: EntityId, type_name: &str) {
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::RemoveComponent {
            entity,
            type_name: type_name.to_owned(),
        });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Remove component"), &mut self.world)
        {
            self.report(error.to_string());
        }
        self.refresh_textures();
    }

    pub(super) fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(340.0)
            .min_size(300.0)
            .max_size(440.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Inspector");
                if self.slicer.is_some() {
                    self.slicer_panel(ui);
                    return;
                }
                let Some(entity) = self.selection else {
                    return;
                };
                let Some(data) = self.world.get(entity) else {
                    return;
                };
                // Widgets edit a draft copy; every difference becomes a command,
                // so the world is only ever written through the command layer.
                let mut draft = EntityDraft::from(data);
                let original = draft.clone();
                let icon = entity_icon(data);
                let original_components = data.components.clone();
                let mut components = original_components.clone();
                let parent = data.parent;
                let choices = reparent_choices(&self.world, entity);
                let mut reparented = ParentChoice::Unchanged;
                let mut removed = None;
                let mut added = None;
                let fonts = self.project.fonts();
                let first_font = fonts.first().map(String::as_str);
                // Either image family: an animated HUD element reads its
                // sheet from the UI image, exactly as a world sprite does.
                let animation_texture = components
                    .get(SPRITE_COMPONENT)
                    .or_else(|| components.get(UI_IMAGE_COMPONENT))
                    .and_then(|image| image.get("texture"))
                    .and_then(Value::as_str)
                    .and_then(|reference| SpriteRef::parse(reference).ok())
                    .map(|reference| reference.texture().to_owned());
                let animation_sprites = animation_texture
                    .as_deref()
                    .map(|texture| self.project.sprites_for_texture(texture))
                    .unwrap_or_default();
                let first_sprite = animation_sprites.first().map(String::as_str);
                let grids = grid_choices(&self.world);
                let first_grid = grids.first().map(|(_, id)| id.as_str());
                let addable =
                    self.addable_components(&components, first_font, first_sprite, first_grid);
                let project_root = self.project.root().map(Path::to_path_buf);
                {
                    let scripts = &self.scripts;
                    let mut tools = InspectorTools {
                        animation: &mut self.animation_tool,
                        tilemap: &mut self.tilemap_tool,
                    };
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, icon, &mut draft);
                        reparented = inspector_parent(ui, entity, parent, &choices);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        removed = components_sections(
                            ui,
                            &mut components,
                            &InspectorProject {
                                scripts,
                                root: project_root.as_deref(),
                                fonts: &fonts,
                                animation_texture: animation_texture.as_deref(),
                                grids: &grids,
                            },
                            &mut tools,
                        );
                        added = add_component_button(ui, &addable);
                    });
                }
                self.commit_draft(entity, &original, &draft);
                self.commit_components(entity, &original_components, &components);
                if let Some(type_name) = removed {
                    self.remove_component(entity, &type_name);
                }
                if let Some(type_name) = added {
                    self.add_component(entity, &type_name, first_font, first_sprite, first_grid);
                }
                match reparented {
                    ParentChoice::Unchanged => {}
                    ParentChoice::Root => self.reparent(entity, None),
                    ParentChoice::Under(parent) => self.reparent(entity, Some(parent)),
                }
            });
    }
}
