//! Which entities the hierarchy lists, in which order, at which depth.
//!
//! The rows are derived from the world every frame rather than kept alongside
//! it, so a command that changes parentage needs nothing here to agree with it.

use std::{collections::BTreeSet, path::Path};

use eframe::egui::{self, Response};
use egui_material_icons::MaterialIcon;
use sindri_core::{EntityId, World};

use crate::ordering::sibling_key as hierarchy_sort_key;
use crate::space::{EntitySpace, space_of};
use crate::ui::widgets::tree;

use super::row::entity_name;

/// One of the two roots the hierarchy hangs from.
///
/// A collapse chevron used to sit in front of it, and nothing collapsed.
pub(super) fn hierarchy_group(ui: &mut egui::Ui, label: &str, icon: MaterialIcon) -> Response {
    tree::group(ui, icon, label)
}

/// What the root is called wherever a parent is named.
///
/// One word for both groups, because there is one root: which of the two an
/// entity is listed under is decided by what it carries, not by where it was
/// dropped. "Top level" says that; "World" would promise that moving something
/// there makes it a world object, which no parent change can do.
pub(crate) const ROOT_LABEL: &str = "Top level";

/// Flattens the world into display rows, parents before their children.
///
/// Siblings are ordered by whatever place they record, and by stable ID where
/// they record none — see [`crate::ordering`]. A scene nobody has reordered
/// therefore lists exactly as it did when the ID was the only answer.
pub(crate) fn hierarchy_rows(world: &World) -> Vec<(EntityId, usize)> {
    let mut roots: Vec<EntityId> = world
        .entities()
        .filter(|(_, data)| data.parent.is_none())
        .map(|(entity, _)| entity)
        .collect();
    roots.sort_by_key(|entity| hierarchy_sort_key(world, *entity));

    let mut rows = Vec::new();
    for root in roots {
        push_hierarchy_row(world, root, 0, &mut rows);
    }
    rows
}

/// Rows currently visible after folding and filtering are applied.
///
/// Search deliberately ignores folded state and retains every ancestor of a
/// match. A result therefore still says where it lives instead of becoming a
/// misleading flat list, and clearing the search restores the user's folds.
pub(crate) fn visible_hierarchy_rows(
    world: &World,
    collapsed: &BTreeSet<EntityId>,
    needle: &str,
    space: EntitySpace,
) -> Vec<(EntityId, usize)> {
    let included = if needle.is_empty() {
        None
    } else {
        let mut included = BTreeSet::new();
        for (entity, data) in world.entities() {
            if !entity_name(data).to_lowercase().contains(needle) {
                continue;
            }
            let mut cursor = Some(entity);
            while let Some(current) = cursor {
                if !included.insert(current) {
                    break;
                }
                cursor = world.get(current).and_then(|data| data.parent);
            }
        }
        Some(included)
    };

    // Only the roots are sorted into a group. What hangs under one stays under
    // it, whatever it carries: the hierarchy an author built is theirs, and a
    // panel that scattered children by component would be rearranging it.
    let mut roots: Vec<EntityId> = world
        .entities()
        .filter(|(_, data)| data.parent.is_none())
        .map(|(entity, _)| entity)
        .filter(|entity| space_of(world, *entity) == space)
        .collect();
    roots.sort_by_key(|entity| hierarchy_sort_key(world, *entity));

    let mut rows = Vec::new();
    for root in roots {
        push_visible_hierarchy_row(world, root, 0, collapsed, included.as_ref(), &mut rows);
    }
    rows
}

fn push_visible_hierarchy_row(
    world: &World,
    entity: EntityId,
    depth: usize,
    collapsed: &BTreeSet<EntityId>,
    included: Option<&BTreeSet<EntityId>>,
    rows: &mut Vec<(EntityId, usize)>,
) {
    if included.is_some_and(|included| !included.contains(&entity)) {
        return;
    }
    rows.push((entity, depth));
    if included.is_none() && collapsed.contains(&entity) {
        return;
    }
    let Some(data) = world.get(entity) else {
        return;
    };
    let mut children = data.children.clone();
    children.sort_by_key(|child| hierarchy_sort_key(world, *child));
    for child in children {
        push_visible_hierarchy_row(world, child, depth + 1, collapsed, included, rows);
    }
}

fn push_hierarchy_row(
    world: &World,
    entity: EntityId,
    depth: usize,
    rows: &mut Vec<(EntityId, usize)>,
) {
    rows.push((entity, depth));
    let Some(data) = world.get(entity) else {
        return;
    };
    let mut children = data.children.clone();
    children.sort_by_key(|child| hierarchy_sort_key(world, *child));
    for child in children {
        push_hierarchy_row(world, child, depth + 1, rows);
    }
}

pub(crate) fn hierarchy_preference_key(
    path: Option<&Path>,
    world: &World,
    entity: EntityId,
) -> Option<String> {
    let path = path?;
    let source_id = world.get(entity)?.source_id.as_ref()?;
    Some(format!("{}::{}", path.display(), source_id.as_str()))
}
