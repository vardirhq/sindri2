//! Which fields serde will ask a component for.
//!
//! Asked of serde rather than written down beside the type, because a list
//! written down beside a type is a list that drifts from it — and a drifted
//! field template is exactly the bug templates exist to prevent. Serde hands the
//! static field list to `Deserializer::deserialize_struct`, so a deserializer
//! that records it and then refuses to go any further learns the names without
//! needing an instance to inspect.

use super::SceneComponent;

/// The field names serde will look for when it decodes this component.
///
/// `None` for a component that is not a struct. An externally or internally
/// tagged enum asks for an enum or a map instead, and its fields depend on
/// which variant it is — `sindri.camera` is the case, and what fields a camera
/// has is decided by its projection rather than by its type.
pub(super) fn declared_fields<T: SceneComponent>() -> Option<Vec<&'static str>> {
    let mut captured = None;
    // The probe always fails; the answer is what it recorded on the way.
    let _ = <T as serde::Deserialize>::deserialize(FieldProbe {
        captured: &mut captured,
    });
    captured
}

/// Records the field list serde asks for, then stops.
struct FieldProbe<'a> {
    captured: &'a mut Option<Vec<&'static str>>,
}

/// The probe's only outcome. It never produces a value, by design.
#[derive(Debug)]
struct ProbeStopped;

impl std::fmt::Display for ProbeStopped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("field probe stopped")
    }
}

impl std::error::Error for ProbeStopped {}

impl serde::de::Error for ProbeStopped {
    fn custom<T: std::fmt::Display>(_message: T) -> Self {
        Self
    }
}

impl<'de> serde::Deserializer<'de> for FieldProbe<'_> {
    type Error = ProbeStopped;

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        *self.captured = Some(fields.to_vec());
        Err(ProbeStopped)
    }

    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ProbeStopped)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}
