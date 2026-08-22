//! Authoring a tilemap from the image that supplies its sprites.
//!
//! A tilemap is stored as one component payload and the editor must preserve
//! fields it does not understand, so painting changes the stored JSON rather
//! than serializing a typed view back over it. [`TilemapComponent`] is still
//! used to interpret and validate the shape. The caller turns the changed
//! payload into `WorldCommand::SetComponent`, which makes a stroke undoable in
//! exactly the same way as every other inspector edit.

use std::path::{Path, PathBuf};

use eframe::egui;
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, SpriteSheetDocument, Transform3D, sheet_id_for};
use sindri_scene::{TileProjection, TilemapComponent};

pub const TYPE_NAME: &str = "sindri.tilemap";

/// One sprite offered by the palette, with the part of the image it previews.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteSprite {
    pub name: String,
    pub rect: Option<[f32; 4]>,
}

/// A project texture and the named sprites from the sheet beside it.
///
/// Shared by tile painting and animation authoring: both need the same image,
/// the same sheet naming rule, and the same UV rectangles. Read once per
/// texture rather than once per frame because an editor panel redraws
/// continuously and a sidecar changes occasionally.
#[derive(Default)]
pub struct SpritePalette {
    key: Option<(PathBuf, String)>,
    image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
    sprites: Vec<PaletteSprite>,
    problem: Option<String>,
}

impl SpritePalette {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.image = None;
        self.texture = None;
        self.sprites.clear();
        self.problem = None;
    }

    /// Loads the texture and its derived sheet when either changes identity.
    pub fn ensure(&mut self, root: Option<&Path>, texture: &str) {
        let Some(root) = root else {
            self.invalidate();
            self.problem = Some("Save the scene before loading project sprites".to_owned());
            return;
        };
        let key = (root.to_path_buf(), texture.to_owned());
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.invalidate();
        self.key = Some(key);

        let Some(id) = AssetId::new(texture.to_owned()).ok() else {
            self.problem = Some(format!("{texture} is not a file-backed texture"));
            return;
        };
        let Some(sheet_id) = sheet_id_for(&id) else {
            self.problem = Some(format!("{texture} cannot have a sprite sheet"));
            return;
        };
        let texture_path = root.join(id.as_str());
        let sheet_path = root.join(sheet_id.as_str());

        let image_bytes = match std::fs::read(&texture_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.problem = Some(format!("{}: {error}", texture_path.display()));
                return;
            }
        };
        match TextureAssetDecoder.decode(AssetBytes::new(id, image_bytes)) {
            Ok(asset) => {
                self.image = Some(egui::ColorImage::from_rgba_unmultiplied(
                    [asset.width() as usize, asset.height() as usize],
                    asset.rgba8(),
                ));
            }
            Err(error) => {
                self.problem = Some(error.to_string());
                return;
            }
        }

        let json = match std::fs::read_to_string(&sheet_path) {
            Ok(json) => json,
            Err(error) => {
                self.problem = Some(format!("{}: {error}", sheet_path.display()));
                return;
            }
        };
        match SpriteSheetDocument::from_json(&json).and_then(|sheet| sheet.rects()) {
            Ok(rects) => {
                self.sprites = rects
                    .into_iter()
                    .map(|(name, rect)| PaletteSprite {
                        name,
                        rect: Some(rect),
                    })
                    .collect();
            }
            Err(error) => self.problem = Some(error.to_string()),
        }
    }

    pub fn texture_id(&mut self, context: &egui::Context) -> Option<egui::TextureId> {
        if self.texture.is_none()
            && let Some(image) = self.image.take()
        {
            let label = self.key.as_ref().map_or_else(
                || "sprite palette".to_owned(),
                |(_, texture)| texture.clone(),
            );
            self.texture = Some(context.load_texture(label, image, egui::TextureOptions::NEAREST));
        }
        self.texture.as_ref().map(egui::TextureHandle::id)
    }

    pub fn sprites(&self) -> &[PaletteSprite] {
        &self.sprites
    }

    pub fn sprite(&self, name: &str) -> Option<&PaletteSprite> {
        self.sprites.iter().find(|sprite| sprite.name == name)
    }

    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }
}

/// The tool state that is not part of a scene: which brush is in the user's
/// hand and whether the Scene view currently belongs to it.
#[derive(Default)]
pub struct TilemapTool {
    pub enabled: bool,
    pub erase: bool,
    pub sprite: Option<String>,
    pub palette: SpritePalette,
}

impl TilemapTool {
    pub fn reset(&mut self) {
        self.enabled = false;
        self.erase = false;
        self.sprite = None;
        self.palette.invalidate();
    }

    pub fn brush(&self) -> Option<TileBrush<'_>> {
        if !self.enabled {
            return None;
        }
        if self.erase {
            return Some(TileBrush::Erase);
        }
        self.sprite.as_deref().map(TileBrush::Sprite)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileBrush<'a> {
    Erase,
    Sprite(&'a str),
}

/// Reads the typed meaning without replacing the stored payload with it.
pub fn component(payload: &Value) -> Result<TilemapComponent, String> {
    serde_json::from_value(payload.clone()).map_err(|error| error.to_string())
}

/// Reshapes a map, preserving the overlap and making every new cell empty.
pub fn resize(payload: &mut Value, columns: u32, rows: u32) -> Result<bool, String> {
    let map = component(payload)?;
    if map.columns == columns && map.rows == rows && map.tiles.len() == map.expected_cells() {
        return Ok(false);
    }
    let cells = (columns as usize)
        .checked_mul(rows as usize)
        .ok_or_else(|| format!("tilemap size {columns}x{rows} is too large"))?;
    let mut tiles = vec![Value::Null; cells];
    let old = payload
        .get("tiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for row in 0..rows.min(map.rows) {
        for column in 0..columns.min(map.columns) {
            let old_cell = row as usize * map.columns as usize + column as usize;
            let new_cell = row as usize * columns as usize + column as usize;
            if let Some(value) = old.get(old_cell) {
                tiles[new_cell] = value.clone();
            }
        }
    }
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "tilemap payload is not an object".to_owned())?;
    object.insert("columns".to_owned(), Value::from(columns));
    object.insert("rows".to_owned(), Value::from(rows));
    object.insert("tiles".to_owned(), Value::Array(tiles));
    Ok(true)
}

/// Paints one cell. A sprite absent from the compact palette is added once.
pub fn paint(
    payload: &mut Value,
    column: u32,
    row: u32,
    brush: TileBrush<'_>,
) -> Result<bool, String> {
    let map = component(payload)?;
    if column >= map.columns || row >= map.rows {
        return Err(format!("tile {column},{row} is outside the map"));
    }
    let cell = row as usize * map.columns as usize + column as usize;
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "tilemap payload is not an object".to_owned())?;
    let next = match brush {
        TileBrush::Erase => Value::Null,
        TileBrush::Sprite(sprite) => {
            let palette = object
                .get_mut("palette")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| "tilemap palette is not an array".to_owned())?;
            let index = palette
                .iter()
                .position(|value| value.as_str() == Some(sprite))
                .unwrap_or_else(|| {
                    palette.push(Value::String(sprite.to_owned()));
                    palette.len() - 1
                });
            Value::from(
                u32::try_from(index).map_err(|_| "tilemap palette is too large".to_owned())?,
            )
        }
    };
    let tiles = object
        .get_mut("tiles")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "tilemap tiles are not an array".to_owned())?;
    let Some(current) = tiles.get_mut(cell) else {
        return Err("tilemap cell array does not match its size; resize it first".to_owned());
    };
    if *current == next {
        return Ok(false);
    }
    *current = next;
    Ok(true)
}

/// Turns a normalized viewport point into a cell on the selected map.
///
/// The ray is transformed into the entity's local space before meeting Z=0,
/// so moving, rotating, or scaling a map does not change which visible tile a
/// click names.
pub fn tile_at_viewport(
    map: &TilemapComponent,
    transform: Transform3D,
    view_projection: Mat4,
    point: [f32; 2],
) -> Option<(u32, u32)> {
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
    map.local_to_tile(local.x, local.y)
}

/// The hovered cell's outline in normalized viewport coordinates.
pub fn tile_outline(
    map: &TilemapComponent,
    transform: Transform3D,
    view_projection: Mat4,
    column: u32,
    row: u32,
) -> Option<[[f32; 2]; 4]> {
    let [x, y] = map.tile_to_local(column, row);
    let half_width = map.tile_size[0] * 0.5;
    let half_height = map.tile_size[1] * 0.5;
    let local = match map.projection {
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

#[cfg(test)]
mod tests {
    use super::{TileBrush, paint, resize, tile_at_viewport, tile_outline};
    use glam::Mat4;
    use serde_json::json;
    use sindri_core::{
        CommandBuffer, CommandHistory, EntityData, Transform3D, World, WorldCommand,
    };
    use sindri_scene::TilemapComponent;

    fn payload() -> serde_json::Value {
        json!({
            "texture": "tiles.png",
            "palette": ["floor"],
            "columns": 2,
            "rows": 2,
            "tile_size": [1.0, 1.0],
            "projection": "orthogonal",
            "tiles": [0, null, null, 0],
            "space": "world",
            "future": { "kept": true }
        })
    }

    #[test]
    fn resizing_preserves_the_overlap_and_unknown_fields() {
        let mut map = payload();
        assert!(resize(&mut map, 3, 2).expect("the map resizes"));
        assert_eq!(map["tiles"], json!([0, null, null, null, 0, null]));
        assert_eq!(map["future"], json!({ "kept": true }));
    }

    #[test]
    fn a_stroke_adds_one_palette_entry_and_erase_writes_null() {
        let mut map = payload();
        assert!(paint(&mut map, 1, 0, TileBrush::Sprite("wall")).expect("paint works"));
        assert!(paint(&mut map, 0, 1, TileBrush::Sprite("wall")).expect("paint works"));
        assert_eq!(map["palette"], json!(["floor", "wall"]));
        assert_eq!(map["tiles"], json!([0, 1, 1, 0]));
        assert!(paint(&mut map, 1, 0, TileBrush::Erase).expect("erase works"));
        assert_eq!(map["tiles"], json!([0, null, 1, 0]));
    }

    #[test]
    fn a_drag_is_one_undoable_world_edit() {
        let original = payload();
        let mut world = World::default();
        let entity = world.spawn(EntityData {
            components: [(super::TYPE_NAME.to_owned(), original.clone())]
                .into_iter()
                .collect(),
            ..EntityData::default()
        });
        let mut history = CommandHistory::default();

        for (column, row) in [(1, 0), (0, 1)] {
            let mut changed = world
                .get(entity)
                .and_then(|data| data.components.get(super::TYPE_NAME))
                .cloned()
                .expect("the map exists");
            assert!(
                paint(&mut changed, column, row, TileBrush::Sprite("wall"))
                    .expect("painting works")
            );
            let mut buffer = CommandBuffer::new();
            buffer.push(WorldCommand::SetComponent {
                entity,
                type_name: super::TYPE_NAME.to_owned(),
                payload: changed,
            });
            history
                .apply(
                    buffer
                        .into_transaction("Paint tilemap")
                        .merging("tilemap stroke"),
                    &mut world,
                )
                .expect("the cell is written");
        }

        history.undo(&mut world).expect("the stroke undoes");
        assert_eq!(
            world
                .get(entity)
                .and_then(|data| data.components.get(super::TYPE_NAME)),
            Some(&original),
        );
        assert_eq!(history.undo_label(), None, "the whole drag was one step");
    }

    #[test]
    fn a_viewport_click_meets_the_map_in_its_own_space() {
        let map: TilemapComponent = serde_json::from_value(payload()).expect("a map");
        assert_eq!(
            tile_at_viewport(&map, Transform3D::default(), Mat4::IDENTITY, [0.75, 0.75]),
            Some((0, 0))
        );
        let outline = tile_outline(&map, Transform3D::default(), Mat4::IDENTITY, 0, 0)
            .expect("the cell projects");
        assert_eq!(outline, [[0.5, 0.5], [1.0, 0.5], [1.0, 1.0], [0.5, 1.0]]);
    }
}
