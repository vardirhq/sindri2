//! What the person is pointing at, on the screen rather than in the world.
//!
//! Runtime state beside the world, derived from what a scene authors and never
//! serialized. A host updates it once a frame before scripts run.

mod hierarchy;
mod layout;
mod rect;
mod slider;

use std::collections::BTreeMap;

use crate::{UiAnchor, UiImageComponent, UiTextComponent};
use serde::Deserialize;
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, EntityId, PressId, PressPhase, Presses,
    SceneComponent, World,
};

pub use hierarchy::{UiHierarchy, UiPlaced};
pub use layout::{UiAlign, UiDirection, UiJustify, UiLayoutComponent};
pub use rect::{SafeArea, ScreenExtent, ScreenRect};
pub use slider::{UiSliderComponent, UiSliderOrientation};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UiButtonComponent {
    #[serde(default)]
    pub label: String,
}

impl SceneComponent for UiButtonComponent {
    const TYPE_NAME: &'static str = "sindri.ui.button";
}

#[derive(Debug, Default)]
pub struct ScreenUi {
    viewport_half: [f32; 2],
    rects: BTreeMap<EntityId, Element>,
    hovered: Option<EntityId>,
    pressing: Option<EntityId>,
    clicked: Option<EntityId>,
    pointer_overlay: Option<[f32; 2]>,
    slider_drag: Option<(EntityId, PressId)>,
    slider_changed: Option<EntityId>,
}

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

    pub fn update(
        &mut self,
        world: &mut World,
        components: &ComponentSchemaRegistry,
        extent: ScreenExtent,
        presses: &Presses,
    ) -> Result<(), ComponentRegistryError> {
        self.viewport_half = extent.half();
        self.rects = Self::place(world, components, extent)?;
        self.read_presses(world, extent, presses);
        Ok(())
    }

    #[must_use]
    pub const fn pointer_overlay(&self) -> Option<[f32; 2]> {
        self.pointer_overlay
    }

    #[must_use]
    pub fn viewport_aspect(&self) -> f32 {
        let aspect = self.viewport_half[0];
        if aspect > 0.0 { aspect } else { 1.0 }
    }

    #[must_use]
    pub const fn hovered(&self) -> Option<EntityId> {
        self.hovered
    }

    #[must_use]
    pub const fn captures_pointer(&self) -> bool {
        self.hovered.is_some() || self.slider_drag.is_some()
    }

    #[must_use]
    pub fn is_hovered(&self, entity: EntityId) -> bool {
        self.hovered == Some(entity)
    }

    #[must_use]
    pub fn is_pressed(&self, entity: EntityId) -> bool {
        self.clicked == Some(entity)
    }

    #[must_use]
    pub fn is_held(&self, entity: EntityId) -> bool {
        self.slider_drag
            .is_some_and(|(dragged, _)| dragged == entity)
            || (self.pressing == Some(entity) && self.hovered == Some(entity))
    }

    #[must_use]
    pub fn slider_changed(&self, entity: EntityId) -> bool {
        self.slider_changed == Some(entity)
    }

    #[must_use]
    pub fn rect(&self, entity: EntityId) -> Option<ScreenRect> {
        self.rects.get(&entity).map(|element| element.rect)
    }

    fn place(
        world: &World,
        components: &ComponentSchemaRegistry,
        extent: ScreenExtent,
    ) -> Result<BTreeMap<EntityId, Element>, ComponentRegistryError> {
        let mut placements = BTreeMap::new();
        let hierarchy = UiHierarchy::of(world, components)?;
        for (entity, anchor, layer, pressable) in Self::elements(world, components)? {
            if !world.is_active(entity) {
                continue;
            }
            let Some(data) = world.get(entity) else {
                continue;
            };
            let placed = hierarchy.placement_or(entity, anchor);
            let origin = extent.anchor_origin(placed.anchor.unit_offset());
            let size = data.transform_3d.unwrap_or_default().scale_2d();
            placements.insert(
                entity,
                Element {
                    rect: ScreenRect {
                        center: [origin[0] + placed.offset.x, origin[1] + placed.offset.y],
                        size,
                    },
                    layer,
                    pressable,
                },
            );
        }
        Ok(placements)
    }

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
            found
                .entry(entity)
                .or_insert((UiAnchor::Center, 0, false))
                .2 = true;
        }
        for (entity, slider) in components.query::<UiSliderComponent>(world)? {
            if !slider.disabled {
                found
                    .entry(entity)
                    .or_insert((UiAnchor::Center, 0, false))
                    .2 = true;
            }
        }
        Ok(found
            .into_iter()
            .map(|(entity, (anchor, layer, pressable))| (entity, anchor, layer, pressable))
            .collect())
    }

    fn read_presses(&mut self, world: &mut World, extent: ScreenExtent, presses: &Presses) {
        self.clicked = None;
        self.slider_changed = None;
        self.pointer_overlay = presses
            .focus()
            .and_then(|position| extent.pointer(position));
        self.hovered = self
            .pointer_overlay
            .and_then(|point| self.topmost_at(point));

        let Some(press) = presses.primary() else {
            self.pressing = None;
            self.slider_drag = None;
            return;
        };
        if press.began_now() {
            if let Some(entity) = self.hovered
                && world
                    .get(entity)
                    .and_then(|data| data.components.get(UiSliderComponent::TYPE_NAME))
                    .and_then(|payload| {
                        serde_json::from_value::<UiSliderComponent>(payload.clone()).ok()
                    })
                    .is_some_and(|slider| !slider.disabled)
            {
                self.slider_drag = Some((entity, press.id()));
                self.pressing = None;
            } else {
                self.pressing = self.hovered;
            }
        }
        if let Some((entity, id)) = self.slider_drag
            && id == press.id()
        {
            if let Some(point) = extent.pointer(press.position()) {
                self.update_slider(world, entity, point);
            }
            if !matches!(press.phase(), PressPhase::Live) {
                self.slider_drag = None;
            }
            return;
        }
        match press.phase() {
            PressPhase::Live => {}
            PressPhase::Ended => {
                if self.pressing.is_some() && self.pressing == self.hovered {
                    self.clicked = self.pressing;
                }
                self.pressing = None;
            }
            PressPhase::Cancelled => self.pressing = None,
        }
    }

    fn update_slider(&mut self, world: &mut World, entity: EntityId, point: [f32; 2]) {
        let Some(element) = self.rects.get(&entity) else {
            return;
        };
        let Some(data) = world.get_mut(entity) else {
            return;
        };
        let Some(payload) = data.components.get_mut(UiSliderComponent::TYPE_NAME) else {
            return;
        };
        let Ok(slider) = serde_json::from_value::<UiSliderComponent>(payload.clone()) else {
            return;
        };
        if slider.disabled {
            return;
        }
        if element.rect.size[0] <= 0.0 || element.rect.size[1] <= 0.0 {
            return;
        }
        let normalized = match slider.orientation {
            UiSliderOrientation::Horizontal => {
                let low = element.rect.center[0] - element.rect.size[0] / 2.0;
                ((point[0] - low) / element.rect.size[0]).clamp(0.0, 1.0)
            }
            UiSliderOrientation::Vertical => {
                let low = element.rect.center[1] - element.rect.size[1] / 2.0;
                (1.0 - (point[1] - low) / element.rect.size[1]).clamp(0.0, 1.0)
            }
        };
        let value = slider.value_at(normalized);
        if (value - slider.value).abs() <= f32::EPSILON {
            return;
        }
        payload["value"] = serde_json::json!(value);
        self.slider_changed = Some(entity);
    }

    fn topmost_at(&self, point: [f32; 2]) -> Option<EntityId> {
        self.rects
            .iter()
            .filter(|(_, element)| element.pressable && element.rect.contains(point))
            .max_by_key(|(entity, element)| (element.layer, entity.index()))
            .map(|(entity, _)| *entity)
    }
}
