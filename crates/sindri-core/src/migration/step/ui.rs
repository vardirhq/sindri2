//! Format 7 to 8: the screen half of the scene becomes its own components.

use serde_json::Value;

use crate::SceneMigrationError;

/// A sprite is in the world, and what is on the screen is a UI element.
///
/// Format 7 said which by a `space` field on `sindri.sprite`, so one component
/// meant two things: a screen sprite anchored to a viewport edge, and a world
/// sprite placed by its transform, for which the anchor decided nothing. Format
/// 8 splits them, so what a component holds no longer depends on the value of
/// one of its own fields.
///
/// - A sprite in `screen` space — which is what a sprite with no `space` was —
///   becomes `sindri.ui.image`, keeping its anchor, tint, layer, and texture.
/// - A sprite in `world` space stays `sindri.sprite` and loses `space` along
///   with the `anchor` that never applied to it.
/// - `sindri.text` becomes `sindri.ui.text`. It was always screen-space, so
///   only the name moves; the payload is carried across untouched.
/// - `sindri.tilemap` loses `space`, because a map is in the world.
///
/// Nothing here moves anything: a migrated scene draws the picture it drew.
pub(crate) fn split_the_screen_from_the_world(
    document: &mut Value,
) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entity in entities {
        let Some(fields) = entity.as_object_mut() else {
            continue;
        };
        let entity_id = fields
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<an entity with no id>")
            .to_owned();
        let Some(components) = fields.get_mut("components").and_then(Value::as_object_mut) else {
            continue;
        };

        if components.contains_key("sindri.text") && components.contains_key("sindri.ui.text") {
            return Err(SceneMigrationError::Unconvertible(format!(
                "entity '{entity_id}' carries both 'sindri.text' and 'sindri.ui.text', which are \
                 different authored data; remove one before upgrading the scene"
            )));
        }
        if let Some(payload) = components.remove("sindri.text") {
            components.insert("sindri.ui.text".to_owned(), payload);
        }

        // A screen-space map has no format 8 spelling. Nothing here can invent
        // one — a viewport-anchored grid of tiles is a UI element rather than a
        // tilemap — so the author is told rather than shown a floor that has
        // silently moved into the world.
        if let Some(tilemap) = components
            .get_mut("sindri.tilemap")
            .and_then(Value::as_object_mut)
        {
            let space = tilemap.remove("space");
            let in_the_world = space.as_ref().and_then(Value::as_str) == Some("world");
            if !in_the_world {
                return Err(SceneMigrationError::Unconvertible(format!(
                    "entity '{entity_id}' has a screen-space tilemap, which format 8 has no name \
                     for; give it \"space\": \"world\" if it is a floor, or replace it with \
                     'sindri.ui.image' elements if it is a HUD"
                )));
            }
        }

        let Some(sprite) = components
            .get_mut("sindri.sprite")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let in_the_world = sprite.remove("space").as_ref().and_then(Value::as_str) == Some("world");
        if in_the_world {
            // An anchor on a world sprite decided nothing in format 7 either,
            // so dropping it changes no picture.
            sprite.remove("anchor");
            continue;
        }
        if components.contains_key("sindri.ui.image") {
            return Err(SceneMigrationError::Unconvertible(format!(
                "entity '{entity_id}' carries both a screen-space 'sindri.sprite' and a \
                 'sindri.ui.image', which are different authored data; remove one before \
                 upgrading the scene"
            )));
        }
        let payload = components
            .remove("sindri.sprite")
            .expect("the sprite was just read from this map");
        components.insert("sindri.ui.image".to_owned(), payload);
    }
    Ok(())
}
