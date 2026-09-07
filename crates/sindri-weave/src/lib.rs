//! Isolated bridge between the Weave language and Sindri UI.
//!
//! The proof of concept deliberately does not add Weave concepts to the engine.
//! It clones the authored [`World`], translates matching Weave declarations into
//! ordinary Sindri transforms/component payloads on that clone, and lets the
//! existing scene/render/input pipeline consume the result normally.

use sindri_core::{EntityId, Transform3D, World};
use thiserror::Error;
use weave::{Stylesheet, Viewport};

mod computed;

use computed::{ComputedStyle, Length};

const RESOLVED_PADDING_FIELD: &str = "_resolved_padding";

#[derive(Debug, Error, PartialEq)]
pub enum ApplyError {
    #[error("entity `{entity}` has invalid `{property}` value `{value}`")]
    InvalidValue {
        entity: String,
        property: String,
        value: String,
    },
}

/// A disposable, styled copy of an authored world.
#[derive(Clone, Debug)]
pub struct PresentationWorld {
    world: World,
}

impl PresentationWorld {
    pub fn resolve(
        source: &World,
        stylesheet: &Stylesheet,
        viewport: Viewport,
    ) -> Result<Self, ApplyError> {
        let mut world = source.clone();
        apply(&mut world, stylesheet, viewport)?;
        Ok(Self { world })
    }

    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }
}

fn apply(world: &mut World, stylesheet: &Stylesheet, viewport: Viewport) -> Result<(), ApplyError> {
    let mut entities: Vec<(EntityId, String, Vec<String>, Vec<String>)> = world
        .entities()
        .filter_map(|(entity, data)| {
            let id = data.source_id.as_ref()?.as_str().to_owned();
            let component_types = data.components.keys().cloned().collect();
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
            Some((entity, id, classes, component_types))
        })
        .collect();
    // Percent sizes and padding resolve against settled ancestor boxes, so
    // parents must resolve before their children regardless of authoring order.
    entities.sort_by_key(|(entity, _, _, _)| hierarchy_depth(world, *entity));

    for (entity, id, classes, component_types) in entities {
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
        let kinds: Vec<&str> = component_types.iter().map(String::as_str).collect();
        let computed = ComputedStyle::resolve(stylesheet, &id, &classes, &kinds, viewport);

        // Visual lengths such as border radius are relative to the final box,
        // so settle both axes and their constraints before decoration.
        apply_sizing(world, entity, &id, &computed, viewport)?;
        for (property, value) in computed.into_declarations() {
            if !matches!(
                property.as_str(),
                "width" | "height" | "min-width" | "max-width" | "min-height" | "max-height"
            ) {
                apply_property(world, entity, &id, &property, &value, viewport)?;
            }
        }
    }
    Ok(())
}

const MAX_HIERARCHY_DEPTH: usize = 64;

fn hierarchy_depth(world: &World, entity: EntityId) -> usize {
    let mut depth = 0;
    let mut current = entity;
    while depth < MAX_HIERARCHY_DEPTH {
        let Some(parent) = world.get(current).and_then(|data| data.parent) else {
            break;
        };
        depth += 1;
        current = parent;
    }
    depth
}

fn apply_sizing(
    world: &mut World,
    entity: EntityId,
    id: &str,
    style: &ComputedStyle,
    viewport: Viewport,
) -> Result<(), ApplyError> {
    let viewport_size = [2.0 * viewport.width / viewport.height.max(1.0), 2.0];
    let parent = world.get(entity).and_then(|data| data.parent);
    let parent_size = parent
        .and_then(|parent| world.get(parent))
        .and_then(|data| data.transform_3d)
        .map_or(viewport_size, Transform3D::scale_2d);
    let parent_padding = parent
        .and_then(|parent| resolved_padding(world, parent))
        .unwrap_or(0.0);
    let parent_content = [
        (parent_size[0].abs() - 2.0 * parent_padding).max(0.0),
        (parent_size[1].abs() - 2.0 * parent_padding).max(0.0),
    ];
    let current = world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default()
        .scale_2d();
    let mut resolved = current;

    for axis in 0..2 {
        let (size_name, min_name, max_name) = if axis == 0 {
            ("width", "min-width", "max-width")
        } else {
            ("height", "min-height", "max-height")
        };
        let basis = parent_content[axis];
        let preferred = dimension(style, id, size_name, viewport, basis)?;
        let minimum = dimension(style, id, min_name, viewport, basis)?;
        let maximum = dimension(style, id, max_name, viewport, basis)?;

        let mut size = preferred.unwrap_or(current[axis].abs());
        if let Some(minimum) = minimum {
            size = size.max(minimum);
        }
        if let Some(maximum) = maximum {
            // CSS gives the minimum precedence when the constraints conflict.
            size = size.min(maximum.max(minimum.unwrap_or(0.0)));
        }
        resolved[axis] = size;
    }

    let data = world.get_mut(entity).expect("entity came from this world");
    let transform = data.transform_3d.get_or_insert_with(Transform3D::default);
    transform.scale[0] = resolved[0];
    transform.scale[1] = resolved[1];
    Ok(())
}

fn dimension(
    style: &ComputedStyle,
    id: &str,
    property: &str,
    viewport: Viewport,
    percent_basis: f32,
) -> Result<Option<f32>, ApplyError> {
    let Some(value) = style.length(property) else {
        return Ok(None);
    };
    let resolved = value
        .map_err(|authored| invalid(id, property, authored))?
        .resolve(viewport, Some(percent_basis))
        .filter(|value| *value >= 0.0)
        .ok_or_else(|| invalid(id, property, style.get(property).unwrap_or_default()))?;
    Ok(Some(resolved))
}

fn apply_property(
    world: &mut World,
    entity: EntityId,
    id: &str,
    property: &str,
    value: &str,
    viewport: Viewport,
) -> Result<(), ApplyError> {
    match property {
        "width" | "height" => {
            let axis = usize::from(property == "height");
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            let data = world.get_mut(entity).expect("entity came from this world");
            let transform = data.transform_3d.get_or_insert_with(Transform3D::default);
            transform.scale[axis] = resolved;
        }
        "x" | "y" => {
            let axis = usize::from(property == "y");
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            let data = world.get_mut(entity).expect("entity came from this world");
            let transform = data.transform_3d.get_or_insert_with(Transform3D::default);
            transform.position[axis] = resolved;
        }
        "anchor" => {
            let stored = anchor(value).ok_or_else(|| invalid(id, property, value))?;
            let data = world.get_mut(entity).expect("entity came from this world");
            for type_name in ["sindri.ui.image", "sindri.ui.text", "sindri.ui.shape"] {
                if let Some(payload) = data.components.get_mut(type_name)
                    && let Some(object) = payload.as_object_mut()
                {
                    object.insert(
                        "anchor".to_owned(),
                        serde_json::Value::String(stored.to_owned()),
                    );
                }
            }
        }
        "direction" => {
            if !matches!(value.trim(), "row" | "column") {
                return Err(invalid(id, property, value));
            }
            set_component_field(
                world,
                entity,
                "sindri.ui.layout",
                "direction",
                value.trim().into(),
            );
        }
        "justify-content" => {
            let stored = match value.trim() {
                "start" => "start",
                "center" => "center",
                "end" => "end",
                "space-between" | "space_between" => "space_between",
                _ => return Err(invalid(id, property, value)),
            };
            set_component_field(world, entity, "sindri.ui.layout", "justify", stored.into());
        }
        "align-items" => {
            let stored = match value.trim() {
                "start" => "start",
                "center" => "center",
                "end" => "end",
                _ => return Err(invalid(id, property, value)),
            };
            set_component_field(world, entity, "sindri.ui.layout", "align", stored.into());
        }
        "gap" => {
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(
                world,
                entity,
                "sindri.ui.layout",
                "spacing",
                resolved.into(),
            );
        }
        "padding" => {
            let scale = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default()
                .scale_2d();
            let basis = scale[0].abs().min(scale[1].abs());
            let resolved = Length::parse(value)
                .and_then(|length| length.resolve(viewport, Some(basis)))
                .filter(|value| *value >= 0.0)
                .ok_or_else(|| invalid(id, property, value))?;
            set_resolved_padding(world, entity, resolved);
        }
        "font-size" | "line-height" | "letter-spacing" => {
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            let field = match property {
                "font-size" => "font_size",
                "line-height" => "line_height",
                _ => "letter_spacing",
            };
            set_component_field(world, entity, "sindri.ui.text", field, resolved.into());
        }
        "background" => {
            let resolved = color(value).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(
                world,
                entity,
                "sindri.ui.shape",
                "fill",
                serde_json::json!(resolved),
            );
        }
        "color" => {
            let resolved = color(value).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(
                world,
                entity,
                "sindri.ui.text",
                "color",
                serde_json::json!(resolved),
            );
        }
        "border-color" => {
            let resolved = color(value).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(
                world,
                entity,
                "sindri.ui.shape",
                "stroke",
                serde_json::json!(resolved),
            );
        }
        "border-width" | "border-radius" => {
            let resolved = size_fraction(world, entity, value, viewport)
                .ok_or_else(|| invalid(id, property, value))?;
            let field = if property == "border-width" {
                "stroke_width"
            } else {
                "corner_radius"
            };
            set_component_field(world, entity, "sindri.ui.shape", field, resolved.into());
        }
        "text-align" => {
            let value = value.trim();
            if !matches!(value, "left" | "center" | "right" | "justify") {
                return Err(invalid(id, property, value));
            }
            set_component_field(world, entity, "sindri.ui.text", "line_align", value.into());
        }
        "text-transform" => {
            let stored = match value.trim() {
                "none" => "as_written",
                "uppercase" => "upper",
                "lowercase" => "lower",
                _ => return Err(invalid(id, property, value)),
            };
            set_component_field(world, entity, "sindri.ui.text", "case", stored.into());
        }
        "font-weight" => {
            let bold = match value.trim() {
                "normal" | "400" => false,
                "bold" | "700" => true,
                _ => return Err(invalid(id, property, value)),
            };
            set_component_field(world, entity, "sindri.ui.text", "bold", bold.into());
        }
        // Unknown properties are ignored in this intentionally tiny POC. A real
        // language surface should diagnose them from a registered property table.
        _ => {}
    }
    Ok(())
}

fn set_component_field(
    world: &mut World,
    entity: EntityId,
    type_name: &str,
    field: &str,
    value: serde_json::Value,
) {
    let Some(data) = world.get_mut(entity) else {
        return;
    };
    let Some(payload) = data.components.get_mut(type_name) else {
        return;
    };
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object.insert(field.to_owned(), value);
}

fn set_resolved_padding(world: &mut World, entity: EntityId, padding: f32) {
    let Some(data) = world.get_mut(entity) else {
        return;
    };
    let payload = data
        .components
        .entry("weave.style".to_owned())
        .or_insert_with(|| serde_json::json!({}));
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object.insert(RESOLVED_PADDING_FIELD.to_owned(), padding.into());
}

fn resolved_padding(world: &World, entity: EntityId) -> Option<f32> {
    world
        .get(entity)?
        .components
        .get("weave.style")?
        .get(RESOLVED_PADDING_FIELD)?
        .as_f64()
        .map(|value| value as f32)
}

fn size_fraction(world: &World, entity: EntityId, value: &str, viewport: Viewport) -> Option<f32> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return Some(percent.trim().parse::<f32>().ok()? / 100.0);
    }
    if !value.ends_with("px") && !value.ends_with("vw") && !value.ends_with("vh") {
        return value.parse::<f32>().ok();
    }
    let absolute = length(value, viewport)?;
    let scale = world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default()
        .scale_2d();
    let shorter = scale[0].abs().min(scale[1].abs());
    (shorter > f32::EPSILON).then_some((absolute / shorter).max(0.0))
}

fn color(value: &str) -> Option<[f32; 4]> {
    match value.trim() {
        "transparent" => return Some([0.0; 4]),
        "black" => return Some([0.0, 0.0, 0.0, 1.0]),
        "white" => return Some([1.0; 4]),
        _ => {}
    }
    let hex = value.trim().strip_prefix('#')?;
    let bytes = match hex.len() {
        3 | 4 => {
            let mut channels = [255; 4];
            for (index, digit) in hex.as_bytes().iter().enumerate() {
                channels[index] = hex_digit(*digit)? * 17;
            }
            channels
        }
        6 | 8 => {
            let mut channels = [255; 4];
            for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                channels[index] = hex_digit(pair[0])? * 16 + hex_digit(pair[1])?;
            }
            channels
        }
        _ => return None,
    };
    Some([
        srgb_channel(bytes[0]),
        srgb_channel(bytes[1]),
        srgb_channel(bytes[2]),
        f32::from(bytes[3]) / 255.0,
    ])
}

fn srgb_channel(byte: u8) -> f32 {
    let encoded = f32::from(byte) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn length(value: &str, viewport: Viewport) -> Option<f32> {
    Length::parse(value)?.resolve(viewport, None)
}

fn anchor(value: &str) -> Option<&'static str> {
    match value.trim() {
        "center" => Some("center"),
        "top" => Some("top"),
        "bottom" => Some("bottom"),
        "left" => Some("left"),
        "right" => Some("right"),
        "top-left" | "top_left" => Some("top_left"),
        "top-right" | "top_right" => Some("top_right"),
        "bottom-left" | "bottom_left" => Some("bottom_left"),
        "bottom-right" | "bottom_right" => Some("bottom_right"),
        _ => None,
    }
}

fn invalid(entity: &str, property: &str, value: &str) -> ApplyError {
    ApplyError::InvalidValue {
        entity: entity.to_owned(),
        property: property.to_owned(),
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests;
