//! Where a UI element ends up once its parents have had their say.
//!
//! The overlay's rule used to be one line: an element's anchor picks a point on
//! the viewport and its transform is an offset from that point. That is exactly
//! right for a HUD reading, and it quietly means a hierarchy is not one. A label
//! parented to a card was placed against the *screen*, so six cards' worth of
//! labels landed on top of each other however far apart the cards were; and a
//! layout's spacing reached the code that decides what was clicked but never the
//! code that decides what is drawn, so an element could be clickable somewhere
//! it was not.
//!
//! This resolves both, once, for everything: the frame, the pointer, and the
//! editor's handles all read the same answer. An element's placement is its own
//! offset composed with every ancestor's, plus whatever a parent's layout has to
//! say about where it sits among its siblings.
//!
//! ## What is inherited, and what is not
//!
//! Position and rotation compose; **size does not**. That is a consequence of
//! what a UI transform means here: `scale` is the element's size in overlay
//! units, not a multiplier on a coordinate space. A card two units wide holding
//! a label is not asking for the label to be two times anything — it is asking
//! for a label on a card. Inheriting size would make every child of a wide panel
//! wide, which is not what anyone drawing a panel means, and it is why this is
//! deliberately not a full `RectTransform`: that answers a different question
//! (how a child *stretches* with its parent) and answering it needs anchors with
//! two corners rather than one point.
//!
//! The anchor is taken from the outermost ancestor that declares one, because a
//! child re-anchoring against the screen is how a label leaves the card it is
//! written on when the window changes shape.

use std::collections::BTreeMap;

use glam::{Quat, Vec2};
use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, Transform3D, World};

use super::{UiButtonComponent, UiLayoutComponent};
use crate::{UiAnchor, UiImageComponent, UiTextComponent};

const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPlaced {
    pub offset: Vec2,
    pub rotation: Quat,
    pub anchor: UiAnchor,
}

impl UiPlaced {
    #[must_use]
    pub fn at_anchor(anchor: UiAnchor) -> Self {
        Self {
            offset: Vec2::ZERO,
            rotation: Quat::IDENTITY,
            anchor,
        }
    }

    #[must_use]
    fn with_child(self, offset: Vec2, rotation: Quat) -> Self {
        let turned = self.rotation * offset.extend(0.0);
        Self {
            offset: self.offset + turned.truncate(),
            rotation: self.rotation * rotation,
            anchor: self.anchor,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiHierarchy {
    placed: BTreeMap<EntityId, UiPlaced>,
}

impl UiHierarchy {
    pub fn of(
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Result<Self, ComponentRegistryError> {
        let anchors = declared_anchors(world, components)?;
        let layouts = layout_offsets(world, components)?;
        let mut placed = BTreeMap::new();
        for entity in anchors.keys().copied() {
            let resolved = resolve(world, &anchors, &layouts, entity);
            placed.insert(entity, resolved);
        }
        Ok(Self { placed })
    }

    #[must_use]
    pub fn placement(&self, entity: EntityId) -> Option<UiPlaced> {
        self.placed.get(&entity).copied()
    }

    #[must_use]
    pub fn placement_or(&self, entity: EntityId, anchor: UiAnchor) -> UiPlaced {
        self.placement(entity)
            .unwrap_or_else(|| UiPlaced::at_anchor(anchor))
    }
}

fn resolve(
    world: &World,
    anchors: &BTreeMap<EntityId, UiAnchor>,
    layouts: &BTreeMap<EntityId, Vec2>,
    entity: EntityId,
) -> UiPlaced {
    let mut chain = vec![entity];
    let mut walker = entity;
    while chain.len() < MAX_DEPTH {
        let Some(parent) = world.get(walker).and_then(|data| data.parent) else {
            break;
        };
        chain.push(parent);
        walker = parent;
    }
    let anchor = chain
        .iter()
        .rev()
        .find_map(|entity| anchors.get(entity).copied())
        .or_else(|| anchors.get(&entity).copied())
        .unwrap_or_default();

    let mut placed = UiPlaced::at_anchor(anchor);
    for link in chain.iter().rev().copied() {
        let transform = world
            .get(link)
            .and_then(|data| data.transform_3d)
            .unwrap_or_default();
        let layout = layouts.get(&link).copied().unwrap_or(Vec2::ZERO);
        placed = placed.with_child(
            Vec2::from_array(transform.position_2d()) + layout,
            rotation_of(transform),
        );
    }
    placed
}

fn rotation_of(transform: Transform3D) -> Quat {
    let raw = Quat::from_array(transform.rotation);
    if raw.length_squared() > f32::EPSILON {
        raw.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn declared_anchors(
    world: &World,
    components: &ComponentSchemaRegistry,
) -> Result<BTreeMap<EntityId, UiAnchor>, ComponentRegistryError> {
    let mut anchors = BTreeMap::new();
    for (entity, image) in components.query::<UiImageComponent>(world)? {
        anchors.insert(entity, image.anchor);
    }
    for (entity, text) in components.query::<UiTextComponent>(world)? {
        anchors.entry(entity).or_insert(text.anchor);
    }
    for (entity, _) in components.query::<UiButtonComponent>(world)? {
        anchors.entry(entity).or_insert(UiAnchor::Center);
    }
    Ok(anchors)
}

/// What each laid-out child owes to its parent's layout.
///
/// Only active children count. Box-aware alignment reads the already-authored
/// parent and child sizes, so rendering, hit testing, and editor handles all
/// consume the same resolved placement rather than reimplementing alignment.
fn layout_offsets(
    world: &World,
    components: &ComponentSchemaRegistry,
) -> Result<BTreeMap<EntityId, Vec2>, ComponentRegistryError> {
    let mut offsets = BTreeMap::new();
    for (parent, layout) in components.query::<UiLayoutComponent>(world)? {
        let Some(data) = world.get(parent) else {
            continue;
        };
        let parent_size = data.transform_3d.unwrap_or_default().scale_2d();
        let shown: Vec<EntityId> = data
            .children
            .iter()
            .copied()
            .filter(|child| world.is_active(*child))
            .collect();
        for (index, child) in shown.iter().enumerate() {
            let child_size = world
                .get(*child)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default()
                .scale_2d();
            offsets.insert(
                *child,
                Vec2::from_array(layout.offset_in_box(
                    index,
                    shown.len(),
                    parent_size,
                    child_size,
                )),
            );
        }
    }
    Ok(offsets)
}

#[cfg(test)]
mod tests {
    use super::{UiHierarchy, UiPlaced};
    use crate::UiAnchor;
    use crate::extract::SceneExtractor;
    use glam::{Quat, Vec2};
    use sindri_core::{SceneDocument, World};

    fn world(entities: &str) -> (World, SceneExtractor) {
        let document = format!(
            r#"{{ "format_version": 9, "metadata": {{ "name": "t" }},
                 "entities": [{entities}] }}"#
        );
        let extractor = SceneExtractor::new().expect("built-in schemas register");
        let parsed = SceneDocument::from_json(&document).expect("the fragment parses");
        let world = World::from_scene(&parsed)
            .expect("the fragment loads")
            .world;
        (world, extractor)
    }

    fn placement(world: &World, extractor: &SceneExtractor, name: &str) -> UiPlaced {
        let hierarchy =
            UiHierarchy::of(world, extractor.components()).expect("the hierarchy resolves");
        let entity = world
            .entities()
            .find(|(_, data)| data.name.as_deref() == Some(name))
            .map_or_else(|| panic!("{name} is in the world"), |(entity, _)| entity);
        hierarchy
            .placement(entity)
            .unwrap_or_else(|| panic!("{name} is a UI element"))
    }

    const IMAGE: &str = r#""sindri.ui.image": { "texture": "sindri:white", "anchor": "center" }"#;

    #[test]
    fn a_child_is_placed_from_its_parent_rather_than_from_the_screen() {
        let (world, extractor) = world(&format!(
            r#"{{ "id": "panel", "name": "panel",
                  "transform_3d": {{ "position": [0.3, -0.2, 0.0] }},
                  "components": {{ {IMAGE} }} }},
               {{ "id": "label", "name": "label", "parent": "panel",
                  "transform_3d": {{ "position": [0.05, 0.1, 0.0] }},
                  "components": {{ {IMAGE} }} }}"#
        ));
        let label = placement(&world, &extractor, "label");
        assert!((label.offset - Vec2::new(0.35, -0.1)).length() < 1.0e-6);
    }

    #[test]
    fn a_turned_parent_turns_where_its_children_sit() {
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let [x, y, z, w] = quarter.to_array();
        let (world, extractor) = world(&format!(
            r#"{{ "id": "panel", "name": "panel",
                  "transform_3d": {{ "rotation": [{x}, {y}, {z}, {w}] }},
                  "components": {{ {IMAGE} }} }},
               {{ "id": "label", "name": "label", "parent": "panel",
                  "transform_3d": {{ "position": [0.2, 0.0, 0.0] }},
                  "components": {{ {IMAGE} }} }}"#
        ));
        let label = placement(&world, &extractor, "label");
        assert!((label.offset - Vec2::new(0.0, 0.2)).length() < 1.0e-5);
    }

    #[test]
    fn a_child_keeps_its_parents_anchor() {
        let (world, extractor) = world(
            r#"{ "id": "panel", "name": "panel",
                 "components": { "sindri.ui.image": { "texture": "sindri:white",
                     "anchor": "bottom_right" } } },
               { "id": "label", "name": "label", "parent": "panel",
                 "components": { "sindri.ui.image": { "texture": "sindri:white",
                     "anchor": "top_left" } } }"#,
        );
        assert_eq!(
            placement(&world, &extractor, "label").anchor,
            UiAnchor::BottomRight
        );
        assert_eq!(
            placement(&world, &extractor, "panel").anchor,
            UiAnchor::BottomRight
        );
    }

    #[test]
    fn an_ancestor_that_draws_nothing_still_moves_its_children() {
        let (world, extractor) = world(&format!(
            r#"{{ "id": "menu", "name": "menu",
                  "transform_3d": {{ "position": [0.0, 0.5, 0.0] }} }},
               {{ "id": "label", "name": "label", "parent": "menu",
                  "transform_3d": {{ "position": [0.0, -0.1, 0.0] }},
                  "components": {{ {IMAGE} }} }}"#
        ));
        let label = placement(&world, &extractor, "label");
        assert!((label.offset.y - 0.4).abs() < 1.0e-6);
    }

    #[test]
    fn a_layout_spaces_what_is_drawn_and_not_only_what_is_clicked() {
        let (world, extractor) = world(&format!(
            r#"{{ "id": "row", "name": "row",
                  "components": {{ "sindri.ui.layout": {{ "direction": "column",
                      "spacing": 0.4 }} }} }},
               {{ "id": "first", "name": "first", "parent": "row",
                  "components": {{ {IMAGE} }} }},
               {{ "id": "second", "name": "second", "parent": "row",
                  "components": {{ {IMAGE} }} }}"#
        ));
        let first = placement(&world, &extractor, "first").offset;
        let second = placement(&world, &extractor, "second").offset;
        assert!((first.y - second.y).abs() > 0.39);
    }

    #[test]
    fn layout_alignment_uses_parent_and_child_boxes() {
        let (world, extractor) = world(&format!(
            r#"{{ "id": "row", "name": "row",
                  "transform_3d": {{ "scale": [4.0, 2.0, 1.0] }},
                  "components": {{ "sindri.ui.layout": {{ "direction": "row",
                      "spacing": 0.5, "justify": "start", "align": "end" }} }} }},
               {{ "id": "first", "name": "first", "parent": "row",
                  "transform_3d": {{ "scale": [1.0, 0.5, 1.0] }},
                  "components": {{ {IMAGE} }} }}"#
        ));
        let first = placement(&world, &extractor, "first").offset;
        assert!((first - Vec2::new(-1.5, -0.75)).length() < 1.0e-6, "{first:?}");
    }

    #[test]
    fn a_button_with_no_art_is_still_placed() {
        let (world, extractor) = world(
            r#"{ "id": "row", "name": "row",
                 "components": { "sindri.ui.layout": { "direction": "column",
                     "spacing": 0.5 } } },
               { "id": "first", "name": "first", "parent": "row",
                 "components": { "sindri.ui.button": { "label": "one" } } },
               { "id": "second", "name": "second", "parent": "row",
                 "components": { "sindri.ui.button": { "label": "two" } } }"#,
        );
        let first = placement(&world, &extractor, "first").offset;
        let second = placement(&world, &extractor, "second").offset;
        assert!((first.y - second.y).abs() > 0.49);
    }

    #[test]
    fn an_element_with_no_parents_is_unchanged() {
        let (world, extractor) = world(
            r#"{ "id": "hud", "name": "hud",
                 "transform_3d": { "position": [0.1, -0.3, 0.0] },
                 "components": { "sindri.ui.image": { "texture": "sindri:white",
                     "anchor": "top_left" } } }"#,
        );
        let hud = placement(&world, &extractor, "hud");
        assert!((hud.offset - Vec2::new(0.1, -0.3)).length() < 1.0e-6);
        assert_eq!(hud.anchor, UiAnchor::TopLeft);
        assert!(hud.rotation.abs_diff_eq(Quat::IDENTITY, 1.0e-6));
    }
}
