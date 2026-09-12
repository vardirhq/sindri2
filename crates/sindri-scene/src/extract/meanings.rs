//! What each built-in component's fields are *for*.
//!
//! The field template beside this says a sprite has a `texture` and that it
//! holds a string. It cannot say the string names a texture in the project, so
//! the editor guessed from the field's name — and a guess from a name is wrong
//! in both directions: `(_, "clip")` offered the project's audio to any
//! component with a `clip` field, including a game's own, while a field the
//! table had never heard of was a text box in silence.
//!
//! Declaring it here puts the knowledge beside the component it describes,
//! where the registry checks every path against the template. A renamed field
//! leaves its meaning naming nothing, and that is a startup error rather than a
//! picker that quietly stopped appearing.
//!
//! Every choice takes its spellings from the engine's own list. A list of names
//! retyped here would be a second copy of an enum, and the drift this module
//! exists to prevent is exactly that.

use serde_json::json;
use sindri_core::{AssetKind, ComponentSchemaRegistry, FieldMeaning};
use sindri_physics::{ColliderShape2d, RigidBodyKind};

use crate::animation::SpriteAnimationComponent;
use crate::audio::AudioSourceComponent;
use crate::components::{
    CameraComponent, GridOccupantComponent, MeshComponent, ShapeComponent, SpriteComponent,
    TileGridComponent, TileProjection, TileVolumeComponent, TilemapComponent, UiImageComponent,
    UiShapeBlend, UiShapeComponent, UiShapeKind, UiTextComponent,
};
use crate::components::{UiAnchor, UiTextCase, UiTextLineAlign, UiTextWrap};
use crate::effects::EffectBurstComponent;
use crate::physics::{Collider2dComponent, RigidBody2dComponent};
use crate::screen_ui::{UiAlign, UiDirection, UiJustify, UiLayoutComponent};

use super::SceneExtractError;

/// A colour, wherever one is stored.
///
/// Worth stating rather than inferring: the editor's old test was "four numbers
/// under a key called `tint`", and four numbers is also a UV rect, a set of
/// bounds, and a quaternion.
const COLOUR: FieldMeaning = FieldMeaning::Colour;

fn texture() -> FieldMeaning {
    FieldMeaning::Asset(AssetKind::Texture)
}

fn anchors() -> FieldMeaning {
    FieldMeaning::choice(UiAnchor::ALL.into_iter().map(UiAnchor::as_str))
}

fn shape_kinds() -> FieldMeaning {
    FieldMeaning::choice(UiShapeKind::ALL.into_iter().map(UiShapeKind::as_str))
}

fn shape_blends() -> FieldMeaning {
    FieldMeaning::choice(UiShapeBlend::ALL.into_iter().map(UiShapeBlend::as_str))
}

/// Says what the built-in components' fields mean.
pub(super) fn describe_builtins(
    components: &mut ComponentSchemaRegistry,
) -> Result<(), SceneExtractError> {
    describe_drawables(components)?;
    describe_gameplay(components)?;
    Ok(())
}

fn describe_drawables(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.describe::<CameraComponent>([(
        "projection",
        FieldMeaning::choice(CameraComponent::PROJECTIONS),
    )])?;
    describe_projections(components)?;
    components.describe::<MeshComponent>([("texture", texture())])?;
    components.describe::<SpriteComponent>([("texture", texture()), ("tint", COLOUR)])?;
    components.describe::<UiImageComponent>([
        ("texture", texture()),
        ("tint", COLOUR),
        ("anchor", anchors()),
    ])?;
    components.describe::<EffectBurstComponent>([("texture", texture()), ("tint", COLOUR)])?;
    components.describe::<TilemapComponent>([
        ("texture", texture()),
        ("tint", COLOUR),
        (
            "projection",
            FieldMeaning::choice(TileProjection::ALL.into_iter().map(TileProjection::as_str)),
        ),
    ])?;
    components.describe::<TileGridComponent>([(
        "projection",
        FieldMeaning::choice(TileProjection::ALL.into_iter().map(TileProjection::as_str)),
    )])?;
    components
        .describe::<TileVolumeComponent>([("tileset", FieldMeaning::Asset(AssetKind::TileSet))])?;
    components.describe::<ShapeComponent>([
        ("kind", shape_kinds()),
        ("blend", shape_blends()),
        ("fill", COLOUR),
        ("stroke", COLOUR),
        ("sweep_start", FieldMeaning::Angle),
    ])?;
    components.describe::<UiShapeComponent>([
        ("kind", shape_kinds()),
        ("blend", shape_blends()),
        ("anchor", anchors()),
        ("fill", COLOUR),
        ("stroke", COLOUR),
        ("sweep_start", FieldMeaning::Angle),
    ])?;
    components.describe::<UiLayoutComponent>([
        (
            "direction",
            FieldMeaning::choice(UiDirection::ALL.into_iter().map(UiDirection::as_str)),
        ),
        (
            "justify",
            FieldMeaning::choice(UiJustify::ALL.into_iter().map(UiJustify::as_str)),
        ),
        (
            "align",
            FieldMeaning::choice(UiAlign::ALL.into_iter().map(UiAlign::as_str)),
        ),
    ])?;
    describe_text(components)?;
    Ok(())
}

/// What each projection makes a camera hold.
///
/// The two share their planes and differ in what they frame with, which is why
/// the tag cannot be written on its own: a camera whose tag says orthographic
/// and whose fields say perspective is not a camera the engine will load. The
/// near and far planes are named in both, so switching keeps whatever they
/// were and a camera that somehow lacked them gains the pair a fresh one has.
fn describe_projections(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    let [perspective, orthographic] = CameraComponent::PROJECTIONS;
    components.describe_variants::<CameraComponent>(
        "projection",
        [
            (
                perspective,
                json!({
                    "vertical_fov_degrees": CameraComponent::DEFAULT_VERTICAL_FOV_DEGREES,
                    "near": CameraComponent::DEFAULT_NEAR,
                    "far": CameraComponent::DEFAULT_FAR
                }),
            ),
            (
                orthographic,
                json!({
                    "vertical_size": CameraComponent::DEFAULT_VERTICAL_SIZE,
                    "near": CameraComponent::DEFAULT_NEAR,
                    "far": CameraComponent::DEFAULT_FAR
                }),
            ),
        ],
    )?;
    Ok(())
}

fn describe_text(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.describe::<UiTextComponent>([
        ("font", FieldMeaning::Asset(AssetKind::Font)),
        ("color", COLOUR),
        ("anchor", anchors()),
        (
            "wrap",
            FieldMeaning::choice(UiTextWrap::ALL.into_iter().map(UiTextWrap::as_str)),
        ),
        (
            "line_align",
            FieldMeaning::choice(
                UiTextLineAlign::ALL
                    .into_iter()
                    .map(UiTextLineAlign::as_str),
            ),
        ),
        (
            "case",
            FieldMeaning::choice(UiTextCase::ALL.into_iter().map(UiTextCase::as_str)),
        ),
        ("outline.color", COLOUR),
        ("shadow.color", COLOUR),
    ])?;
    Ok(())
}

fn describe_gameplay(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    // A grid occupant names the entity whose tilemap it stands on, which is the
    // one field here that is neither an asset nor a number.
    components.describe::<GridOccupantComponent>([("grid", FieldMeaning::Entity)])?;
    components.describe::<RigidBody2dComponent>([
        (
            "kind",
            FieldMeaning::choice(RigidBodyKind::ALL.into_iter().map(RigidBodyKind::as_str)),
        ),
        ("pose.rotation", FieldMeaning::Angle),
    ])?;
    // One meaning covers every piece: a compound is a list, and the template
    // carries one exemplar for the list to be described through.
    components.describe::<Collider2dComponent>([
        (
            "pieces[].shape.shape",
            FieldMeaning::choice(ColliderShape2d::SHAPES),
        ),
        ("pieces[].rotation", FieldMeaning::Angle),
        (
            "pieces[].restitution",
            FieldMeaning::Range { min: 0.0, max: 1.0 },
        ),
        ("pieces[].layers.memberships", FieldMeaning::Mask),
        ("pieces[].layers.filter", FieldMeaning::Mask),
    ])?;
    describe_shapes(components)?;
    describe_audio(components)?;
    Ok(())
}

/// What each shape makes a collider piece hold.
///
/// The same relationship a camera's projection has, one level down: a box is
/// half extents, a circle is a radius, a capsule is both a half height and a
/// radius. The measurements are a piece of the size a fresh collider is, so a
/// shape switched to is a shape somebody can see rather than one of zero size.
fn describe_shapes(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    let [rectangle, circle, capsule] = ColliderShape2d::SHAPES;
    components.describe_variants::<Collider2dComponent>(
        "pieces[].shape.shape",
        [
            (rectangle, json!({ "half_extents": [0.5, 0.5] })),
            (circle, json!({ "radius": 0.5 })),
            (capsule, json!({ "half_height": 0.5, "radius": 0.25 })),
        ],
    )?;
    Ok(())
}

fn describe_audio(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.describe::<AudioSourceComponent>([
        ("clip", FieldMeaning::Asset(AssetKind::Audio)),
        ("volume", FieldMeaning::Range { min: 0.0, max: 1.0 }),
    ])?;
    components.describe::<SpriteAnimationComponent>([(
        "speed",
        FieldMeaning::Range {
            min: 0.0,
            max: f64::INFINITY,
        },
    )])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sindri_core::{AssetKind, FieldMeaning};

    use crate::extract::registry::builtin_components;

    /// Every field the editor used to guess an asset list for is now described.
    ///
    /// The guard on deleting that table: if a meaning here is dropped, the
    /// editor silently returns to a text box, and this says so instead.
    #[test]
    fn the_asset_fields_the_editor_guessed_are_described() {
        let components = builtin_components().expect("the built-ins register");
        for (type_name, path, kind) in [
            ("sindri.sprite", "texture", AssetKind::Texture),
            ("sindri.mesh", "texture", AssetKind::Texture),
            ("sindri.ui.image", "texture", AssetKind::Texture),
            ("sindri.tilemap", "texture", AssetKind::Texture),
            ("sindri.tile_volume", "tileset", AssetKind::TileSet),
            ("sindri.effect.burst", "texture", AssetKind::Texture),
            ("sindri.ui.text", "font", AssetKind::Font),
            ("sindri.audio.source", "clip", AssetKind::Audio),
        ] {
            assert_eq!(
                components.meaning(type_name, path),
                Some(&FieldMeaning::Asset(kind)),
                "{type_name}.{path} should name a {kind:?}"
            );
        }
    }

    /// A meaning reaches into a nested object and through a list.
    #[test]
    fn nested_and_listed_fields_are_described() {
        let components = builtin_components().expect("the built-ins register");
        assert_eq!(
            components.meaning("sindri.ui.text", "outline.color"),
            Some(&FieldMeaning::Colour)
        );
        assert_eq!(
            components.meaning("sindri.physics2d.collider", "pieces.2.rotation"),
            Some(&FieldMeaning::Angle),
            "every piece of a compound is described by the one exemplar"
        );
        assert_eq!(
            components.meaning("sindri.physics2d.collider", "pieces.0.layers.filter"),
            Some(&FieldMeaning::Mask)
        );
    }

    /// A choice offers exactly the spellings the engine accepts.
    #[test]
    fn a_choice_offers_the_engines_own_spellings() {
        let components = builtin_components().expect("the built-ins register");
        let Some(FieldMeaning::Choice(shapes)) =
            components.meaning("sindri.physics2d.collider", "pieces[].shape.shape")
        else {
            panic!("a collider piece's shape should be a choice");
        };
        assert_eq!(shapes.as_slice(), &["box", "circle", "capsule"]);
    }

    /// A field nobody described says nothing, rather than something wrong.
    #[test]
    fn an_undescribed_field_has_no_meaning() {
        let components = builtin_components().expect("the built-ins register");
        assert_eq!(components.meaning("sindri.sprite", "layer"), None);
        assert_eq!(components.meaning("sindri.tags", "tags"), None);
    }
}
