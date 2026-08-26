//! One section per component type, and the dispatch that picks it.
//!
//! A component gets a typed editor here rather than a raw JSON field
//! wherever a text box could turn a scene into one that will not load.
//! Adding a component type means a file beside these and one arm in
//! `components_sections`.

pub(super) mod animation;
pub(super) mod grid;
pub(super) mod script;
pub(super) mod text;
pub(super) mod tilemap;

use std::collections::BTreeMap;

use eframe::egui;
use serde_json::Value;
use sindri_core::ComponentSchemaRegistry;

use self::animation::animation_section;
use self::grid::{grid_navigation_section, grid_occupant_section};
use self::script::{script_choice_row, script_exports_section};
use self::text::text_section;
use self::tilemap::tilemap_section;
use crate::inspector;
use crate::ui::icons;
use crate::ui::widgets::{
    button::{self, Intent},
    section,
};

use super::super::hierarchy::row::component_label;
use super::super::{GRID_NAVIGATION_COMPONENT, GRID_OCCUPANT_COMPONENT, UI_TEXT_COMPONENT};
use super::field::object_rows;
use super::{InspectorProject, InspectorTools};

/// Draws every component on an entity, editable, and reports what changed.
///
/// The payload is edited in place on a draft; the caller diffs it and turns
/// each difference into a `SetComponent`. Nothing here writes to the world.
pub(super) fn components_sections(
    ui: &mut egui::Ui,
    components: &mut BTreeMap<String, Value>,
    registry: &ComponentSchemaRegistry,
    project: &InspectorProject<'_>,
    tools: &mut InspectorTools<'_>,
) -> Option<String> {
    let InspectorProject {
        scripts,
        root: project_root,
        assets,
        animation_texture,
        grids,
    } = *project;
    let grid_size = components
        .get(crate::tilemap::TYPE_NAME)
        .and_then(|payload| crate::tilemap::component(payload).ok())
        .map(|map| (map.columns, map.rows));
    let mut removed = None;
    for (name, payload) in components.iter_mut() {
        let title = component_label(name);
        // The same icons the hierarchy gives these entities, so a row and its
        // component are recognisably the same thing.
        let open = section::component(
            ui,
            egui::Id::new(("inspector-component", name.as_str())),
            icons::for_component(name),
            &title,
            |ui| {
                if inspector::is_removable(name)
                    && button::row_icon(
                        ui,
                        icons::REMOVE,
                        Intent::Danger,
                        &format!("Remove {title}"),
                    )
                    .clicked()
                {
                    removed = Some(name.clone());
                }
            },
        );
        if !open {
            continue;
        }
        ui.add_space(4.0);

        // A script's @export fields come first and are drawn from what the
        // script declared, which is the whole reason the language is typed.
        // The rest of the payload -- the source, the container -- follows as
        // ordinary rows.
        if name == "sindri.script" {
            script_choice_row(ui, payload, scripts);
            script_exports_section(ui, payload, scripts);
        }
        if name == UI_TEXT_COMPONENT {
            text_section(ui, payload, assets.fonts);
        }
        if name == crate::animation::TYPE_NAME {
            animation_section(
                ui,
                payload,
                project_root,
                animation_texture,
                tools.animation,
            );
        }
        if name == crate::tilemap::TYPE_NAME {
            tilemap_section(ui, payload, project_root, tools.tilemap);
        }
        if name == GRID_NAVIGATION_COMPONENT {
            grid_navigation_section(ui, payload, grid_size);
        }
        if name == GRID_OCCUPANT_COMPONENT {
            grid_occupant_section(ui, payload, grids);
        }
        // The registry's blank is what says which fields this component has,
        // so an instance that wrote none of them still shows all of them.
        object_rows(
            ui,
            name,
            payload,
            registry.default_payload(name),
            assets,
            name == "sindri.script",
        );
        ui.add_space(5.0);
    }
    removed
}
