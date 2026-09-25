//! The scene as the element tree Weave's selectors walk.

use std::collections::BTreeMap;

use sindri_core::{EntityId, World};
use weave::{Element, Position, States, Tree};

use crate::{UiStates, hierarchy_depth};

/// One entity as a selector sees it.
pub(crate) struct Node {
    pub(crate) entity: EntityId,
    pub(crate) parent: Option<usize>,
    pub(crate) id: String,
    classes: Vec<String>,
    names: Vec<String>,
    states: States,
    /// Where it sits among its siblings, in the scene's order.
    position: Position,
    /// The node just before it among its siblings.
    previous: Option<usize>,
}

/// The scene's entities as the element tree selectors walk.
pub(crate) struct Elements {
    pub(crate) nodes: Vec<Node>,
}

impl Tree for Elements {
    fn element(&self, node: usize) -> Element<'_> {
        let node = &self.nodes[node];
        Element {
            id: &node.id,
            classes: &node.classes,
            names: &node.names,
            states: node.states,
        }
    }

    fn parent(&self, node: usize) -> Option<usize> {
        self.nodes[node].parent
    }

    fn position(&self, node: usize) -> Option<Position> {
        Some(self.nodes[node].position)
    }

    fn previous_sibling(&self, node: usize) -> Option<usize> {
        self.nodes[node].previous
    }
}

/// The names an element answers to: its components' full type names, and
/// the short ones a stylesheet would rather write, `text` for
/// `sindri.ui.text` and `sprite` for `sindri.sprite`.
fn element_names(component_types: impl Iterator<Item = String>) -> Vec<String> {
    let mut names = Vec::new();
    for full in component_types {
        let short = full
            .strip_prefix("sindri.ui.")
            .or_else(|| full.strip_prefix("sindri."))
            .filter(|short| !short.contains('.'))
            .map(str::to_owned);
        names.push(full);
        names.extend(short);
    }
    names
}

/// The states an element's own data puts it in: a component that says it is
/// disabled, or checked.
fn component_states(data: &sindri_core::EntityData) -> States {
    let flag = |field: &str| {
        data.components
            .values()
            .any(|payload| payload.get(field).and_then(serde_json::Value::as_bool) == Some(true))
    };
    let mut states = States::NONE;
    if flag("disabled") {
        states = states.with(States::DISABLED);
    }
    if flag("checked") {
        states = states.with(States::CHECKED);
    }
    states
}

pub(crate) fn elements(world: &World, states: &UiStates) -> Elements {
    let mut ordered: Vec<EntityId> = world.entities().map(|(entity, _)| entity).collect();
    // Parents before children: a child inherits its parent's computed style,
    // and percent sizes resolve against a parent box already settled.
    ordered.sort_by_key(|entity| hierarchy_depth(world, *entity));
    let index: BTreeMap<EntityId, usize> = ordered
        .iter()
        .enumerate()
        .map(|(position, entity)| (*entity, position))
        .collect();
    // Siblings in the order the scene gives them, which is not the order
    // above: a parent's children list, and the scene's own order for roots.
    // One pass over each line, so a large scene is not scanned per entity.
    let roots: Vec<EntityId> = world
        .entities()
        .filter(|(_, data)| data.parent.is_none())
        .map(|(entity, _)| entity)
        .collect();
    let mut placed: BTreeMap<EntityId, (Position, Option<usize>)> = BTreeMap::new();
    let lines = std::iter::once(roots.as_slice())
        .chain(world.entities().map(|(_, data)| data.children.as_slice()));
    for line in lines {
        for (at, entity) in line.iter().enumerate() {
            let previous = at
                .checked_sub(1)
                .and_then(|before| line.get(before))
                .and_then(|sibling| index.get(sibling).copied());
            let position = Position {
                index: at,
                count: line.len(),
            };
            placed.insert(*entity, (position, previous));
        }
    }
    let alone = (Position { index: 0, count: 1 }, None);
    let nodes = ordered
        .iter()
        .filter_map(|entity| {
            let data = world.get(*entity)?;
            let (position, previous) = placed.get(entity).copied().unwrap_or(alone);
            let classes = data
                .components
                .get("weave.style")
                .and_then(|payload| payload.get("classes"))
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect();
            Some(Node {
                entity: *entity,
                parent: data.parent.and_then(|parent| index.get(&parent).copied()),
                id: data
                    .source_id
                    .as_ref()
                    .map(|id| id.as_str().to_owned())
                    .unwrap_or_default(),
                classes,
                names: element_names(data.components.keys().cloned()),
                states: component_states(data)
                    .with(states.get(entity).copied().unwrap_or_default()),
                position,
                previous,
            })
        })
        .collect();
    Elements { nodes }
}
