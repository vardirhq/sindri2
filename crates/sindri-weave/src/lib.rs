use sindri_core::{ComponentSchemaRegistry, SceneEntityId, Transform3D, World};
use sindri_scene::{UiAnchor, UiImageComponent, UiLayoutComponent, UiTextComponent};
use thiserror::Error;
use weave::{Stylesheet, Viewport};

#[derive(Debug, Error)]
pub enum WeaveApplyError {
    #[error("component query failed: {0}")]
    Components(#[from] sindri_core::ComponentRegistryError),
    #[error("invalid value `{value}` for `{property}` on `{entity}`")]
    InvalidValue {
        entity: String,
        property: String,
        value: String,
    },
}

/// A disposable presentation copy of a game world.
///
/// The authored world is never mutated. The POC deliberately translates Weave
/// into existing Sindri scene state so no foundational engine crate needs to
/// know the language exists.
#[derive(Clone, Debug)]
pub struct PresentationWorld {
    world: World,
}

impl PresentationWorld {
    pub fn resolve(
        source: &World,
        components: &ComponentSchemaRegistry,
        stylesheet: &Stylesheet,
        viewport: Viewport,
    ) -> Result<Self, WeaveApplyError> {
        let mut world = source.clone();
        apply(&mut world, components, stylesheet, viewport)?;
        Ok(Self { world })
    }

    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }
}

fn apply(
    world: &mut World,
    components: &ComponentSchemaRegistry,
    stylesheet: &Stylesheet,
    viewport: Viewport,
) -> Result<(), WeaveApplyError> {
    let entities: Vec<_> = world
        .entities()
        .map(|(entity, data)| {
            let id = data
                .scene_id
                .as_ref()
                .map(SceneEntityId::as_str)
                .unwrap_or("")
                .to_owned();
            (entity, id)
        })
        .collect();

    for (entity, id) in entities {
        if id.is_empty() {
            continue;
        }
        let mut kinds = Vec::new();
        if components.get::<UiImageComponent>(world, entity)?.is_some() {
            kinds.push(UiImageComponent::TYPE_NAME);
        }
        if components.get::<UiTextComponent>(world, entity)?.is_some() {
            kinds.push(UiTextComponent::TYPE_NAME);
        }
        if components.get::<UiLayoutComponent>(world, entity)?.is_some() {
            kinds.push(UiLayoutComponent::TYPE_NAME);
        }

        for rule in stylesheet
            .rules
            .iter()
            .filter(|rule| rule.applies(&id, &kinds, viewport))
        {
            for (property, value) in &rule.declarations {
                apply_property(world, components, entity, &id, property, value, viewport)?;
            }
        }
    }
    Ok(())
}

fn apply_property(
    world: &mut World,
    components: &ComponentSchemaRegistry,
    entity: sindri_core::EntityId,
    id: &str,
    property: &str,
    value: &str,
    viewport: Viewport,
) -> Result<(), WeaveApplyError> {
    match property {
        "width" | "height" => {
            let axis = usize::from(property == "height");
            let size = length(value, viewport, axis).ok_or_else(|| invalid(id, property, value))?;
            let data = world.get_mut(entity).expect("entity came from this world");
            let transform = data.transform_3d.get_or_insert_with(Transform3D::default);
            transform.scale[axis] = size;
        }
        "x" | "y" => {
            let axis = usize::from(property == "y");
            let offset = length(value, viewport, axis).ok_or_else(|| invalid(id, property, value))?;
            let data = world.get_mut(entity).expect("entity came from this world");
            let transform = data.transform_3d.get_or_insert_with(Transform3D::default);
            transform.position[axis] = offset;
        }
        "direction" | "gap" => {
            if let Some(mut layout) = components.get::<UiLayoutComponent>(world, entity)? {
                if property == "direction" {
                    layout.direction = match value.trim() {
                        "row" => sindri_scene::UiDirection::Row,
                        "column" => sindri_scene::UiDirection::Column,
                        _ => return Err(invalid(id, property, value)),
                    };
                } else {
                    layout.spacing = length(value, viewport, 1)
                        .ok_or_else(|| invalid(id, property, value))?;
                }
                components.set(world, entity, layout)?;
            }
        }
        "anchor" => {
            let anchor = parse_anchor(value).ok_or_else(|| invalid(id, property, value))?;
            if let Some(mut image) = components.get::<UiImageComponent>(world, entity)? {
                image.anchor = anchor;
                components.set(world, entity, image)?;
            }
            if let Some(mut text) = components.get::<UiTextComponent>(world, entity)? {
                text.anchor = anchor;
                components.set(world, entity, text)?;
            }
        }
        "font-size" => {
            if let Some(mut text) = components.get::<UiTextComponent>(world, entity)? {
                text.font_size = length(value, viewport, 1)
                    .ok_or_else(|| invalid(id, property, value))?;
                components.set(world, entity, text)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn invalid(entity: &str, property: &str, value: &str) -> WeaveApplyError {
    WeaveApplyError::InvalidValue {
        entity: entity.to_owned(),
        property: property.to_owned(),
        value: value.to_owned(),
    }
}

fn length(value: &str, viewport: Viewport, axis: usize) -> Option<f32> {
    let value = value.trim();
    if let Some(number) = value.strip_suffix("vw") {
        return number.trim().parse::<f32>().ok().map(|n| {
            // Sindri's overlay is two units tall. Horizontal units are scaled by
            // aspect, so a percentage of viewport width converts through height.
            (n / 100.0) * 2.0 * viewport.width / viewport.height.max(1.0)
        });
    }
    if let Some(number) = value.strip_suffix("vh") {
        return number.trim().parse::<f32>().ok().map(|n| (n / 100.0) * 2.0);
    }
    if let Some(number) = value.strip_suffix("px") {
        return number.trim().parse::<f32>().ok().map(|n| {
            let _ = axis;
            n * 2.0 / viewport.height.max(1.0)
        });
    }
    value.parse().ok()
}

fn parse_anchor(value: &str) -> Option<UiAnchor> {
    match value.trim() {
        "center" => Some(UiAnchor::Center),
        "top" => Some(UiAnchor::Top),
        "bottom" => Some(UiAnchor::Bottom),
        "left" => Some(UiAnchor::Left),
        "right" => Some(UiAnchor::Right),
        "top-left" => Some(UiAnchor::TopLeft),
        "top-right" => Some(UiAnchor::TopRight),
        "bottom-left" => Some(UiAnchor::BottomLeft),
        "bottom-right" => Some(UiAnchor::BottomRight),
        _ => None,
    }
}
