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

use computed::ComputedStyle;

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
    let entities: Vec<(EntityId, String, Vec<String>, Vec<String>)> = world
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

    for (entity, id, classes, component_types) in entities {
        let classes: Vec<&str> = classes.iter().map(String::as_str).collect();
        let kinds: Vec<&str> = component_types.iter().map(String::as_str).collect();
        let computed = ComputedStyle::resolve(stylesheet, &id, &classes, &kinds, viewport);

        // Visual lengths such as border radius are relative to the final box,
        // so settle both axes before translating any decoration.
        for property in ["width", "height"] {
            if let Some(value) = computed.get(property) {
                apply_property(world, entity, &id, property, value, viewport)?;
            }
        }
        for (property, value) in computed.into_declarations() {
            if !matches!(property.as_str(), "width" | "height") {
                apply_property(world, entity, &id, &property, &value, viewport)?;
            }
        }
    }
    Ok(())
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
    let value = value.trim();
    if let Some(number) = value.strip_suffix("vw") {
        let percent = number.trim().parse::<f32>().ok()?;
        return Some((percent / 100.0) * 2.0 * viewport.width / viewport.height.max(1.0));
    }
    if let Some(number) = value.strip_suffix("vh") {
        let percent = number.trim().parse::<f32>().ok()?;
        return Some((percent / 100.0) * 2.0);
    }
    if let Some(number) = value.strip_suffix("px") {
        let pixels = number.trim().parse::<f32>().ok()?;
        return Some(pixels * 2.0 / viewport.height.max(1.0));
    }
    value.parse::<f32>().ok()
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
mod tests {
    use sindri_core::{SceneDocument, World};
    use weave::{Viewport, parse};

    use super::{PresentationWorld, srgb_channel};

    fn assert_color(actual: &serde_json::Value, expected: [f64; 4]) {
        let channels = actual.as_array().expect("color is an array");
        for (actual, expected) in channels.iter().zip(expected) {
            let actual = actual.as_f64().expect("channel is numeric");
            assert!((actual - expected).abs() < 0.000_01);
        }
    }

    fn assert_number(actual: &serde_json::Value, expected: f64) {
        let actual = actual.as_f64().expect("value is numeric");
        assert!((actual - expected).abs() < 0.000_01);
    }

    #[test]
    fn resolution_does_not_mutate_authored_world() {
        let document = SceneDocument::from_json(
            r#"{
                "format_version": 9,
                "metadata": { "name": "weave" },
                "entities": [{
                    "id": "panel",
                    "transform_3d": { "scale": [0.5, 0.5, 1.0] },
                    "components": {
                        "sindri.ui.image": { "texture": "sindri:white", "anchor": "center" }
                    }
                }]
            }"#,
        )
        .expect("scene parses");
        let source = World::from_scene(&document).expect("scene loads").world;
        let before = source.clone();
        let sheet =
            parse("#panel { width: 420px; } @media (max-width: 700px) { #panel { width: 90vw; } }")
                .expect("Weave parses");
        let styled = PresentationWorld::resolve(
            &source,
            &sheet,
            Viewport {
                width: 390.0,
                height: 844.0,
            },
        )
        .expect("styles resolve");

        let source_scale = source
            .entities()
            .next()
            .expect("source entity")
            .1
            .transform_3d
            .expect("transform")
            .scale[0];
        let styled_scale = styled
            .world()
            .entities()
            .next()
            .expect("styled entity")
            .1
            .transform_3d
            .expect("transform")
            .scale[0];
        let before_scale = before
            .entities()
            .next()
            .expect("before entity")
            .1
            .transform_3d
            .expect("transform")
            .scale[0];
        assert_eq!(source_scale, before_scale);
        assert_ne!(styled_scale, source_scale);
    }
    #[test]
    fn class_rules_style_shape_and_text_components() {
        let document = SceneDocument::from_json(
            r#"{
                "format_version": 9,
                "metadata": { "name": "weave-visuals" },
                "entities": [{
                    "id": "panel",
                    "transform_3d": { "scale": [0.5, 0.5, 1.0] },
                    "components": {
                        "weave.style": { "classes": ["card"] },
                        "sindri.ui.shape": {
                            "kind": "rect",
                            "fill": [0.0, 0.0, 0.0, 1.0],
                            "anchor": "center"
                        },
                        "sindri.ui.text": {
                            "text": "hello",
                            "font": "fonts/test.ttf",
                            "font_size": 0.05
                        }
                    }
                }]
            }"#,
        )
        .expect("scene parses");
        let source = World::from_scene(&document).expect("scene loads").world;
        let sheet = parse(
            r#"
                #panel { background: #abcdef; }
                .card {
                    width: 400px;
                    height: 200px;
                    background: #112233;
                    color: #f8fafc;
                    border-color: #445566;
                    border-width: 5%;
                    border-radius: 20%;
                    font-weight: 700;
                    text-transform: uppercase;
                    text-align: center;
                }
            "#,
        )
        .expect("Weave parses");

        let styled = PresentationWorld::resolve(
            &source,
            &sheet,
            Viewport {
                width: 1_200.0,
                height: 800.0,
            },
        )
        .expect("styles resolve");
        let (_, entity) = styled.world().entities().next().expect("styled entity");
        let transform = entity.transform_3d.expect("styled transform");
        assert_eq!(transform.scale[0], 1.0);
        assert_eq!(transform.scale[1], 0.5);

        let shape = entity
            .components
            .get("sindri.ui.shape")
            .expect("shape payload");
        assert_color(
            shape.get("fill").expect("fill"),
            [
                f64::from(srgb_channel(171)),
                f64::from(srgb_channel(205)),
                f64::from(srgb_channel(239)),
                1.0,
            ],
        );
        assert_color(
            shape.get("stroke").expect("stroke"),
            [
                f64::from(srgb_channel(68)),
                f64::from(srgb_channel(85)),
                f64::from(srgb_channel(102)),
                1.0,
            ],
        );
        assert_number(&shape["stroke_width"], 0.05);
        assert_number(&shape["corner_radius"], 0.2);

        let text = entity
            .components
            .get("sindri.ui.text")
            .expect("text payload");
        assert_color(
            text.get("color").expect("color"),
            [
                f64::from(srgb_channel(248)),
                f64::from(srgb_channel(250)),
                f64::from(srgb_channel(252)),
                1.0,
            ],
        );
        assert_eq!(text["bold"], true);
        assert_eq!(text["case"], "upper");
        assert_eq!(text["line_align"], "center");

        let source_entity = source.entities().next().expect("source entity").1;
        assert_eq!(source_entity.components["sindri.ui.shape"]["fill"][0], 0.0);
        assert!(
            source_entity.components["sindri.ui.text"]
                .get("bold")
                .is_none()
        );
    }
}
