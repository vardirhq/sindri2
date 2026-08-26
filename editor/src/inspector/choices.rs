//! Fields whose value is one of a few named things, and what picking one writes.
//!
//! A stored string is a text box unless something says otherwise, and for an
//! enum that is a trap: `perspective` is a camera projection and `perspectve`
//! is a scene that will not load. Worse, some of these strings decide which
//! *other* fields the component has, so typing the other name by hand produces
//! a payload missing half of itself, which the schema then refuses — a field
//! that looks editable and cannot be edited.
//!
//! Every entry here names a choice the engine already defines, and takes the
//! spellings from the engine's own list rather than repeating them. Where a
//! choice decides what else the component holds, [`choose`] writes those fields
//! too, so picking one is a whole edit rather than half of one.

use serde_json::{Map, Value};
use sindri_scene::{CameraComponent, RigidBodyKind, TileProjection, UiAnchor};

/// The field whose value decides what else a component holds.
///
/// One entry, because one built-in component is a tagged enum: a camera is a
/// perspective one or an orthographic one, and the two hold different fields.
/// Anything else here is a choice that decides only itself.
#[must_use]
pub fn variant_tag(type_name: &str) -> Option<&'static str> {
    (type_name == "sindri.camera").then_some("projection")
}

/// The registry's blank, rewritten for the variant this payload actually is.
///
/// A registry holds one blank per component, and for a tagged component that
/// blank is one variant of it — a fresh camera is a perspective one. Filling an
/// orthographic camera's missing fields from that blank would give it a
/// vertical field of view as well as a vertical size, which is the payload of
/// two cameras. So the blank is put through the same [`choose`] the author's
/// own switch goes through, and comes out describing the same variant they are
/// looking at.
#[must_use]
pub fn blank_for(type_name: &str, defaults: &Value, payload: &Value) -> Value {
    let mut blank = defaults.clone();
    if let Some(tag) = variant_tag(type_name)
        && let Some(chosen) = payload.get(tag).and_then(Value::as_str)
    {
        choose(type_name, tag, chosen, &mut blank);
    }
    blank
}

/// The values this field may hold, if it is a choice among named ones.
#[must_use]
pub fn choices(type_name: &str, key: &str) -> Option<Vec<&'static str>> {
    match (type_name, key) {
        ("sindri.camera", "projection") => Some(CameraComponent::PROJECTIONS.to_vec()),
        ("sindri.ui.image" | "sindri.ui.text", "anchor") => {
            Some(UiAnchor::ALL.iter().map(|anchor| anchor.as_str()).collect())
        }
        ("sindri.tilemap", "projection") => Some(
            TileProjection::ALL
                .iter()
                .map(|projection| projection.as_str())
                .collect(),
        ),
        ("sindri.physics2d.rigid_body", "kind") => Some(
            RigidBodyKind::ALL
                .iter()
                .map(|kind| kind.as_str())
                .collect(),
        ),
        _ => None,
    }
}

/// Writes a chosen value, along with whatever else the choice decides.
///
/// A camera's projection is the one case that decides more than itself: the
/// two projections are two shapes of payload, sharing near and far and
/// differing in what they frame with. Switching keeps the planes, drops the
/// field belonging to the projection being left, and writes the one the
/// arriving projection needs — because a camera whose tag says orthographic
/// and whose fields say perspective is not a camera the engine will load.
pub fn choose(type_name: &str, key: &str, chosen: &str, payload: &mut Value) {
    let Some(fields) = payload.as_object_mut() else {
        return;
    };
    fields.insert(key.to_owned(), Value::String(chosen.to_owned()));
    if (type_name, key) == ("sindri.camera", "projection") {
        camera_projection(chosen, fields);
    }
}

fn camera_projection(chosen: &str, fields: &mut Map<String, Value>) {
    const FOV: &str = "vertical_fov_degrees";
    const SIZE: &str = "vertical_size";
    let (arriving, leaving, default) = if chosen == CameraComponent::PROJECTIONS[1] {
        (SIZE, FOV, CameraComponent::DEFAULT_VERTICAL_SIZE)
    } else {
        (FOV, SIZE, CameraComponent::DEFAULT_VERTICAL_FOV_DEGREES)
    };
    fields.remove(leaving);
    fields
        .entry(arriving.to_owned())
        .or_insert_with(|| Value::from(default));
    // The planes are shared, so a camera that had them keeps them and one that
    // did not gains the same pair a fresh camera starts with.
    fields
        .entry("near".to_owned())
        .or_insert_with(|| Value::from(CameraComponent::DEFAULT_NEAR));
    fields
        .entry("far".to_owned())
        .or_insert_with(|| Value::from(CameraComponent::DEFAULT_FAR));
}

#[cfg(test)]
mod tests {
    use super::{choices, choose};
    use serde_json::json;

    #[test]
    fn an_enum_field_offers_the_engines_own_spellings() {
        assert_eq!(
            choices("sindri.camera", "projection"),
            Some(vec!["perspective", "orthographic"])
        );
        assert_eq!(
            choices("sindri.ui.image", "anchor").map(|all| all.len()),
            Some(9)
        );
        assert_eq!(choices("sindri.sprite", "texture"), None);
        assert_eq!(choices("game.health", "hp"), None);
    }

    /// The camera's tag decides which other fields it has, so switching it is
    /// not a change of one string. Typing the other name into a text box left
    /// a payload the schema refused, which is a field that looks editable and
    /// is not.
    #[test]
    fn switching_a_cameras_projection_writes_the_fields_that_projection_has() {
        let mut camera = json!({
            "projection": "perspective",
            "vertical_fov_degrees": 45.0,
            "near": 0.2,
            "far": 80.0
        });
        choose("sindri.camera", "projection", "orthographic", &mut camera);
        assert_eq!(camera["projection"], json!("orthographic"));
        assert!(camera.get("vertical_fov_degrees").is_none());
        assert!(camera["vertical_size"].as_f64().is_some());
        assert_eq!(camera["near"], json!(0.2), "the planes are shared");
        assert_eq!(camera["far"], json!(80.0));

        choose("sindri.camera", "projection", "perspective", &mut camera);
        assert!(camera.get("vertical_size").is_none());
        assert!(camera["vertical_fov_degrees"].as_f64().is_some());
    }

    /// A payload that already holds the arriving field keeps what it holds:
    /// switching away and back must not overwrite an authored number with a
    /// default.
    #[test]
    fn switching_back_keeps_what_was_already_there() {
        let mut camera = json!({
            "projection": "orthographic",
            "vertical_size": 12.0,
            "vertical_fov_degrees": 30.0,
            "near": 0.1,
            "far": 100.0
        });
        choose("sindri.camera", "projection", "perspective", &mut camera);
        assert_eq!(camera["vertical_fov_degrees"], json!(30.0));
    }

    /// A blank describing the other variant would fill an orthographic camera
    /// with a field of view, and the panel would show one camera's worth of
    /// rows plus another's.
    #[test]
    fn the_blank_describes_the_variant_the_payload_is() {
        let defaults = json!({
            "projection": "perspective",
            "vertical_fov_degrees": 60.0,
            "near": 0.1,
            "far": 100.0
        });
        let orthographic = json!({ "projection": "orthographic", "vertical_size": 4.0 });
        let blank = super::blank_for("sindri.camera", &defaults, &orthographic);
        assert!(blank.get("vertical_fov_degrees").is_none());
        assert!(blank["vertical_size"].as_f64().is_some());

        let perspective = json!({ "projection": "perspective" });
        assert_eq!(
            super::blank_for("sindri.camera", &defaults, &perspective),
            defaults,
            "and the variant the blank already describes is left alone"
        );
    }

    #[test]
    fn a_component_with_no_variants_keeps_its_blank() {
        let defaults = json!({ "texture": "a.png", "layer": 0 });
        assert_eq!(
            super::blank_for("sindri.sprite", &defaults, &json!({ "texture": "b.png" })),
            defaults
        );
    }

    #[test]
    fn a_plain_enum_writes_only_itself() {
        let mut image = json!({ "texture": "a.png", "anchor": "center" });
        choose("sindri.ui.image", "anchor", "top_left", &mut image);
        assert_eq!(
            image,
            json!({ "texture": "a.png", "anchor": "top_left" }),
            "nothing else about the element changed"
        );
    }
}
