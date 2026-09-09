//! What the registry promises about registration, validation, and queries.

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

/// A component with an optional field, which is where a template drifts:
/// the struct gains one and the template still decodes without it.
#[derive(Debug, Deserialize, PartialEq)]
struct Armour {
    plates: u8,
    #[serde(default)]
    shine: f32,
}

impl SceneComponent for Armour {
    const TYPE_NAME: &'static str = "game.armour";
}

fn health_scene(payload: Value) -> SceneDocument {
    SceneDocument {
        entities: vec![SceneEntity {
            components: BTreeMap::from([(Health::TYPE_NAME.to_owned(), payload)]),
            ..SceneEntity::new(SceneEntityId::new("player").unwrap())
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

/// A field template is a second copy of the struct's field list, and a
/// second copy drifts. Serde knows the real list, so the drift is caught
/// where it is made rather than in a panel a release later — which is how
/// `sindri.ui.text` came to inspect as two rows for a seven-field
/// component, and `sindri.tilemap` to have no way to be isometric.
#[test]
fn a_template_that_does_not_match_the_component_is_refused() {
    let mut registry = ComponentSchemaRegistry::default();
    // The realistic drift: a field with a serde default is added to the
    // struct and nobody updates the template. Nothing else notices, because
    // the template still decodes — `serde_json` fills the missing field in.
    let missing = registry
        .register_with_fields::<Armour>("Armour", json!({ "plates": 0 }))
        .expect_err("a template that names fewer fields than the type has");
    assert!(
        matches!(&missing, ComponentRegistryError::TemplateMismatch { wrong, .. }
            if wrong.contains("shine")),
        "it says which field is missing: {missing}"
    );

    let extra = registry
        .register_with_fields::<Armour>("Armour", json!({ "plates": 0, "shine": 0.0, "polish": 1 }))
        .expect_err("a template naming a field the type does not have");
    assert!(
        matches!(&extra, ComponentRegistryError::TemplateMismatch { wrong, .. }
            if wrong.contains("polish")),
        "serde_json would ignore it in silence, so the registry does not: {extra}"
    );

    registry
        .register_with_fields::<Armour>("Armour", json!({ "plates": 0, "shine": 0.0 }))
        .expect("a template naming exactly the type's fields");
}

/// Fields and a fresh blank are different questions, and a type may answer
/// only the first.
#[test]
fn a_type_can_have_fields_and_no_default() {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register_with_fields::<Health>("Health", json!({ "current": 0, "maximum": 0 }))
        .unwrap();
    assert_eq!(
        registry.fields(Health::TYPE_NAME),
        Some(&json!({ "current": 0, "maximum": 0 })),
        "what the component consists of"
    );
    assert_eq!(
        registry.default_payload(Health::TYPE_NAME),
        None,
        "and nothing claiming a fresh one is valid"
    );
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

/// A described field has to exist.
///
/// The whole point of declaring meaning next to the component is that it can be
/// checked against it. A path naming nothing is the drift this is meant to
/// catch, so it fails where the registration is written.
#[test]
fn describing_a_field_the_component_does_not_have_is_refused() {
    // The fields are here so serde can report them: that list is what
    // the registry checks a template and its meanings against. Nothing
    // reads them back.
    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    struct Portrait {
        image: String,
        scale: f32,
    }
    impl SceneComponent for Portrait {
        const TYPE_NAME: &'static str = "game.portrait";
    }

    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register_with_fields::<Portrait>("Portrait", json!({ "image": "", "scale": 1.0 }))
        .unwrap();

    assert!(matches!(
        registry.describe::<Portrait>([("picture", FieldMeaning::Asset(AssetKind::Texture))]),
        Err(ComponentRegistryError::UnknownFieldPath { path, .. }) if path == "picture"
    ));
    registry
        .describe::<Portrait>([("image", FieldMeaning::Asset(AssetKind::Texture))])
        .expect("a field the component has is described");
    assert_eq!(
        registry.meaning("game.portrait", "image"),
        Some(&FieldMeaning::Asset(AssetKind::Texture))
    );
    assert_eq!(registry.meaning("game.portrait", "scale"), None);
}

/// One meaning covers every item of a list.
#[test]
fn a_described_list_item_answers_for_every_index() {
    // The fields are here so serde can report them: that list is what
    // the registry checks a template and its meanings against. Nothing
    // reads them back.
    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    struct Loadout {
        slots: Vec<Slot>,
    }
    // The fields are here so serde can report them: that list is what
    // the registry checks a template and its meanings against. Nothing
    // reads them back.
    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    struct Slot {
        icon: String,
    }
    impl SceneComponent for Loadout {
        const TYPE_NAME: &'static str = "game.loadout";
    }

    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register_with_fields::<Loadout>("Loadout", json!({ "slots": [{ "icon": "" }] }))
        .unwrap();
    registry
        .describe::<Loadout>([("slots[].icon", FieldMeaning::Asset(AssetKind::Texture))])
        .unwrap();

    for path in ["slots.0.icon", "slots.4.icon", "slots[].icon"] {
        assert_eq!(
            registry.meaning("game.loadout", path),
            Some(&FieldMeaning::Asset(AssetKind::Texture)),
            "{path} should be answered by the exemplar"
        );
    }
}

/// A component nothing has described says nothing, rather than guessing.
#[test]
fn a_component_without_a_template_cannot_be_described() {
    // The fields are here so serde can report them: that list is what
    // the registry checks a template and its meanings against. Nothing
    // reads them back.
    #[allow(dead_code)]
    #[derive(Debug, serde::Deserialize)]
    struct Mystery {
        anything: f32,
    }
    impl SceneComponent for Mystery {
        const TYPE_NAME: &'static str = "game.mystery";
    }

    let mut registry = ComponentSchemaRegistry::default();
    registry.register::<Mystery>("Mystery").unwrap();
    assert!(matches!(
        registry.describe::<Mystery>([("anything", FieldMeaning::Angle)]),
        Err(ComponentRegistryError::DescribedWithoutFields(_))
    ));
}
