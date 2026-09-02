//! What the person is pointing at, on the screen rather than in the world.
//!
//! Runtime state beside the world, derived from what a scene authors and never
//! serialized — the same shape as `SpriteAnimations` and `ScenePhysics2d`. A
//! host updates it once a frame, before scripts run, and a script asks it what
//! is hovered and what was clicked.
//!
//! Screens are not a thing here. A screen is an entity with children, showing
//! one is switching it on, and `World.is_active` already governs a subtree —
//! so a menu, a pause overlay and a HUD are the same mechanism the rest of the
//! engine already has, rather than a stack this module would have to own.

mod layout;
mod rect;

use std::collections::BTreeMap;

use crate::{UiAnchor, UiImageComponent, UiTextComponent};
use serde::Deserialize;
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, EntityId, SceneComponent, World,
};

pub use layout::{UiDirection, UiLayoutComponent};
pub use rect::{SafeArea, ScreenExtent, ScreenRect};

/// What the pointer is doing this frame, in the terms this module needs.
///
/// Plain values rather than the platform's input state, because `AGENTS.md`
/// keeps `sindri-scene` off `sindri-platform`: the layer that derives renderer
/// state from a scene has no business knowing what a mouse button is. A host
/// fills this in from whatever it has — a mouse, a finger, a test.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerFrame {
    /// Where the pointer is, in viewport pixels, or `None` when it is not over
    /// the viewport at all.
    pub position: Option<[f32; 2]>,
    /// Whether it went down during this frame.
    pub pressed: bool,
    /// Whether it came up during this frame.
    pub released: bool,
    /// Whether it is down now.
    pub down: bool,
}

/// Something on the screen a person can press.
///
/// Its rect is the entity's own: an element is as big as its transform says,
/// which is what the same transform already means for drawing. A button with
/// no image is a hit area, which is how a whole card is made pressable without
/// the art having to know.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiButtonComponent {
    /// What this button is, for someone who cannot see it.
    ///
    /// Authored here because this is where it belongs — beside the thing it
    /// names, in the file a designer edits. Nothing surfaces it yet: there is
    /// no DOM to surface it to until a project can be exported to the web, and
    /// inventing a second accessibility path before then would be building the
    /// wrong one. It travels with the scene in the meantime.
    #[serde(default)]
    pub label: String,
}

impl SceneComponent for UiButtonComponent {
    const TYPE_NAME: &'static str = "sindri.ui.button";
}

/// Where every screen element is this frame, and what the pointer is doing.
#[derive(Debug, Default)]
pub struct ScreenUi {
    /// Every laid-out element, and whether it can be pressed.
    rects: BTreeMap<EntityId, Element>,
    /// The topmost button under the pointer.
    hovered: Option<EntityId>,
    /// The button a press began on, kept across frames.
    ///
    /// A click is a press and a release on the same button, which is what lets
    /// a person change their mind by sliding off before letting go — the
    /// behaviour every platform's buttons already have.
    pressing: Option<EntityId>,
    /// The button a click completed on this frame.
    clicked: Option<EntityId>,
}

/// One element's box, and how it sorts against the others.
#[derive(Clone, Copy, Debug)]
struct Element {
    rect: ScreenRect,
    layer: i32,
    pressable: bool,
}

impl ScreenUi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Recomputes every element's box and reads the pointer against it.
    ///
    /// Run before scripts, so a script asking what was clicked is asking about
    /// the frame it is in rather than the one before it.
    pub fn update(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
        extent: ScreenExtent,
        pointer: PointerFrame,
    ) -> Result<(), ComponentRegistryError> {
        self.rects.clear();
        let placements = Self::place(world, components, extent)?;
        self.rects = placements;
        self.read_pointer(extent, pointer);
        Ok(())
    }

    /// The topmost pressable element under the pointer, if any.
    #[must_use]
    pub const fn hovered(&self) -> Option<EntityId> {
        self.hovered
    }

    /// Whether a screen element is taking the pointer this frame.
    ///
    /// A gameplay script asks this to keep a click on a pause button from also
    /// firing the gun behind it — see `Pointer.over_ui` in `docs/scripting.md`.
    #[must_use]
    pub const fn captures_pointer(&self) -> bool {
        self.hovered.is_some()
    }

    /// Whether this element is under the pointer.
    #[must_use]
    pub fn is_hovered(&self, entity: EntityId) -> bool {
        self.hovered == Some(entity)
    }

    /// Whether this element was clicked during this frame.
    #[must_use]
    pub fn is_pressed(&self, entity: EntityId) -> bool {
        self.clicked == Some(entity)
    }

    /// Whether the pointer is being held down on this element.
    #[must_use]
    pub fn is_held(&self, entity: EntityId) -> bool {
        self.pressing == Some(entity) && self.hovered == Some(entity)
    }

    /// Where an element ended up, for a host that draws or tests it.
    #[must_use]
    pub fn rect(&self, entity: EntityId) -> Option<ScreenRect> {
        self.rects.get(&entity).map(|element| element.rect)
    }

    /// Every element's box, laid out.
    fn place(
        world: &World,
        components: &ComponentSchemaRegistry,
        extent: ScreenExtent,
    ) -> Result<BTreeMap<EntityId, Element>, ComponentRegistryError> {
        let mut placements = BTreeMap::new();
        let laid_out = Self::child_offsets(world, components)?;
        for (entity, anchor, layer, pressable) in Self::elements(world, components)? {
            if !world.is_active(entity) {
                continue;
            }
            let Some(data) = world.get(entity) else {
                continue;
            };
            let transform = data.transform_3d.unwrap_or_default();
            let origin = extent.anchor_origin(anchor.unit_offset());
            let offset = laid_out.get(&entity).copied().unwrap_or([0.0, 0.0]);
            let position = transform.position_2d();
            let size = transform.scale_2d();
            placements.insert(
                entity,
                Element {
                    rect: ScreenRect {
                        center: [
                            origin[0] + position[0] + offset[0],
                            origin[1] + position[1] + offset[1],
                        ],
                        size,
                    },
                    layer,
                    pressable,
                },
            );
        }
        Ok(placements)
    }

    /// What each laid-out child owes to its parent's layout.
    ///
    /// Only active children count, and only their index among the active ones,
    /// which is what makes a menu close up around a hidden entry instead of
    /// leaving a hole where it was.
    fn child_offsets(
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Result<BTreeMap<EntityId, [f32; 2]>, ComponentRegistryError> {
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
                offsets.insert(*child, layout.offset(index, shown.len()));
            }
        }
        Ok(offsets)
    }

    /// Every entity carrying a screen element, with what placing it needs.
    ///
    /// A button contributes even with no image or text of its own, because a
    /// hit area with no art is a legitimate thing to author.
    fn elements(
        world: &World,
        components: &ComponentSchemaRegistry,
    ) -> Result<Vec<(EntityId, UiAnchor, i32, bool)>, ComponentRegistryError> {
        let mut found: BTreeMap<EntityId, (UiAnchor, i32, bool)> = BTreeMap::new();
        for (entity, image) in components.query::<UiImageComponent>(world)? {
            found.insert(entity, (image.anchor, image.layer, false));
        }
        for (entity, text) in components.query::<UiTextComponent>(world)? {
            found
                .entry(entity)
                .or_insert((text.anchor, text.layer, false));
        }
        for (entity, _) in components.query::<UiButtonComponent>(world)? {
            // A button's own anchor comes from whatever it draws with; a bare
            // one is centred, like anything else that says nothing.
            found
                .entry(entity)
                .or_insert((UiAnchor::Center, 0, false))
                .2 = true;
        }
        Ok(found
            .into_iter()
            .map(|(entity, (anchor, layer, pressable))| (entity, anchor, layer, pressable))
            .collect())
    }

    /// Hover and click, from where the pointer is and what it is doing.
    fn read_pointer(&mut self, extent: ScreenExtent, pointer: PointerFrame) {
        self.clicked = None;
        self.hovered = pointer
            .position
            .and_then(|position| extent.pointer(position))
            .and_then(|point| self.topmost_at(point));

        if pointer.pressed {
            self.pressing = self.hovered;
        }
        if pointer.released {
            // A click is a press and a release on the same element. Sliding off
            // before letting go is how a person changes their mind, and it has
            // to keep working here for the same reason it does everywhere else.
            if self.pressing.is_some() && self.pressing == self.hovered {
                self.clicked = self.pressing;
            }
            self.pressing = None;
        }
        // A pointer that left the window without a release — the browser
        // swallowing it, focus lost — must not leave a button armed forever.
        if !pointer.down {
            self.pressing = None;
        }
    }

    /// The pressable element on top at this point.
    ///
    /// Highest layer wins, and the later entity wins a tie, which is the same
    /// order the overlay draws in: what is on top is what is pressed.
    fn topmost_at(&self, point: [f32; 2]) -> Option<EntityId> {
        self.rects
            .iter()
            .filter(|(_, element)| element.pressable && element.rect.contains(point))
            .max_by_key(|(entity, element)| (element.layer, entity.index()))
            .map(|(entity, _)| *entity)
    }
}
