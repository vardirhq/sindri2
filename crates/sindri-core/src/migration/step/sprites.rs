//! Formats 2 to 4: sprite draw order, and naming the parts of a sheet.

use serde_json::Value;

use crate::SceneMigrationError;

use super::transform::set_transform_z;

/// Format 3 sorts transparent sprites by how far from the camera they are
/// rather than by a `depth` number typed beside them, so the field goes and the
/// transform's Z takes over the job.
///
/// A screen-space sprite's Z did nothing at all in format 2 — the overlay read
/// only X and Y — so its `depth` becomes a Z, negated, because screen overlay
/// space looks down the axis from `+Z` and a greater depth meant further away.
/// The stack it describes comes out in the same order it went in.
///
/// A world-space sprite already had a Z that placed it, and that Z is now what
/// orders it too, so its `depth` is simply dropped. That is the change itself
/// rather than a loss: a sort key that disagreed with where the sprite was is
/// exactly what this format stops allowing.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn sort_sprites_by_where_they_are(
    document: &mut Value,
) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };

    for entity in entities {
        let Some(fields) = entity.as_object_mut() else {
            continue;
        };
        let Some(sprite) = fields
            .get_mut("components")
            .and_then(Value::as_object_mut)
            .and_then(|components| components.get_mut(SPRITE_COMPONENT))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let in_the_world = sprite.get("space").and_then(Value::as_str) == Some("world");
        let Some(depth) = sprite.remove("depth").as_ref().and_then(Value::as_f64) else {
            continue;
        };
        if in_the_world {
            continue;
        }
        set_transform_z(fields, -depth);
    }
    Ok(())
}

/// The component type name the format itself has to know, because the format is
/// what changed. `sindri-scene` owns what the component means.
pub(crate) const SPRITE_COMPONENT: &str = "sindri.sprite";

/// Format 3 to 4: a sheet is cut by the image, not by whoever draws it.
///
/// Before this, three components each said how a sheet was divided — a sprite
/// carried a raw rect, an animation carried a grid and cell numbers, a tilemap
/// carried a second grid and more cell numbers. After it, all three name
/// sprites and a sheet document beside the image says where those are.
///
/// **A migrated scene needs its sheets written.** This step can convert a
/// document; it cannot create the sidecar files beside the textures, because a
/// migration is handed JSON and not a project. What it does instead is emit the
/// names a default slice produces — cell `n` is called `"n"` — so a sheet
/// declaring the same grid the scene used to carry resolves every reference it
/// writes. The grid to declare is the one being removed here, which is why the
/// errors below name it.
pub(crate) fn name_the_parts_of_a_sheet(document: &mut Value) -> Result<(), SceneMigrationError> {
    let Some(entities) = document.get_mut("entities").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entity in entities {
        let Some(components) = entity
            .as_object_mut()
            .and_then(|fields| fields.get_mut("components"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };

        if let Some(sprite) = components
            .get_mut("sindri.sprite")
            .and_then(Value::as_object_mut)
            && let Some(rect) = sprite.remove("uv_rect")
        {
            let cell = cell_of(&rect)?;
            if let Some(index) = cell
                && let Some(texture) = sprite.get("texture").and_then(Value::as_str)
            {
                let named = format!("{texture}#{index}");
                sprite.insert("texture".to_owned(), Value::String(named));
            }
        }

        if let Some(animation) = components
            .get_mut("sindri.sprite_animation")
            .and_then(Value::as_object_mut)
        {
            animation.remove("sheet");
            if let Some(clips) = animation.get_mut("clips").and_then(Value::as_object_mut) {
                for clip in clips.values_mut() {
                    let Some(frames) = clip.get_mut("frames").and_then(Value::as_array_mut) else {
                        continue;
                    };
                    for frame in frames.iter_mut() {
                        if let Some(cell) = frame.as_u64() {
                            *frame = Value::String(cell.to_string());
                        }
                    }
                }
            }
        }

        if let Some(tilemap) = components
            .get_mut("sindri.tilemap")
            .and_then(Value::as_object_mut)
        {
            tilemap.remove("sheet_columns");
            tilemap.remove("sheet_rows");
            let highest = tilemap
                .get("tiles")
                .and_then(Value::as_array)
                .map(|tiles| {
                    tiles
                        .iter()
                        .filter_map(Value::as_u64)
                        .max()
                        .map_or(0, |highest| highest + 1)
                })
                .unwrap_or_default();
            let palette: Vec<Value> = (0..highest)
                .map(|index| Value::String(index.to_string()))
                .collect();
            tilemap.insert("palette".to_owned(), Value::Array(palette));
        }
    }
    Ok(())
}

pub(crate) fn cell_of(rect: &Value) -> Result<Option<u64>, SceneMigrationError> {
    const TOLERANCE: f64 = 1.0e-4;

    let Some(values) = rect.as_array() else {
        return Ok(None);
    };
    let [x, y, width, height] = <[f64; 4]>::try_from(
        values
            .iter()
            .filter_map(Value::as_f64)
            .collect::<Vec<f64>>()
            .as_slice(),
    )
    .map_err(|_| {
        SceneMigrationError::Unconvertible(format!(
            "a sprite's uv_rect must be four numbers, and this one is {rect}"
        ))
    })?;
    let close = |value: f64, to: f64| (value - to).abs() < TOLERANCE;
    if close(x, 0.0) && close(y, 0.0) && close(width, 1.0) && close(height, 1.0) {
        return Ok(None);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = |value: f64| -> Option<u64> {
        let rounded = value.round();
        (rounded >= 0.0 && close(value, rounded)).then_some(rounded as u64)
    };
    let unconvertible = || {
        SceneMigrationError::Unconvertible(format!(
            "a sprite's uv_rect {rect} is not a whole cell of a uniform grid, so format 4 has no \
             name for it — give its texture a sheet naming that rect, and point the sprite at it"
        ))
    };
    if width <= 0.0 || height <= 0.0 {
        return Err(unconvertible());
    }
    let columns = whole(1.0 / width).ok_or_else(unconvertible)?;
    let column = whole(x / width).ok_or_else(unconvertible)?;
    let row = whole(y / height).ok_or_else(unconvertible)?;
    Ok(Some(row * columns + column))
}
