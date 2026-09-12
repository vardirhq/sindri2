//! LEGO-like authoring of sparse stackable tile cells.
//!
//! A volume remains one component payload so a stroke is one undoable command.
//! These helpers edit only `cells`; unknown future fields survive unchanged.

use glam::{Mat4, Quat, Vec3};
use serde_json::{Value, json};
use sindri_core::Transform3D;
use sindri_grid::GridCoord3;
use sindri_scene::{TileGridComponent, TileProjection, TileVolumeComponent};

#[cfg(test)]
mod tests;

pub const GRID_TYPE_NAME: &str = "sindri.tile_grid";
pub const TYPE_NAME: &str = "sindri.tile_volume";

#[derive(Default)]
pub struct TileVolumeTool {
    pub enabled: bool,
    pub erase: bool,
    pub tile: Option<String>,
    pub level: i32,
}

impl TileVolumeTool {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn brush(&self) -> Option<TileBrush<'_>> {
        if !self.enabled {
            return None;
        }
        if self.erase {
            return Some(TileBrush::Erase);
        }
        self.tile.as_deref().map(TileBrush::Tile)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileBrush<'a> {
    Erase,
    Tile(&'a str),
}

pub fn component(payload: &Value) -> Result<TileVolumeComponent, String> {
    serde_json::from_value(payload.clone()).map_err(|error| error.to_string())
}

/// Place or remove one exact XYZ cell, keeping serialized cells deterministic.
pub fn paint(payload: &mut Value, coord: GridCoord3, brush: TileBrush<'_>) -> Result<bool, String> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "tile volume payload is not an object".to_owned())?;
    let cells = object
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "tile volume cells are not an array".to_owned())?;
    let at = cells.iter().position(|cell| position(cell) == Some(coord));

    match (brush, at) {
        (TileBrush::Erase, None) => return Ok(false),
        (TileBrush::Erase, Some(index)) => {
            cells.remove(index);
        }
        (TileBrush::Tile(tile), Some(index)) => {
            if cells[index].get("tile").and_then(Value::as_str) == Some(tile) {
                return Ok(false);
            }
            cells[index] = cell(coord, tile);
        }
        (TileBrush::Tile(tile), None) => cells.push(cell(coord, tile)),
    }
    cells.sort_by_key(|value| {
        position(value).map_or((i32::MAX, i32::MAX, i32::MAX), |at| (at.z, at.y, at.x))
    });
    Ok(true)
}

fn cell(coord: GridCoord3, tile: &str) -> Value {
    json!({ "position": [coord.x, coord.y, coord.z], "tile": tile })
}

fn position(value: &Value) -> Option<GridCoord3> {
    let values = value.get("position")?.as_array()?;
    if values.len() != 3 {
        return None;
    }
    Some(GridCoord3::new(
        i32::try_from(values[0].as_i64()?).ok()?,
        i32::try_from(values[1].as_i64()?).ok()?,
        i32::try_from(values[2].as_i64()?).ok()?,
    ))
}

/// Resolve a pointer onto the currently edited horizontal block level.
pub fn cell_at_viewport(
    grid: &TileGridComponent,
    transform: Transform3D,
    view_projection: Mat4,
    point: [f32; 2],
    level: i32,
) -> Option<GridCoord3> {
    if !(0.0..=1.0).contains(&point[0]) || !(0.0..=1.0).contains(&point[1]) {
        return None;
    }
    let inverse_view = view_projection.inverse();
    let inverse_model = transform_matrix(transform).inverse();
    if !matrix_is_finite(inverse_view) || !matrix_is_finite(inverse_model) {
        return None;
    }
    let x = point[0] * 2.0 - 1.0;
    let y = 1.0 - point[1] * 2.0;
    let near = inverse_model.transform_point3(inverse_view.project_point3(Vec3::new(x, y, 0.0)));
    let far = inverse_model.transform_point3(inverse_view.project_point3(Vec3::new(x, y, 1.0)));
    let direction = far - near;
    if direction.z.abs() <= f32::EPSILON {
        return None;
    }
    let distance = -near.z / direction.z;
    if !(0.0..=1.0).contains(&distance) {
        return None;
    }
    let local = near + direction * distance;
    grid.local_to_cell_at_level([local.x, local.y], level)
}

/// Project the editable top diamond or rectangle back into viewport fractions.
pub fn cell_outline(
    grid: &TileGridComponent,
    transform: Transform3D,
    view_projection: Mat4,
    coord: GridCoord3,
) -> Option<[[f32; 2]; 4]> {
    let [x, y] = grid.cell_to_local(coord)?;
    let half_width = grid.cell_size[0] * 0.5;
    let half_height = grid.cell_size[1] * 0.5;
    let local = match grid.projection {
        TileProjection::Orthogonal => [
            Vec3::new(x - half_width, y + half_height, 0.0),
            Vec3::new(x + half_width, y + half_height, 0.0),
            Vec3::new(x + half_width, y - half_height, 0.0),
            Vec3::new(x - half_width, y - half_height, 0.0),
        ],
        TileProjection::Isometric => [
            Vec3::new(x, y + half_height, 0.0),
            Vec3::new(x + half_width, y, 0.0),
            Vec3::new(x, y - half_height, 0.0),
            Vec3::new(x - half_width, y, 0.0),
        ],
    };
    let model = transform_matrix(transform);
    let mut projected = [[0.0; 2]; 4];
    for (index, point) in local.into_iter().enumerate() {
        let clip = view_projection * model.transform_point3(point).extend(1.0);
        if !clip.is_finite() || clip.w.abs() <= f32::EPSILON {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        projected[index] = [(ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5];
    }
    Some(projected)
}

fn transform_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.position),
    )
}

fn matrix_is_finite(matrix: Mat4) -> bool {
    matrix.to_cols_array().into_iter().all(f32::is_finite)
}
