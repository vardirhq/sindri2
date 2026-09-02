use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::{EntityId, SceneDocument, SceneEntityId, SceneError, World};

mod fields;
#[cfg(test)]
mod tests;

use fields::declared_fields;

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
    /// Every field this component has, at the value it takes when nobody has
    /// said otherwise.
    ///
    /// This is the answer to "what does this component consist of", and it is
    /// deliberately a different question from "what is a valid fresh one". An
    /// editor draws a component by filling this out with whatever the instance
    /// stored, so two of one component show the same rows whether or not each
    /// wrote every field down.
    ///
    /// `None` only for a type registered by someone who did not say — a game's
    /// own component, say. Such a type is still readable and editable; its
    /// panel just shows the fields its payload happens to carry, which is all
    /// anything knows about it.
    fields: Option<Value>,
    /// The payload a freshly added component of this type starts as.
    ///
    /// `None` for a type that has no sensible blank — one naming an asset it
    /// cannot invent, say. Such a type stays readable and editable and simply
    /// cannot be *added* by a button, which is honest: a button that adds a
    /// component the engine will then reject is worse than no button.
    ///
    /// A type can have fields and no default. `sindri.ui.text` is the case
    /// that made the distinction necessary: it has seven fields and no honest
    /// blank, because there is no font the engine can invent. Conflating the
    /// two meant its panel showed two rows where the same component authored by
    /// hand showed seven.
    default_payload: Option<Value>,
}

#[derive(Clone, Debug, Default)]
pub struct ComponentSchemaRegistry {
    registrations: BTreeMap<String, ComponentRegistration>,
}

impl ComponentSchemaRegistry {
    /// Registers a component whose fields nothing has described.
    ///
    /// Readable, editable, and not creatable, and its panel shows only the
    /// fields its payload carries. For a built-in type prefer
    /// [`Self::register_with_fields`], which is what stops one added by a
    /// button from being a different-looking component than the same one
    /// authored by hand.
    pub fn register<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
    ) -> Result<(), ComponentRegistryError> {
        self.register_component::<T>(display_name, None, None)
    }

    /// Registers a component along with every field it has, but no fresh
    /// blank.
    ///
    /// For a type that cannot honestly be created without something only the
    /// host can supply — a font, a sheet, the ID of another entity. The field
    /// template still says what the component consists of, so it inspects like
    /// any other; it just cannot be added by a button until whoever owns the
    /// project completes it.
    pub fn register_with_fields<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
        fields: Value,
    ) -> Result<(), ComponentRegistryError> {
        Self::check_template::<T>(&fields)?;
        self.register_component::<T>(display_name, Some(fields), None)
    }

    /// Registers a component along with the payload a fresh one starts as.
    ///
    /// The default is checked here rather than when someone clicks Add, so a
    /// default that does not decode fails at startup — where it is a build
    /// error someone sees immediately — instead of producing an entity the
    /// scene will not load.
    ///
    /// A type with an honest default has no separate field template: the
    /// default already names every field, or it would not be one.
    pub fn register_with_default<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
        default_payload: Value,
    ) -> Result<(), ComponentRegistryError> {
        Self::check_template::<T>(&default_payload)?;
        self.register_component::<T>(
            display_name,
            Some(default_payload.clone()),
            Some(default_payload),
        )
    }

    /// A template has to decode as the component, be an object, and name every
    /// field the component actually has.
    ///
    /// The last check is the one that matters. A hand-written template is a
    /// second copy of the struct's field list, and a second copy drifts: the
    /// editor's `sindri.ui.text` showed two rows for a component with seven
    /// fields because nobody noticed the two lists had parted. Serde knows the
    /// real list ([`declared_fields`]), so the drift is a registration error
    /// rather than something to discover in a screenshot a release later.
    ///
    /// Both directions are checked. A missing name means a field nothing can
    /// edit; an extra one means a row for a field the component does not have,
    /// which `serde_json` would otherwise ignore in silence.
    fn check_template<T: SceneComponent>(template: &Value) -> Result<(), ComponentRegistryError> {
        let Some(object) = template.as_object() else {
            return Err(ComponentRegistryError::InvalidFields(T::TYPE_NAME));
        };
        validate_payload::<T>(template).map_err(|source| {
            ComponentRegistryError::InvalidDefault {
                type_name: T::TYPE_NAME.to_owned(),
                source,
            }
        })?;
        let Some(declared) = declared_fields::<T>() else {
            return Ok(());
        };
        let mut wrong: Vec<String> = declared
            .iter()
            .filter(|field| !object.contains_key(**field))
            .map(|field| format!("missing '{field}'"))
            .collect();
        wrong.extend(
            object
                .keys()
                .filter(|key| !declared.contains(&key.as_str()))
                .map(|key| format!("unknown '{key}'")),
        );
        if wrong.is_empty() {
            Ok(())
        } else {
            Err(ComponentRegistryError::TemplateMismatch {
                type_name: T::TYPE_NAME,
                wrong: wrong.join(", "),
            })
        }
    }

    fn register_component<T: SceneComponent>(
        &mut self,
        display_name: impl Into<String>,
        fields: Option<Value>,
        default_payload: Option<Value>,
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
                fields,
                default_payload,
            },
        );
        Ok(())
    }

    /// Every field this component has, at its unstated value, or `None` for a
    /// type nothing has described.
    ///
    /// What an editor draws a component *from*. A panel that used
    /// [`Self::default_payload`] for this showed only the fields a creatable
    /// component happened to start with, and nothing at all for a type that
    /// cannot be created.
    pub fn fields(&self, type_name: &str) -> Option<&Value> {
        self.registrations.get(type_name)?.fields.as_ref()
    }

    /// The payload a freshly added component of this type starts as, or `None`
    /// for one that cannot be created blank.
    pub fn default_payload(&self, type_name: &str) -> Option<&Value> {
        self.registrations.get(type_name)?.default_payload.as_ref()
    }

    /// Whether a payload decodes as this component.
    ///
    /// The editor's guard on an edit: a payload is written back exactly as it
    /// is stored, so an edit that stops it decoding would produce a scene the
    /// engine refuses to load. Checking here means the refusal happens while
    /// the author is still looking at the field.
    pub fn validate_payload(
        &self,
        type_name: &str,
        payload: &Value,
    ) -> Result<(), ComponentRegistryError> {
        let Some(registration) = self.registrations.get(type_name) else {
            // An unregistered type is one the policy preserves rather than
            // understands. Nothing can be said about its shape, so nothing is.
            return Ok(());
        };
        (registration.validate)(payload).map_err(|source| ComponentRegistryError::InvalidPayload {
            entity: "<edited>".to_owned(),
            type_name: type_name.to_owned(),
            source,
        })
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

    /// Every *active* entity carrying `T`, decoded.
    ///
    /// Active is the filter, not a caller's afterthought: this is what
    /// rendering, stepping, scripting and picking all ask, and an entity that
    /// has been switched off — or whose parent has — takes no part in any of
    /// them. Leaving it to each caller would mean six places to forget it, and
    /// the ones that forgot would draw something nothing can click or step
    /// something nobody can see.
    pub fn query<T: SceneComponent>(
        &self,
        world: &World,
    ) -> Result<Vec<(EntityId, T)>, ComponentRegistryError> {
        self.require_registered(T::TYPE_NAME)?;
        world
            .entities()
            .filter(|(entity_id, _)| world.is_active(*entity_id))
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

    /// The `T` one entity carries, decoded, or `None`.
    ///
    /// Filtered on active for the same reason [`Self::query`] is: an entity
    /// that has been switched off — or whose parent has — takes no part in
    /// rendering, stepping, scripting or picking, and a caller asking about one
    /// entity is asking the same question as a caller asking about all of them.
    pub fn get<T: SceneComponent>(
        &self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<T>, ComponentRegistryError> {
        self.require_registered(T::TYPE_NAME)?;
        if !world.is_active(entity) {
            return Ok(None);
        }
        let Some(data) = world.get(entity) else {
            return Ok(None);
        };
        let Some(payload) = data.components.get(T::TYPE_NAME) else {
            return Ok(None);
        };
        serde_json::from_value(payload.clone())
            .map(Some)
            .map_err(|source| ComponentRegistryError::InvalidPayload {
                entity: data.source_id.as_ref().map_or_else(
                    || format!("{entity:?}"),
                    |source_id| source_id.as_str().to_owned(),
                ),
                type_name: T::TYPE_NAME.to_owned(),
                source,
            })
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
    #[error("the field template for component type '{0}' is not an object")]
    InvalidFields(&'static str),
    #[error("the field template for component type '{type_name}' does not match it: {wrong}")]
    TemplateMismatch {
        type_name: &'static str,
        wrong: String,
    },
    #[error("the payload registered for component type '{type_name}' does not decode as it")]
    InvalidDefault {
        type_name: String,
        #[source]
        source: serde_json::Error,
    },
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
