//! What text elements measure, for the layouts that size to them.
//!
//! A label's size is decided by its font, and only a host holding that font
//! can work it out: the layout in this crate has no glyphs. So a host measures
//! the text elements that asked to fit their words, through the same shaping
//! it draws them with, and hands the sizes to layout, drawing and hit-testing
//! alike. An element that did not ask keeps its authored size, and a host that
//! measures nothing lays everything out as it did before.

use std::collections::BTreeMap;

use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, World};
use sindri_render::TextRenderer;

use super::UiBoxComponent;
use crate::UiTextComponent;

/// Each measured text element's words, across and down, in overlay units.
pub type UiTextSizes = BTreeMap<EntityId, [f32; 2]>;

/// Measures every text element whose box asks to fit its content.
///
/// On an axis it fits, a text is measured with no limit there, so a label is
/// one line as wide as its words; on an axis it does not, its own bounds hold,
/// so a paragraph of fixed width fits its height to the lines it wraps into.
/// A text whose font is not bound measures nothing and keeps its own size.
pub fn measure_ui_text(
    world: &World,
    components: &ComponentSchemaRegistry,
    text: &mut TextRenderer,
) -> Result<UiTextSizes, ComponentRegistryError> {
    let boxes: BTreeMap<EntityId, UiBoxComponent> = components
        .query::<UiBoxComponent>(world)?
        .into_iter()
        .filter(|(_, own)| own.fit_content.iter().any(|fits| *fits))
        .collect();
    let mut sizes = UiTextSizes::new();
    if boxes.is_empty() {
        return Ok(sizes);
    }
    for (entity, component) in components.query::<UiTextComponent>(world)? {
        let Some(own) = boxes.get(&entity) else {
            continue;
        };
        if !world.is_active(entity) {
            continue;
        }
        let mut component = component;
        for (axis, fits) in own.fit_content.iter().enumerate() {
            if *fits {
                component.bounds[axis] = 0.0;
            }
        }
        let Ok(instance) = component.instance([0.0, 0.0]) else {
            continue;
        };
        if let Some(size) = text.measure(&instance) {
            sizes.insert(entity, size);
        }
    }
    Ok(sizes)
}
