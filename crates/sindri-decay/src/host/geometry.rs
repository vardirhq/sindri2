//! A gameplay grid's shape, from whichever component carries it.
//!
//! Two components describe the same geometry. `sindri.tilemap` has carried it
//! since the flat map was the only map; `sindri.tile_grid` carries it for a
//! stackable tile volume, under different field names and beside a level step
//! the flat map has no concept of. The columns, the rows, the cell size and the
//! projection mean exactly the same thing in both, and both place their origin
//! and their Y axis the same way.
//!
//! So a script asking where a cell is, or how big the map is, is asking a
//! question neither component owns. Reading it here rather than in each caller
//! is what lets a scene drop `sindri.tilemap` without every `Grid.*` call in
//! every script going with it.
//!
//! What is *not* shared is cell contents: a flat array of palette indices is a
//! `sindri.tilemap` idea, and `Grid.tile` and `Grid.set_tile` still say so.

use decay_ir::Path;
use decay_runtime::RuntimeError;
use serde_json::Value as Json;
use sindri_core::EntityData;
use sindri_grid::{GridSpace, PlanePoint, PlaneYAxis, Projection};

use crate::surface::{TILE_GRID_COMPONENT, TILEMAP_COMPONENT};

/// One grid's logical shape and the plane its cells live on.
#[derive(Clone, Copy)]
pub(super) struct GridGeometry {
    pub(super) space: GridSpace,
    pub(super) columns: usize,
    pub(super) rows: usize,
}

/// Which component answered, so a caller needing more than geometry can say
/// what is missing rather than reporting the generic absence of both.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GridSource {
    Tilemap,
    TileGrid,
}

/// The geometry of the grid on this entity, and the component it came from.
///
/// `sindri.tilemap` is tried first so a scene carrying both — which is what a
/// half-migrated scene looks like — keeps answering exactly as it did before
/// the second component existed.
pub(super) fn read(
    path: &Path,
    data: &EntityData,
) -> Result<(GridGeometry, GridSource), RuntimeError> {
    if let Some(payload) = data.components.get(TILEMAP_COMPONENT) {
        let geometry = from_payload(path, payload, "tile_size")?;
        return Ok((geometry, GridSource::Tilemap));
    }
    if let Some(payload) = data.components.get(TILE_GRID_COMPONENT) {
        let geometry = from_payload(path, payload, "cell_size")?;
        return Ok((geometry, GridSource::TileGrid));
    }
    Err(RuntimeError::Host(format!(
        "{} needs its grid entity to carry {TILEMAP_COMPONENT} or {TILE_GRID_COMPONENT}",
        path.dotted()
    )))
}

/// Read from the stored payload rather than through a typed view, so a grid
/// carrying a field this build does not know about is still answerable.
fn from_payload(
    path: &Path,
    payload: &Json,
    size_field: &str,
) -> Result<GridGeometry, RuntimeError> {
    let cell = cell_size(path, payload, size_field)?;
    let projection = projection(path, payload)?;
    // Both components put a cell's origin in the same place: an orthogonal map
    // measures from a cell's centre, an isometric one from the diamond's apex.
    let origin = match projection {
        Projection::Orthogonal => PlanePoint::new(cell[0] * 0.5, -cell[1] * 0.5),
        Projection::Isometric => PlanePoint::default(),
    };
    let space =
        GridSpace::with_origin_and_y_axis(projection, cell[0], cell[1], origin, PlaneYAxis::Up)
            .map_err(|error| RuntimeError::Host(format!("{}: {error}", path.dotted())))?;
    Ok(GridGeometry {
        space,
        columns: count(path, payload, "columns")?,
        rows: count(path, payload, "rows")?,
    })
}

fn cell_size(path: &Path, payload: &Json, field: &str) -> Result<[f64; 2], RuntimeError> {
    let Some(size) = payload.get(field) else {
        return Ok([1.0, 1.0]);
    };
    let Some([width, height]) = size.as_array().map(Vec::as_slice) else {
        return Err(RuntimeError::Host(format!(
            "{} found a {field} that is not two numbers",
            path.dotted()
        )));
    };
    let read = |value: &Json, which: &str| {
        value.as_f64().ok_or_else(|| {
            RuntimeError::Host(format!(
                "{} found a cell {which} that is not a number",
                path.dotted()
            ))
        })
    };
    Ok([read(width, "width")?, read(height, "height")?])
}

fn projection(path: &Path, payload: &Json) -> Result<Projection, RuntimeError> {
    match payload
        .get("projection")
        .and_then(Json::as_str)
        .unwrap_or("orthogonal")
    {
        "orthogonal" => Ok(Projection::Orthogonal),
        "isometric" => Ok(Projection::Isometric),
        other => Err(RuntimeError::Host(format!(
            "{} found an unknown tile projection `{other}`",
            path.dotted()
        ))),
    }
}

fn count(path: &Path, payload: &Json, field: &str) -> Result<usize, RuntimeError> {
    usize::try_from(
        payload.get(field).and_then(Json::as_u64).ok_or_else(|| {
            RuntimeError::Host(format!("{}'s grid has no {field}", path.dotted()))
        })?,
    )
    .map_err(|_| RuntimeError::Host(format!("{}'s grid has too many {field}", path.dotted())))
}
