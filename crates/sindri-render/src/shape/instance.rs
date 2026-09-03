//! One shape as the GPU sees it.

use glam::Mat4;

use super::Shape;

/// A single shape quad: what it is, what fills it, and what strokes it.
///
/// Every distance on it — stroke width, corner radius — is a fraction of the
/// shape's own size, so one instance scaled up is the same drawing larger
/// rather than a thicker-lined version of it.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeInstance {
    model: [[f32; 4]; 4],
    fill: [f32; 4],
    stroke: [f32; 4],
    /// kind, sides or grid cells, stroke width, corner radius.
    geometry: [f32; 4],
    /// dash count, dash duty, sweep start, sweep turns.
    pattern: [f32; 4],
}

impl ShapeInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4,
        9 => Float32x4
    ];

    /// A filled shape with no stroke.
    ///
    /// The starting point rather than the common case: most of what this draws
    /// is an outline, which is [`Self::stroked`] or this with a transparent
    /// fill. Both exist because "a filled disc" and "a ring" are different
    /// enough that spelling out which one is meant reads better than a colour
    /// with a zero in it.
    #[must_use]
    pub fn filled(model: Mat4, kind: Shape, fill: [f32; 4]) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            fill,
            stroke: [0.0; 4],
            geometry: [kind.tag(), kind.parameter(), 0.0, 0.0],
            // A full turn of undashed outline: the shape of "no pattern", so a
            // caller that never mentions dashes never pays for them.
            pattern: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// An outline with nothing inside it.
    #[must_use]
    pub fn stroked(model: Mat4, kind: Shape, width: f32, color: [f32; 4]) -> Self {
        Self::filled(model, kind, [0.0; 4]).with_stroke(width, color)
    }

    /// Draws a stroke of `width` — a fraction of the shape's size — on its edge.
    #[must_use]
    pub const fn with_stroke(mut self, width: f32, color: [f32; 4]) -> Self {
        self.geometry[2] = width;
        self.stroke = color;
        self
    }

    /// Rounds a rectangle's corners by a fraction of its size. Ignored by every
    /// other kind, which has no corners to round.
    #[must_use]
    pub const fn with_corner_radius(mut self, radius: f32) -> Self {
        self.geometry[3] = radius;
        self
    }

    /// Breaks the outline into `count` evenly spaced dashes, each covering
    /// `duty` of its share of the way round.
    ///
    /// A count rather than a dash length, because what an author is choosing is
    /// how many ticks are on the dial — and a length would have to be respaced
    /// by hand every time the shape's size changed.
    #[must_use]
    pub const fn dashed(mut self, count: f32, duty: f32) -> Self {
        self.pattern[0] = count;
        self.pattern[1] = duty;
        self
    }

    /// Draws only `turns` of the outline, starting `start` of the way round
    /// from the top and going clockwise. One turn is the whole shape.
    ///
    /// This is a progress arc, a charge meter, and a cooldown ring.
    #[must_use]
    pub const fn swept(mut self, start: f32, turns: f32) -> Self {
        self.pattern[2] = start;
        self.pattern[3] = turns;
        self
    }

    /// The per-instance model transform.
    #[must_use]
    pub fn model(self) -> Mat4 {
        Mat4::from_cols_array_2d(&self.model)
    }

    /// The colour inside the shape.
    #[must_use]
    pub const fn fill(self) -> [f32; 4] {
        self.fill
    }

    /// The colour of its outline, and how wide that outline is.
    #[must_use]
    pub const fn stroke(self) -> ([f32; 4], f32) {
        (self.stroke, self.geometry[2])
    }

    #[must_use]
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ShapeUniform {
    pub(super) view_projection: [[f32; 4]; 4],
}

#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::{Shape, ShapeInstance};

    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

    /// These are values written straight into a slot and read straight back,
    /// so the comparison really is exact — but a float is a float, and a
    /// tolerance costs nothing and keeps the lint honest rather than silenced.
    #[track_caller]
    fn same(left: &[f32], right: &[f32]) {
        assert_eq!(left.len(), right.len(), "{left:?} against {right:?}");
        for (a, b) in left.iter().zip(right) {
            assert!((a - b).abs() < 1.0e-6, "{left:?} against {right:?}");
        }
    }

    /// A shape with no modifiers is a full undashed turn of outline.
    ///
    /// The default matters more than it looks: the shader reads the sweep as
    /// "how much of the way round to draw", so a zero there would draw nothing
    /// at all and every plain ring in every scene would be invisible.
    #[test]
    fn a_plain_shape_draws_all_the_way_round() {
        let ring = ShapeInstance::stroked(Mat4::IDENTITY, Shape::Ellipse, 0.05, RED);
        same(&[ring.pattern[0]], &[0.0]);
        same(&[ring.pattern[3]], &[1.0]);
        let (color, width) = ring.stroke();
        same(&color, &RED);
        same(&[width], &[0.05]);
        same(&ring.fill(), &[0.0; 4]);
    }

    /// The modifiers compose, and each lands in its own slot.
    #[test]
    fn modifiers_stack_without_overwriting_each_other() {
        let dial = ShapeInstance::stroked(Mat4::IDENTITY, Shape::Ellipse, 0.04, RED)
            .dashed(12.0, 0.5)
            .swept(0.25, 0.5);
        same(&dial.pattern, &[12.0, 0.5, 0.25, 0.5]);
        let (color, width) = dial.stroke();
        same(&color, &RED);
        same(&[width], &[0.04]);
    }

    /// A filled shape can carry a stroke too, which is what a card is.
    #[test]
    fn a_shape_can_be_filled_and_stroked_at_once() {
        let card = ShapeInstance::filled(Mat4::IDENTITY, Shape::Rect, [0.1, 0.1, 0.1, 1.0])
            .with_corner_radius(0.2)
            .with_stroke(0.03, RED);
        same(&card.fill(), &[0.1, 0.1, 0.1, 1.0]);
        let (color, width) = card.stroke();
        same(&color, &RED);
        same(&[width], &[0.03]);
        same(&[card.geometry[3]], &[0.2]);
    }

    /// The kind and its one number reach the GPU, so a hexagon is not a pentagon.
    #[test]
    fn the_kind_reaches_the_instance() {
        let hex = ShapeInstance::filled(Mat4::IDENTITY, Shape::Polygon { sides: 6.0 }, RED);
        same(
            &[hex.geometry[0], hex.geometry[1]],
            &[Shape::Polygon { sides: 6.0 }.tag(), 6.0],
        );
    }
}
