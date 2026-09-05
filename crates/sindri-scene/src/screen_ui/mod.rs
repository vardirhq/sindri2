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

mod hierarchy;
mod layout;
mod rect;

use std::collections::BTreeMap;

use crate::{UiAnchor, UiImageComponent, UiTextComponent};
use serde::Deserialize;
use sindri_core::{
    ComponentRegistryError, ComponentSchemaRegistry, EntityId, PressPhase, Presses, SceneComponent,
    World,
};

pub use hierarchy::{UiHierarchy, UiPlaced};
pub use layout::{UiDirection, UiLayoutComponent};
pub use rect::{SafeArea, ScreenExtent, ScreenRect};

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
    /// Half the viewport in overlay units: `[aspect, 1]`.
    ///
    /// Kept with the other facts derived from the screen so a script can
    /// author responsive world-space rules without learning viewport pixels.
    viewport_half: [f32; 2],
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
    /// Where the pointer is, in the overlay's own units.
    ///
    /// Computed here because this is where it is already computed: hit-testing
    /// a button needs the pointer in the space the buttons are laid out in. A
    /// gameplay script aiming at a point in the world needs the same number,
    /// and had no way to get it — `Pointer.x` is viewport pixels, and how many
    /// pixels tall the window is is not something the scene knows.
    pointer_overlay: Option<[f32; 2]>,
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
        presses: &Presses,
    ) -> Result<(), ComponentRegistryError> {
        self.viewport_half = extent.half();
        self.rects.clear();
        let placements = Self::place(world, components, extent)?;
        self.rects = placements;
        self.read_presses(extent, presses);
        Ok(())
    }

    /// Where the pointer is in overlay units, or `None` when it is not over
    /// the viewport at all.
    ///
    /// The overlay is two tall and centred on the origin, running out to the
    /// aspect ratio either side — so a scene that authored its camera knows
    /// what these numbers are worth in world units, and the engine does not
    /// have to guess at a camera on a script's behalf.
    #[must_use]
    pub const fn pointer_overlay(&self) -> Option<[f32; 2]> {
        self.pointer_overlay
    }

    /// The viewport width divided by its height.
    ///
    /// A screen not laid out yet answers one, the same finite square fallback
    /// as [`ScreenExtent::new`] uses for a viewport with no area.
    #[must_use]
    pub fn viewport_aspect(&self) -> f32 {
        let aspect = self.viewport_half[0];
        if aspect > 0.0 { aspect } else { 1.0 }
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
        // The same resolution the frame is drawn from. This used to fold in a
        // parent's layout spacing and nothing else, so an element could be
        // clickable somewhere it was never drawn — and a child of a panel was
        // clickable against the screen rather than against the panel.
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
            // The element's own size, never its parents': `scale` is a size in
            // overlay units here rather than a multiplier on a space.
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

    /// Hover and click, from the press the person is making.
    fn read_presses(&mut self, extent: ScreenExtent, presses: &Presses) {
        self.clicked = None;
        // Where to test against: the press being made, or — for a device that
        // rests somewhere, which a finger does not — where it is resting.
        self.pointer_overlay = presses
            .focus()
            .and_then(|position| extent.pointer(position));
        self.hovered = self
            .pointer_overlay
            .and_then(|point| self.topmost_at(point));

        let Some(press) = presses.primary() else {
            // Nothing is being pressed, so nothing is armed. A press that
            // disappeared without ending — a browser swallowing it, a window
            // losing focus — must not leave a button armed for ever.
            self.pressing = None;
            return;
        };
        // Arming reads the arrival and completing reads the phase, rather than
        // both reading one value: a tap quick enough to begin and end between
        // two frames -- most taps on a phone -- is both at once, and matching
        // on a single value saw only the ending and armed nothing.
        if press.began_now() {
            self.pressing = self.hovered;
        }
        match press.phase() {
            PressPhase::Live => {}
            PressPhase::Ended => {
                // A click is a press and a release on the same element.
                // Sliding off before letting go is how a person changes their
                // mind, and it has to keep working here for the same reason it
                // does everywhere else.
                //
                // This reads the press rather than the device, which is the
                // whole point: on the frame a finger lifts it is out of the
                // device's live set, and asking the device where the release
                // happened answered "nowhere" — so the release never matched
                // the element the press began on, and a tap never clicked
                // anything.
                if self.pressing.is_some() && self.pressing == self.hovered {
                    self.clicked = self.pressing;
                }
                self.pressing = None;
            }
            // Taken away rather than let go, so it completes nothing.
            PressPhase::Cancelled => self.pressing = None,
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
