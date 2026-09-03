//! Format 8 to 9: a font size stops being pixels.

use serde_json::Value;

use crate::SceneMigrationError;

/// How much overlay one pixel was worth on the screen these scenes were
/// authored against.
///
/// The overlay is two units tall whatever the viewport is, so a pixel is only a
/// fraction of it once you say how many pixels tall the screen was. Seven
/// hundred and twenty is the height every one of these scenes was looked at on
/// — it is the conventional design height and the size the captures are taken
/// at — so a scene migrated with it draws what it drew, and now keeps drawing
/// it when the window is a different size.
const OVERLAY_PER_PIXEL_AT_720: f64 = 2.0 / 720.0;

/// Text was the last thing on the screen measured in pixels.
///
/// Everything else about a screen element is in the overlay's own units: two
/// tall, centred on the origin, running out to the aspect ratio either side. Its
/// position is, its scale is, and the safe area is converted into them before
/// anything is placed. A font size was not, so the one number that decides
/// whether a word can be read was the one number that did not follow the
/// screen — a HUD authored on a desktop had unreadable text on a phone, and a
/// heading authored in the units the rest of the element uses drew nothing at
/// all.
///
/// This does not move anything. A scene that read correctly at 720 reads
/// correctly at 720 after it, and correctly everywhere else for the first time.
///
/// The result is never an error, and is one anyway: every step in the chain has
/// this shape, and a step that did not would be the one a future step could not
/// be written like.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn size_text_in_overlay_units(document: &mut Value) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entity in entities {
        let Some(text) = entity
            .get_mut("components")
            .and_then(Value::as_object_mut)
            .and_then(|components| components.get_mut("sindri.ui.text"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        for field in ["font_size", "line_height"] {
            let Some(pixels) = text.get(field).and_then(Value::as_f64) else {
                continue;
            };
            let overlay = pixels * OVERLAY_PER_PIXEL_AT_720;
            let Some(converted) = serde_json::Number::from_f64(overlay) else {
                // A size that is not a number was already broken; leaving it is
                // how it stays reported as broken rather than becoming a
                // plausible-looking zero.
                continue;
            };
            text.insert((*field).to_owned(), Value::Number(converted));
        }
    }
    Ok(())
}
