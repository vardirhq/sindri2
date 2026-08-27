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
pub(super) mod field;
pub(super) mod header;
pub(super) mod rows;
mod scene;
pub(super) mod section;

use std::{collections::BTreeMap, path::Path};

use eframe::egui;
use serde_json::Value;
use sindri_core::{CommandBuffer, ComponentSchemaRegistry, EntityId, SpriteRef, WorldCommand};

use self::draft::{
    EntityDraft, Offer, ProjectDefaults, SceneHolds, addable_components, component_commands,
    component_default, draft_commands, identity_commands,
};
use self::field::FieldAssets;
use self::header::{
    Identity, IdentityEdit, ParentChoice, inspector_identity, inspector_parent,
    transform_3d_section,
};
use self::rows::add_component_button;
use self::scene::{SceneSummary, scene_section};
use self::section::components_sections;
use self::section::grid::grid_choices;
use sindri_scene::PROCEDURAL_TEXTURES;

use crate::project::ProjectTree;
use crate::ui::icons;
use crate::ui::theme::color;
use crate::ui::widgets::{panel, toolbar};
use crate::{
    animation::AnimationTool, scripts::SceneScripts, space::declared_space, tilemap::TilemapTool,
};

use super::editing::reparent_choices;
use super::hierarchy::row::entity_icon;
use super::{CAMERA_COMPONENT, EditorApp, SPRITE_COMPONENT, UI_IMAGE_COMPONENT};

/// Every texture reference the engine can actually draw.
///
/// The project's own files, plus the handful the engine generates. A procedural
/// reference is deliberately not parseable as an asset path, so a picker built
/// from the directory alone both refused to offer `procedural:checkerboard` and
/// marked the fixture's own cube as naming a texture that does not exist.
fn drawable_textures(project: &ProjectTree) -> Vec<String> {
    let mut textures: Vec<String> = PROCEDURAL_TEXTURES
        .iter()
        .map(|texture| texture.reference.to_owned())
        .collect();
    textures.extend(project.textures());
    textures
}

/// Stateful authoring surfaces shared across component sections.
pub(super) struct InspectorTools<'a> {
    animation: &'a mut AnimationTool,
    tilemap: &'a mut TilemapTool,
}

/// Everything the panel reads off the project and the registry before it draws
/// anything.
///
/// Gathered in one place because it is all read from `self`, and the panel then
/// borrows `self` mutably to draw: without this the two would fight, and the
/// alternative — reading each list inline — is what made one method long enough
/// that nobody would read it either.
struct PanelContext {
    fonts: Vec<String>,
    textures: Vec<String>,
    scripts: Vec<String>,
    audio: Vec<String>,
    /// The first `.decay` source the project holds that declares a script, and
    /// the first script it declares.
    ///
    /// Both, because a script component naming a source and no container is one
    /// that loads and runs nothing. A source still compiling declares nothing
    /// yet, so Script becomes addable a moment after the project opens rather
    /// than immediately — which is the honest order.
    script_container: Option<(String, String)>,
    /// The sheet an animation on this entity reads, from whichever image
    /// component the entity carries.
    animation_texture: Option<String>,
    animation_sprites: Vec<String>,
    grids: Vec<(String, String)>,
    root: Option<std::path::PathBuf>,
    registry: ComponentSchemaRegistry,
}

impl PanelContext {
    /// What the project can complete a component the engine cannot invent with.
    fn defaults(&self) -> ProjectDefaults<'_> {
        ProjectDefaults {
            font: self.fonts.first().map(String::as_str),
            sprite: self.animation_sprites.first().map(String::as_str),
            grid: self.grids.first().map(|(_, id)| id.as_str()),
            audio: self.audio.first().map(String::as_str),
            script: self
                .script_container
                .as_ref()
                .map(|(source, script)| (source.as_str(), script.as_str())),
        }
    }

    fn assets(&self) -> FieldAssets<'_> {
        FieldAssets {
            textures: &self.textures,
            fonts: &self.fonts,
            scripts: &self.scripts,
            audio: &self.audio,
        }
    }
}

/// What the inspector reads about the project it is editing inside.
///
/// Grouped rather than passed one by one because every component section wants
/// some subset of it, and the list only grows: each new component that names a
/// project asset would otherwise add another parameter to one signature.
pub(super) struct InspectorProject<'a> {
    scripts: &'a SceneScripts,
    root: Option<&'a Path>,
    /// What the project holds, for the fields that name one of its files.
    assets: FieldAssets<'a>,
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

    /// Every component this entity could carry, and whether it can carry it
    /// yet — each one that cannot saying why.
    pub(super) fn addable_components(
        &self,
        present: &BTreeMap<String, Value>,
        project: ProjectDefaults<'_>,
    ) -> Vec<Offer> {
        addable_components(
            self.scene.components(),
            present,
            project,
            self.scene_holds(),
        )
    }

    /// What the rest of the scene already holds that a component here would
    /// have to be the only one of.
    fn scene_holds(&self) -> SceneHolds {
        SceneHolds {
            world_camera: self
                .world
                .entities()
                .any(|(_, data)| data.components.contains_key(CAMERA_COMPONENT)),
        }
    }

    /// The scene itself, shown where an entity's inspector would be.
    ///
    /// The name is held across frames and written once the edit is finished,
    /// for the reason the stable ID beside an entity is: a name committed every
    /// keystroke is one undo step per letter.
    fn scene_section(&mut self, ui: &mut egui::Ui) {
        let mut name = self
            .scene_name_edit
            .clone()
            .unwrap_or_else(|| self.world.metadata().name.clone().unwrap_or_default());
        let file = self.file.path().map(|path| path.display().to_string());
        let finished = scene_section(
            ui,
            SceneSummary {
                name: &mut name,
                file: file.as_deref(),
                entities: self.world.len(),
            },
        );
        if finished {
            self.scene_name_edit = None;
            self.rename_scene(&name);
        } else {
            self.scene_name_edit = Some(name);
        }
    }

    /// Renames the scene, through the history like every other edit.
    ///
    /// A blank name is a request to have none rather than to be called nothing,
    /// which is what a scene that was never named already has.
    fn rename_scene(&mut self, name: &str) {
        let name = name.trim();
        let wanted = (!name.is_empty()).then(|| name.to_owned());
        if self.world.metadata().name == wanted {
            return;
        }
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetSceneName { name: wanted });
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Rename scene"), &mut self.world)
        {
            self.report(error.to_string());
        }
    }

    /// Holds the ID being typed, and writes it once the edit is finished.
    fn settle_identity(&mut self, entity: EntityId, text: String, edit: IdentityEdit) {
        if edit.finished {
            self.id_edit = None;
            self.commit_identity(entity, &text);
        } else if edit.changed {
            self.id_edit = Some((entity, text));
        }
    }

    /// Writes a changed stable ID, and whatever pointed at the old one.
    ///
    /// Its own transaction rather than part of the entity draft: a rename here
    /// rewrites other entities' components, which is a discrete act and not
    /// something to merge into a transform drag. A value the scene cannot use
    /// produces no commands at all — the field says why instead.
    fn commit_identity(&mut self, entity: EntityId, wanted: &str) {
        let Ok(buffer) = identity_commands(&self.world, entity, wanted) else {
            return;
        };
        if buffer.is_empty() {
            return;
        }
        self.history.break_merge_run();
        if let Err(error) = self
            .history
            .apply(buffer.into_transaction("Change stable ID"), &mut self.world)
        {
            self.report(error.to_string());
        }
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
        project: ProjectDefaults<'_>,
    ) {
        let Some(payload) = component_default(self.scene.components(), type_name, project) else {
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

    fn panel_context(&self, components: &BTreeMap<String, Value>) -> PanelContext {
        // Either image family: an animated HUD element reads its sheet from the
        // UI image, exactly as a world sprite does.
        let animation_texture = components
            .get(SPRITE_COMPONENT)
            .or_else(|| components.get(UI_IMAGE_COMPONENT))
            .and_then(|image| image.get("texture"))
            .and_then(Value::as_str)
            .and_then(|reference| SpriteRef::parse(reference).ok())
            .map(|reference| reference.texture().to_owned());
        let scripts = self.project.scripts();
        PanelContext {
            script_container: scripts.iter().find_map(|source| {
                self.scripts
                    .declared(source)
                    .into_iter()
                    .next()
                    .map(|script| (source.clone(), script))
            }),
            fonts: self.project.fonts(),
            textures: drawable_textures(&self.project),
            scripts,
            audio: self.project.audio(),
            animation_sprites: animation_texture
                .as_deref()
                .map(|texture| self.project.sprites_for_texture(texture))
                .unwrap_or_default(),
            animation_texture,
            grids: grid_choices(&self.world),
            root: self.project.root().map(Path::to_path_buf),
            registry: self.scene.components().clone(),
        }
    }

    /// The panel: its frame, its header, and whichever of its three states it
    /// is in — slicing an image, editing an entity, or empty.
    pub(super) fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(340.0)
            .min_size(300.0)
            .max_size(440.0)
            .resizable(true)
            .frame(panel::frame())
            .show(ui, |ui| {
                let slicing = self.slicer.is_some();
                panel::header(ui, icons::INSPECTOR, "Inspector", |ui| {
                    if slicing {
                        toolbar::chip(ui, "Slicing", color::FORGE);
                    }
                });
                if slicing {
                    self.slicer_panel(ui);
                    return;
                }
                // An empty inspector used to be a blank rectangle, which is
                // indistinguishable from a panel that has stopped working.
                // With nothing in focus, the thing in focus is the scene. The
                // panel used to spend this space on a shrug, and the scene's
                // own name — a real field that round-trips through a save —
                // was shown nowhere at all.
                let Some(entity) = self.selection else {
                    self.scene_section(ui);
                    return;
                };
                if self.world.get(entity).is_none() {
                    panel::empty_state(
                        ui,
                        icons::INSPECTOR,
                        "That entity is gone",
                        "It was removed from the scene while it was selected.",
                    );
                    return;
                }
                self.inspect_entity(ui, entity);
            });
    }

    /// Everything one entity has, drawn from a draft and committed as commands.
    ///
    /// Drawn disabled while the scene is playing. Every control here becomes a
    /// command against the world, and Stop restores the world as it was when
    /// Play was pressed — so an edit made now would be discarded silently and
    /// leave the history describing a change that is no longer there.
    fn inspect_entity(&mut self, ui: &mut egui::Ui, entity: EntityId) {
        let Some(data) = self.world.get(entity) else {
            return;
        };
        // Widgets edit a draft copy; every difference becomes a command,
        // so the world is only ever written through the command layer.
        let mut draft = EntityDraft::from(data);
        let original = draft.clone();
        let icon = entity_icon(data);
        let space = declared_space(&data.components);
        let original_components = data.components.clone();
        let mut components = original_components.clone();
        let parent = data.parent;
        let choices = reparent_choices(&self.world, entity);
        let mut reparented = ParentChoice::Unchanged;
        let mut removed = None;
        let mut added = None;
        let authoring = self.authoring_enabled();
        let context = self.panel_context(&components);
        let addable = self.addable_components(&components, context.defaults());
        // The text the ID field is showing: whatever is being typed if this
        // entity's ID is mid-edit, and what the world holds otherwise.
        let mut identity = match &self.id_edit {
            Some((held, text)) if *held == entity => text.clone(),
            _ => draft.source_id.clone(),
        };
        // Asked before the panel draws, because the field has to show a
        // refusal rather than the console reporting one every frame.
        let identity_refused = identity_commands(&self.world, entity, &identity).err();
        let mut identity_edit = IdentityEdit::default();
        {
            let scripts = &self.scripts;
            let mut tools = InspectorTools {
                animation: &mut self.animation_tool,
                tilemap: &mut self.tilemap_tool,
            };
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_enabled_ui(authoring, |ui| {
                        identity_edit = inspector_identity(
                            ui,
                            icon,
                            space,
                            Identity {
                                text: &mut identity,
                                refused: identity_refused,
                            },
                            &mut draft,
                        );
                        reparented = inspector_parent(ui, entity, parent, &choices);
                        if let Some(transform) = &mut draft.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        removed = components_sections(
                            ui,
                            &mut components,
                            &context.registry,
                            &InspectorProject {
                                scripts,
                                root: context.root.as_deref(),
                                assets: context.assets(),
                                animation_texture: context.animation_texture.as_deref(),
                                grids: &context.grids,
                            },
                            &mut tools,
                        );
                        added = add_component_button(ui, &addable);
                    });
                });
        }
        self.commit_draft(entity, &original, &draft);
        self.settle_identity(entity, identity, identity_edit);
        self.commit_components(entity, &original_components, &components);
        if let Some(type_name) = removed {
            self.remove_component(entity, &type_name);
        }
        if let Some(type_name) = added {
            self.add_component(entity, &type_name, context.defaults());
        }
        match reparented {
            ParentChoice::Unchanged => {}
            ParentChoice::Root => self.reparent(entity, None),
            ParentChoice::Under(parent) => self.reparent(entity, Some(parent)),
        }
    }
}
