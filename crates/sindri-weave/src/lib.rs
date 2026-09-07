//! Isolated bridge between the Weave language and Sindri UI.
//!
//! The proof of concept deliberately does not add Weave concepts to the engine.
//! It clones the authored [`World`], translates matching Weave declarations into
//! ordinary Sindri transforms/component payloads on that clone, and lets the
//! existing scene/render/input pipeline consume the result normally.

use sindri_core::{EntityId, Transform3D, World};
use thiserror::Error;
use weave::{Stylesheet, Viewport};

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
    let entities: Vec<(EntityId, String, Vec<String>)> = world
        .entities()
        .filter_map(|(entity, data)| {
            let id = data.source_id.as_ref()?.as_str().to_owned();
            let component_types = data.components.keys().cloned().collect();
            Some((entity, id, component_types))
        })
        .collect();

    for (entity, id, component_types) in entities {
        let kinds: Vec<&str> = component_types.iter().map(String::as_str).collect();
        for rule in stylesheet
            .rules
            .iter()
            .filter(|rule| rule.applies(&id, &kinds, viewport))
        {
            for (property, value) in &rule.declarations {
                apply_property(world, entity, &id, property, value, viewport)?;
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
                    object.insert("anchor".to_owned(), serde_json::Value::String(stored.to_owned()));
                }
            }
        }
        "direction" => {
            if !matches!(value.trim(), "row" | "column") {
                return Err(invalid(id, property, value));
            }
            set_component_field(world, entity, "sindri.ui.layout", "direction", value.trim().into());
        }
        "gap" => {
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(world, entity, "sindri.ui.layout", "spacing", resolved.into());
        }
        "font-size" => {
            let resolved = length(value, viewport).ok_or_else(|| invalid(id, property, value))?;
            set_component_field(world, entity, "sindri.ui.text", "font_size", resolved.into());
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

    use super::PresentationWorld;

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
        let sheet = parse("#panel { width: 420px; } @media (max-width: 700px) { #panel { width: 90vw; } }")
            .expect("Weave parses");
        let styled = PresentationWorld::resolve(
            &source,
            &sheet,
            Viewport { width: 390.0, height: 844.0 },
        )
        .expect("styles resolve");

        let source_scale = source.entities().next().expect("source entity").1.transform_3d.expect("transform").scale[0];
        let styled_scale = styled.world().entities().next().expect("styled entity").1.transform_3d.expect("transform").scale[0];
        let before_scale = before.entities().next().expect("before entity").1.transform_3d.expect("transform").scale[0];
        assert_eq!(source_scale, before_scale);
        assert_ne!(styled_scale, source_scale);
    }
}
