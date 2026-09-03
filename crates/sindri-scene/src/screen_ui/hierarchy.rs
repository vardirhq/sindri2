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

/// How deep a parent chain is followed.
///
/// A malformed world can hold a cycle — an entity that is its own ancestor —
/// and a walk up such a chain never ends. Bounded rather than detected, because
/// the bound is also the honest answer to a chain nobody could have authored on
/// purpose: no real UI is sixty-four elements deep.
const MAX_DEPTH: usize = 64;

/// Where one UI element sits, with its ancestors folded in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPlaced {
    /// The offset from the anchor's point on the viewport, in overlay units.
    pub offset: Vec2,
    /// The element's own rotation with its ancestors' turned in.
    pub rotation: Quat,
    /// The anchor `offset` is measured from: the outermost one in the chain.
    pub anchor: UiAnchor,
}

impl UiPlaced {
    /// An element placed by nothing but its own anchor, at the anchor's point.
    #[must_use]
    pub fn at_anchor(anchor: UiAnchor) -> Self {
        Self {
            offset: Vec2::ZERO,
            rotation: Quat::IDENTITY,
            anchor,
        }
    }

    /// This placement with a child's own offset and turn applied inside it.
    ///
    /// The child's offset is turned by the parent's rotation before it is added,
    /// which is what makes a rotated panel carry its contents round with it
    /// rather than sliding them sideways.
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

/// Every UI element's placement, resolved once.
#[derive(Clone, Debug, Default)]
pub struct UiHierarchy {
    placed: BTreeMap<EntityId, UiPlaced>,
}

impl UiHierarchy {
    /// Resolves every UI element in the world.
    ///
    /// Every element, including inactive ones: what is drawn and what is
    /// clickable are decided elsewhere, and an editor showing a hidden screen's
    /// layout needs the same answer as the frame that would draw it.
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

    /// Where this element sits, or `None` for an entity that draws no UI.
    #[must_use]
    pub fn placement(&self, entity: EntityId) -> Option<UiPlaced> {
        self.placed.get(&entity).copied()
    }

    /// Where this element sits, falling back to its own anchor alone.
    ///
    /// For a caller that has an anchor in hand and only wants the hierarchy's
    /// answer where there is one — a tool inspecting an entity mid-edit, before
    /// the hierarchy it belongs to has been resolved again.
    #[must_use]
    pub fn placement_or(&self, entity: EntityId, anchor: UiAnchor) -> UiPlaced {
        self.placement(entity)
            .unwrap_or_else(|| UiPlaced::at_anchor(anchor))
    }
}

/// One element's placement, by walking its chain to the outermost ancestor and
/// composing back down.
fn resolve(
    world: &World,
    anchors: &BTreeMap<EntityId, UiAnchor>,
    layouts: &BTreeMap<EntityId, Vec2>,
    entity: EntityId,
) -> UiPlaced {
    // Up first, collecting the chain, because the anchor belongs to its far end
    // and the composition runs the other way.
    let mut chain = vec![entity];
    let mut walker = entity;
    while chain.len() < MAX_DEPTH {
        let Some(parent) = world.get(walker).and_then(|data| data.parent) else {
            break;
        };
        chain.push(parent);
        walker = parent;
    }
    // The outermost declared anchor wins; an element in a chain that declares
    // none anywhere falls back to its own, which is what a lone HUD reading has.
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

/// The transform's rotation, or none at all if it is not a rotation.
///
/// A quaternion of zeros deserializes happily and normalizes to a NaN, which
/// would put an element nowhere at all rather than somewhere wrong.
fn rotation_of(transform: Transform3D) -> Quat {
    let raw = Quat::from_array(transform.rotation);
    if raw.length_squared() > f32::EPSILON {
        raw.normalize()
    } else {
        Quat::IDENTITY
    }
}

/// The anchor every UI element declares.
///
/// This is also the set of entities that *are* UI elements, which is why the
/// hierarchy is keyed on it: an entity with a transform and no UI component is
/// a group, and a group is placed but never drawn.
///
/// A button counts even with no image or text of its own, because a hit area
/// with no art is a legitimate thing to author — and because leaving it out is
/// not "it gets no anchor", it is "it gets no *placement*", so a row of bare
/// buttons stops being laid out at all. The same set `ScreenUi::elements`
/// collects, for the same reason.
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
        // A button's own anchor comes from whatever it draws with; a bare one
        // is centred, like anything else that says nothing.
        anchors.entry(entity).or_insert(UiAnchor::Center);
    }
    Ok(anchors)
}

/// What each laid-out child owes to its parent's layout.
///
/// Only active children count, and only their index among the active ones,
/// which is what makes a menu close up around a hidden entry instead of leaving
/// a hole where it was.
fn layout_offsets(
    world: &World,
    components: &ComponentSchemaRegistry,
) -> Result<BTreeMap<EntityId, Vec2>, ComponentRegistryError> {
    let mut offsets = BTreeMap::new();
    for (parent, layout) in components.query::<UiLayoutComponent>(world)? {
        let Some(data) = world.get(parent) else {
            continue;
        };
        let shown: Vec<EntityId> = data
            .children
            .iter()
            .copied()
            .filter(|child| world.is_active(*child))
            .collect();
        for (index, child) in shown.iter().enumerate() {
            offsets.insert(*child, Vec2::from_array(layout.offset(index, shown.len())));
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

    /// A world from a scene fragment, with the built-in schemas behind it.
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

    /// A child's offset is measured from where its parent ended up, not from the
    /// screen.
    ///
    /// The bug this is here for put six upgrade cards' labels on top of each
    /// other however far apart the cards were, because each label was placed
    /// against the viewport as though it had no parent at all.
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
        assert!(
            (label.offset - Vec2::new(0.35, -0.1)).length() < 1.0e-6,
            "{label:?}"
        );
    }

    /// A parent's turn carries its children round with it rather than sliding
    /// them sideways.
    #[test]
    fn a_turned_parent_turns_where_its_children_sit() {
        // A quarter turn about Z sends +X to +Y.
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
        assert!(
            (label.offset - Vec2::new(0.0, 0.2)).length() < 1.0e-5,
            "{label:?}"
        );
    }

    /// The anchor comes from the outermost element that declares one, so a label
    /// stays on its card when the window changes shape instead of running off to
    /// its own corner of the screen.
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

    /// A group with no UI component of its own still moves what is under it,
    /// which is what makes "put the whole menu somewhere else" one edit.
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
        assert!((label.offset.y - 0.4).abs() < 1.0e-6, "{label:?}");
    }

    /// A layout spaces its children, and that reaches what is drawn.
    ///
    /// It used to reach only what was clickable, so an element could be
    /// clickable somewhere it was never drawn.
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
        assert!(
            (first.y - second.y).abs() > 0.39,
            "a column of two at 0.4 spacing: {first:?} then {second:?}"
        );
    }

    /// A button with no art of its own is still an element, and is still laid
    /// out.
    ///
    /// Left out of the hierarchy it does not fall back to "no anchor" but to
    /// "no placement", so a row of bare hit areas silently stops being spaced.
    /// A test caught exactly that.
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
        assert!(
            (first.y - second.y).abs() > 0.49,
            "{first:?} then {second:?}"
        );
    }

    /// An element with no parents is placed exactly where it always was.
    #[test]
    fn an_element_with_no_parents_is_unchanged() {
        let (world, extractor) = world(
            r#"{ "id": "hud", "name": "hud",
                 "transform_3d": { "position": [0.1, -0.3, 0.0] },
                 "components": { "sindri.ui.image": { "texture": "sindri:white",
                     "anchor": "top_left" } } }"#,
        );
        let hud = placement(&world, &extractor, "hud");
        assert!(
            (hud.offset - Vec2::new(0.1, -0.3)).length() < 1.0e-6,
            "{hud:?}"
        );
        assert_eq!(hud.anchor, UiAnchor::TopLeft);
        assert!(hud.rotation.abs_diff_eq(Quat::IDENTITY, 1.0e-6));
    }
}
