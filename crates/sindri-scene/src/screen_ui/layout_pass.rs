//! Every layout in the world, resolved in the order their sizes depend on.
//!
//! Two passes, because size flows both ways in a UI. An element that fits its
//! content needs its children's sizes first, so those are worked out from the
//! innermost layout outwards. A child that grows, shrinks or stretches needs
//! its parent's size first, so placing runs from the outermost layout inwards,
//! and a panel grown by its parent lays its own children out in the room it
//! was given.
//!
//! Only active children count, which is what makes a menu close up around a
//! hidden entry instead of leaving a hole where it was.

use std::collections::BTreeMap;

use glam::Vec2;
use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, World};

use super::layout::UiLayoutBox;
use super::{UiBoxComponent, UiGridComponent, UiLayoutChild, UiLayoutComponent, UiTextSizes};

/// A parent that places its children: a flex line, or a grid.
#[derive(Clone, Debug)]
enum Container {
    Flex(UiLayoutComponent),
    Grid(UiGridComponent),
}

impl Container {
    fn fit_content(&self) -> [bool; 2] {
        match self {
            Self::Flex(flex) => flex.fit_content,
            Self::Grid(grid) => grid.fit_content,
        }
    }

    fn content_size(&self, padding: super::UiSides, children: &[UiLayoutChild]) -> [f32; 2] {
        match self {
            Self::Flex(flex) => flex.content_size(padding, children),
            Self::Grid(grid) => super::grid::content_size(grid, padding, children),
        }
    }

    fn resolve(
        &self,
        size: [f32; 2],
        padding: super::UiSides,
        children: &[UiLayoutChild],
    ) -> Vec<UiLayoutBox> {
        match self {
            Self::Flex(flex) => flex.resolve_boxes(size, padding, children),
            Self::Grid(grid) => super::grid::resolve(grid, size, padding, children),
        }
    }
}

/// How deep a parent chain is followed; see the hierarchy's own bound.
const MAX_DEPTH: usize = 64;

/// What the layouts decided: where each laid-out child sits relative to its
/// parent, and every size a layout changed.
#[derive(Debug, Default)]
pub(super) struct Laid {
    pub offsets: BTreeMap<EntityId, Vec2>,
    pub sizes: BTreeMap<EntityId, [f32; 2]>,
}

pub(super) fn lay_out(
    world: &World,
    components: &ComponentSchemaRegistry,
    text: &UiTextSizes,
) -> Result<Laid, ComponentRegistryError> {
    let boxes: BTreeMap<EntityId, UiBoxComponent> = components
        .query::<UiBoxComponent>(world)?
        .into_iter()
        .collect();
    // An entity that is both keeps its flex layout, which it had first.
    let mut by_entity: BTreeMap<EntityId, Container> = components
        .query::<UiGridComponent>(world)?
        .into_iter()
        .map(|(entity, grid)| (entity, Container::Grid(grid)))
        .collect();
    for (entity, flex) in components.query::<UiLayoutComponent>(world)? {
        by_entity.insert(entity, Container::Flex(flex));
    }
    let mut layouts: Vec<(EntityId, Container)> = by_entity
        .iter()
        .map(|(entity, container)| (*entity, container.clone()))
        .collect();
    let scene = Context {
        world,
        boxes: &boxes,
        layouts: &by_entity,
        text,
    };
    // Innermost first for fitting; the placing pass walks it backwards.
    layouts.sort_by_key(|(entity, _)| std::cmp::Reverse(depth(world, *entity)));

    let mut laid = Laid::default();
    // Measured words first, so a layout fitting its children sees them.
    for (entity, words) in text {
        let own = scene.item(*entity);
        if !own.fit_content.iter().any(|fits| *fits) {
            continue;
        }
        let mut size = size_of(world, &laid.sizes, *entity);
        let inset = [
            own.padding[1].max(0.0) + own.padding[3].max(0.0),
            own.padding[0].max(0.0) + own.padding[2].max(0.0),
        ];
        for axis in 0..2 {
            if own.fit_content[axis] {
                size[axis] = own.clamp(axis, words[axis] + inset[axis]);
            }
        }
        laid.sizes.insert(*entity, size);
    }
    for (parent, layout) in &layouts {
        let fits = layout.fit_content();
        if !fits.iter().any(|fits| *fits) {
            continue;
        }
        let own = boxes.get(parent).copied().unwrap_or_default();
        let children = scene.children_of(&laid.sizes, *parent);
        let content = layout.content_size(own.padding, &children);
        let mut size = size_of(world, &laid.sizes, *parent);
        for axis in 0..2 {
            if fits[axis] {
                size[axis] = own.clamp(axis, content[axis]);
            }
        }
        laid.sizes.insert(*parent, size);
    }

    for (parent, layout) in layouts.iter().rev() {
        let padding = boxes.get(parent).map_or([0.0; 4], |own| own.padding);
        let shown = shown_children(world, *parent);
        let children = scene.children_of(&laid.sizes, *parent);
        let parent_size = size_of(world, &laid.sizes, *parent);
        let resolved = layout.resolve(parent_size, padding, &children);
        for (child, placed) in shown.into_iter().zip(resolved) {
            laid.offsets.insert(child, Vec2::from_array(placed.offset));
            laid.sizes.insert(child, placed.size);
        }
    }
    Ok(laid)
}

fn shown_children(world: &World, parent: EntityId) -> Vec<EntityId> {
    world.get(parent).map_or_else(Vec::new, |data| {
        data.children
            .iter()
            .copied()
            .filter(|child| world.is_active(*child))
            .collect()
    })
}

/// What every step of laying out reads and none changes.
struct Context<'a> {
    world: &'a World,
    boxes: &'a BTreeMap<EntityId, UiBoxComponent>,
    layouts: &'a BTreeMap<EntityId, Container>,
    text: &'a UiTextSizes,
}

impl Context<'_> {
    fn children_of(
        &self,
        sizes: &BTreeMap<EntityId, [f32; 2]>,
        parent: EntityId,
    ) -> Vec<UiLayoutChild> {
        shown_children(self.world, parent)
            .into_iter()
            .map(|child| UiLayoutChild {
                size: size_of(self.world, sizes, child),
                item: self.item(child),
                min_content: self.min_content(sizes, child),
            })
            .collect()
    }

    fn item(&self, entity: EntityId) -> UiBoxComponent {
        self.boxes.get(&entity).copied().unwrap_or_default()
    }

    /// A layout's automatic minimum: its children as they are, and its
    /// padding. One level deep, which is where the content a shrinking
    /// element would cut into is.
    fn min_content(&self, sizes: &BTreeMap<EntityId, [f32; 2]>, entity: EntityId) -> [f32; 2] {
        let Some(layout) = self.layouts.get(&entity) else {
            // A measured text's words are its minimum: a label does not
            // shrink into itself. (No wrapping to a narrower block yet.)
            return self.text.get(&entity).map_or([0.0; 2], |words| {
                let padding = self.item(entity).padding;
                [
                    words[0] + padding[1].max(0.0) + padding[3].max(0.0),
                    words[1] + padding[0].max(0.0) + padding[2].max(0.0),
                ]
            });
        };
        let children: Vec<UiLayoutChild> = shown_children(self.world, entity)
            .into_iter()
            .map(|child| UiLayoutChild {
                size: size_of(self.world, sizes, child),
                item: self.item(child),
                min_content: [0.0; 2],
            })
            .collect();
        layout.content_size(self.item(entity).padding, &children)
    }
}

/// An element's size as the layouts have left it so far, or its own.
fn size_of(world: &World, sizes: &BTreeMap<EntityId, [f32; 2]>, entity: EntityId) -> [f32; 2] {
    sizes.get(&entity).copied().unwrap_or_else(|| {
        let own = world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .unwrap_or_default()
            .scale_2d();
        [own[0].abs(), own[1].abs()]
    })
}

fn depth(world: &World, entity: EntityId) -> usize {
    let mut depth = 0;
    let mut current = world.get(entity).and_then(|data| data.parent);
    while let Some(parent) = current {
        depth += 1;
        if depth >= MAX_DEPTH {
            break;
        }
        current = world.get(parent).and_then(|data| data.parent);
    }
    depth
}
