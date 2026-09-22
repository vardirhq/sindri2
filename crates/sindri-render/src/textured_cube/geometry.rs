use crate::TexturedVertex;

const FACE_UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

pub(super) const VERTICES: [TexturedVertex; 24] = [
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[3]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, 1.0, 1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, 1.0, 1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, 1.0, -1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, 1.0, -1.0], FACE_UVS[3]),
    TexturedVertex::new([-1.0, -1.0, -1.0], FACE_UVS[0]),
    TexturedVertex::new([1.0, -1.0, -1.0], FACE_UVS[1]),
    TexturedVertex::new([1.0, -1.0, 1.0], FACE_UVS[2]),
    TexturedVertex::new([-1.0, -1.0, 1.0], FACE_UVS[3]),
];

pub(super) const INDICES: [u16; 36] = [
    0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16, 17, 18,
    18, 19, 16, 20, 21, 22, 22, 23, 20,
];
