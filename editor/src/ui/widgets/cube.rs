//! A block drawn as a small isometric cube: its top on top, its side on the two
//! faces that show.
//!
//! What a voxel material is, said the way the world will say it. A menu of
//! material IDs made an author remember that 4 is sand; the cube is sand.

use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Sense, Shape, Stroke, Vec2, pos2};

use crate::ui::theme::color;

/// Part of a loaded texture: the sheet on the GPU, and which of it to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub texture: egui::TextureId,
    /// In texture coordinates, top-left origin.
    pub uv: Rect,
}

/// How much darker each side face is than the top, so the three faces read as
/// a solid rather than as one flat hexagon.
const LEFT_SHADE: u8 = 214;
const RIGHT_SHADE: u8 = 168;

/// A cube `extent` points square, allocated in the layout.
pub fn cube(ui: &mut egui::Ui, extent: f32, top: Option<Picture>, side: Option<Picture>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(extent), Sense::hover());
    paint(ui.painter(), rect, top, side);
}

/// A cube filling `rect`'s largest centred square.
///
/// A face with no picture — a texture still loading, or one that will not —
/// is drawn flat grey, so the shape is still a block rather than a gap.
pub fn paint(painter: &egui::Painter, rect: Rect, top: Option<Picture>, side: Option<Picture>) {
    let [upper, right, centre, left, bottom, lower_left, lower_right] = corners(rect);
    face(painter, [upper, right, centre, left], top, 255);
    face(
        painter,
        [left, centre, bottom, lower_left],
        side,
        LEFT_SHADE,
    );
    face(
        painter,
        [centre, right, lower_right, bottom],
        side,
        RIGHT_SHADE,
    );
    painter.add(Shape::closed_line(
        vec![upper, right, lower_right, bottom, lower_left, left],
        Stroke::new(1.0, color::LINE_SOFT),
    ));
}

/// The seven points of an isometric cube's outline and middle, in `rect`:
/// the top corner, the right, the centre, the left, the bottom, and the two
/// lower corners of the sides.
fn corners(rect: Rect) -> [Pos2; 7] {
    let side = rect.width().min(rect.height());
    let centre = rect.center();
    let half = side * 0.5;
    // The horizontal reach of an isometric cube is cos(30°) of its height.
    let reach = half * 0.866;
    let quarter = half * 0.5;
    [
        pos2(centre.x, centre.y - half),
        pos2(centre.x + reach, centre.y - quarter),
        centre,
        pos2(centre.x - reach, centre.y - quarter),
        pos2(centre.x, centre.y + half),
        pos2(centre.x - reach, centre.y + quarter),
        pos2(centre.x + reach, centre.y + quarter),
    ]
}

/// One face, its corners in the order top-left, top-right, bottom-right,
/// bottom-left of the picture it shows.
fn face(painter: &egui::Painter, points: [Pos2; 4], picture: Option<Picture>, shade: u8) {
    let tint = Color32::from_gray(shade);
    let Some(picture) = picture else {
        let grey = Color32::from_gray(shade / 2);
        painter.add(Shape::convex_polygon(points.to_vec(), grey, Stroke::NONE));
        return;
    };
    let uv = picture.uv;
    let corners = [
        uv.left_top(),
        uv.right_top(),
        uv.right_bottom(),
        uv.left_bottom(),
    ];
    let mut mesh = Mesh::with_texture(picture.texture);
    for (pos, uv) in points.into_iter().zip(corners) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: tint,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// A picture drawn flat, `extent` points square, allocated in the layout.
///
/// For a texture reference, where a cube would claim the picture is a block.
/// No picture draws an empty frame, so a row keeps its shape while a texture
/// is still loading.
pub fn swatch(ui: &mut egui::Ui, extent: f32, picture: Option<Picture>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(extent), Sense::hover());
    let painter = ui.painter();
    if let Some(picture) = picture {
        painter.image(picture.texture, rect, picture.uv, Color32::WHITE);
    }
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, color::LINE_SOFT),
        egui::StrokeKind::Inside,
    );
    response
}

/// The size a cube is drawn at beside a row's value.
pub const ROW: f32 = 18.0;

/// The space a row gives up to show a cube before its value.
#[must_use]
pub fn row_width() -> f32 {
    ROW + 4.0
}

#[cfg(test)]
mod tests {
    use eframe::egui::vec2;

    use super::*;

    #[test]
    fn a_cube_fits_its_square_and_its_top_is_above_its_middle() {
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(20.0, 20.0));
        let [upper, right, centre, left, bottom, lower_left, lower_right] = corners(rect);
        for point in [upper, right, centre, left, bottom, lower_left, lower_right] {
            assert!(rect.expand(0.01).contains(point), "{point:?}");
        }
        assert!(upper.y < centre.y && bottom.y > centre.y);
        assert!(left.x < centre.x && right.x > centre.x);
    }
}
