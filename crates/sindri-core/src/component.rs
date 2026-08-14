use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::{EntityId, SceneDocument, SceneEntityId, SceneError, World};

pub trait SceneComponent: DeserializeOwned {
    const TYPE_NAME: &'static str;
    const SCHEMA_VERSION: u32 = 1;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentMetadata {
    pub type_name: String,
    pub display_name: String,
    pub schema_version: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnknownComponentPolicy {
    #[default]
    Preserve,
    Reject,
}

type ComponentValidator = fn(&Value) -> Result<(), serde_json::Error>;

#[derive(Clone, Debug)]
struct ComponentRegistration {
    metadata: ComponentMetadata,
    validate: ComponentValidator,
}

#[derive(Clone, Debug, Default)]
pub struct ComponentSchemaRegistry {
    registrations: BTreeMap<String, ComponentRegistration>,
}

impl ComponentSchemaRegistry {
    pub fn register<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
    ) -> Result<(), ComponentRegistryError> {
        validate_type_name(T::TYPE_NAME)?;
        let display_name = display_name.into();
        if display_name.trim().is_empty() {
            return Err(ComponentRegistryError::EmptyDisplayName);
        }
        if self.registrations.contains_key(T::TYPE_NAME) {
            return Err(ComponentRegistryError::AlreadyRegistered(
                T::TYPE_NAME.to_owned(),
            ));
        }
        self.registrations.insert(
            T::TYPE_NAME.to_owned(),
            ComponentRegistration {
                metadata: ComponentMetadata {
                    type_name: T::TYPE_NAME.to_owned(),
                    display_name,
                    schema_version: T::SCHEMA_VERSION,
                },
                validate: validate_payload::<T>,
            },
        );
        Ok(())
    }

    pub fn metadata(&self, type_name: &str) -> Option<&ComponentMetadata> {
        self.registrations
            .get(type_name)
            .map(|registration| &registration.metadata)
    }

    pub fn registered_components(&self) -> impl Iterator<Item = &ComponentMetadata> {
        self.registrations
            .values()
            .map(|registration| &registration.metadata)
    }

    pub fn validate_scene(
        &self,
        scene: &SceneDocument,
        unknown_policy: UnknownComponentPolicy,
    ) -> Result<(), ComponentRegistryError> {
        scene.validate()?;
        for entity in &scene.entities {
            for (type_name, payload) in &entity.components {
                let Some(registration) = self.registrations.get(type_name) else {
                    if unknown_policy == UnknownComponentPolicy::Reject {
                        return Err(ComponentRegistryError::UnknownComponent {
                            entity: entity.id.clone(),
                            type_name: type_name.clone(),
                        });
                    }
                    continue;
                };
                (registration.validate)(payload).map_err(|source| {
                    ComponentRegistryError::InvalidPayload {
                        entity: entity.id.as_str().to_owned(),
                        type_name: type_name.clone(),
                        source,
                    }
                })?;
            }
        }
        Ok(())
    }

    pub fn decode<T: SceneComponent>(&self, payload: &Value) -> Result<T, ComponentRegistryError> {
        self.require_registered(T::TYPE_NAME)?;
        serde_json::from_value(payload.clone()).map_err(|source| {
            ComponentRegistryError::InvalidPayload {
                entity: "<detached>".to_owned(),
                type_name: T::TYPE_NAME.to_owned(),
                source,
            }
        })
    }

    pub fn query<T: SceneComponent>(
        &self,
        world: &World,
    ) -> Result<Vec<(EntityId, T)>, ComponentRegistryError> {
        self.require_registered(T::TYPE_NAME)?;
        world
            .entities()
            .filter_map(|(entity_id, entity)| {
                entity
                    .components
                    .get(T::TYPE_NAME)
                    .map(|payload| (entity_id, entity, payload))
            })
            .map(|(entity_id, entity, payload)| {
                serde_json::from_value(payload.clone())
                    .map(|component| (entity_id, component))
                    .map_err(|source| ComponentRegistryError::InvalidPayload {
                        entity: entity.source_id.as_ref().map_or_else(
                            || format!("{entity_id:?}"),
                            |source_id| source_id.as_str().to_owned(),
                        ),
                        type_name: T::TYPE_NAME.to_owned(),
                        source,
                    })
            })
            .collect()
    }

    fn require_registered(&self, type_name: &'static str) -> Result<(), ComponentRegistryError> {
        self.registrations
            .contains_key(type_name)
            .then_some(())
            .ok_or(ComponentRegistryError::NotRegistered(type_name))
    }
}

fn validate_payload<T: SceneComponent>(payload: &Value) -> Result<(), serde_json::Error> {
    serde_json::from_value::<T>(payload.clone()).map(|_| ())
}

fn validate_type_name(type_name: &'static str) -> Result<(), ComponentRegistryError> {
    let valid = !type_name.is_empty()
        && !type_name.split('.').any(str::is_empty)
        && type_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        });
    if valid {
        Ok(())
    } else {
        Err(ComponentRegistryError::InvalidTypeName(type_name))
    }
}

#[derive(Debug, Error)]
pub enum ComponentRegistryError {
    #[error("component type name '{0}' is invalid")]
    InvalidTypeName(&'static str),
    #[error("component display names cannot be empty")]
    EmptyDisplayName,
    #[error("component type '{0}' is already registered")]
    AlreadyRegistered(String),
    #[error("component type '{0}' is not registered")]
    NotRegistered(&'static str),
    #[error("entity {entity:?} contains unknown component type '{type_name}'")]
    UnknownComponent {
        entity: SceneEntityId,
        type_name: String,
    },
    #[error("entity '{entity}' has invalid '{type_name}' component data")]
    InvalidPayload {
        entity: String,
        type_name: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    InvalidScene(#[from] SceneError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::SceneEntity;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Health {
        current: u16,
        maximum: u16,
    }

    impl SceneComponent for Health {
        const TYPE_NAME: &'static str = "game.health";
    }

    fn health_scene(payload: Value) -> SceneDocument {
        SceneDocument {
            entities: vec![SceneEntity {
                id: SceneEntityId::new("player").unwrap(),
                name: None,
                parent: None,
                transform_2d: None,
                transform_3d: None,
                components: BTreeMap::from([(Health::TYPE_NAME.to_owned(), payload)]),
            }],
            ..SceneDocument::default()
        }
    }

    #[test]
    fn registered_components_validate_and_query_as_typed_values() {
        let mut registry = ComponentSchemaRegistry::default();
        registry.register::<Health>("Health").unwrap();
        assert_eq!(
            registry.metadata(Health::TYPE_NAME),
            Some(&ComponentMetadata {
                type_name: Health::TYPE_NAME.to_owned(),
                display_name: "Health".to_owned(),
                schema_version: 1,
            })
        );
        let scene = health_scene(json!({ "current": 8, "maximum": 10 }));
        registry
            .validate_scene(&scene, UnknownComponentPolicy::Reject)
            .unwrap();
        let loaded = World::from_scene(&scene).unwrap();

        let components = registry.query::<Health>(&loaded.world).unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(
            components[0].1,
            Health {
                current: 8,
                maximum: 10,
            }
        );
    }

    #[test]
    fn unknown_components_can_be_preserved_or_rejected() {
        let scene = health_scene(json!({ "current": 8, "maximum": 10 }));
        let registry = ComponentSchemaRegistry::default();
        registry
            .validate_scene(&scene, UnknownComponentPolicy::Preserve)
            .unwrap();
        assert!(matches!(
            registry.validate_scene(&scene, UnknownComponentPolicy::Reject),
            Err(ComponentRegistryError::UnknownComponent { .. })
        ));
    }

    #[test]
    fn registered_schema_rejects_invalid_payloads() {
        let mut registry = ComponentSchemaRegistry::default();
        registry.register::<Health>("Health").unwrap();
        let scene = health_scene(json!({ "current": "many", "maximum": 10 }));
        assert!(matches!(
            registry.validate_scene(&scene, UnknownComponentPolicy::Reject),
            Err(ComponentRegistryError::InvalidPayload { .. })
        ));
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        let mut registry = ComponentSchemaRegistry::default();
        registry.register::<Health>("Health").unwrap();
        assert!(matches!(
            registry.register::<Health>("Health"),
            Err(ComponentRegistryError::AlreadyRegistered(_))
        ));
    }
}
