// Shapes drawn as signed distance fields on a unit quad.
//
// The same trick the glyphs use, for the same reason: the distance says where
// the edge is, so `fwidth` finds it exactly at whatever size the quad ended up
// on screen. One antialiased edge at any zoom, and no texture at all.
//
// It also makes the modifiers free. A ring is a circle whose fill is
// transparent; an arc is a ring with a sweep; a dashed ring is an arc with a
// duty cycle. None of them is a second pipeline or a second asset, which is
// what makes an orbit whose radius grows with an upgrade a number rather than
// an art request.

struct Camera {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) fill: vec4<f32>,
    @location(7) stroke: vec4<f32>,
    // kind, sides (or grid cells), stroke width, corner radius
    @location(8) geometry: vec4<f32>,
    // dash count, dash duty, sweep start, sweep turns
    @location(9) pattern: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) stroke: vec4<f32>,
    @location(3) geometry: vec4<f32>,
    @location(4) pattern: vec4<f32>,
};

const TAU: f32 = 6.283185307179586;

/// Room left around a shape for the antialiasing to fade out in, as a fraction
/// of the shape.
///
/// A shape is inscribed in its quad — a polygon's vertices sit at radius 0.5,
/// exactly on the edge — so anything drawn *outside* the outline has nowhere to
/// land. The stroke is the obvious victim, and it is centred on the edge, so
/// half of it falls off: every vertex is sliced flat, a rectangle loses half its
/// stroke on all four sides, and a circle picks up four flat spots at the
/// compass points. The quad is grown to make room, which is exactly what the
/// glyph atlas does with its field's spread and for exactly this reason.
const EDGE_BLEED: f32 = 0.02;

const KIND_RECT: i32 = 0;
const KIND_ELLIPSE: i32 = 1;
const KIND_POLYGON: i32 = 2;
const KIND_GRID: i32 = 3;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );
    // Grown by whatever the stroke needs plus a little for the fade, so the
    // shape is still exactly the size it was authored at and everything drawn
    // around its edge has somewhere to go.
    let bleed = instance.geometry.z * 0.5 + EDGE_BLEED;
    let corner = vertex.position.xy * (1.0 + bleed * 2.0);

    var out: VertexOutput;
    out.clip_position =
        camera.view_projection * model * vec4<f32>(corner, vertex.position.z, 1.0);
    // Shape space: the shape itself still spans -0.5 to 0.5, so every distance
    // below is a fraction of the shape's own size and scales with it. Past that
    // is the bleed.
    out.local = corner;
    out.fill = instance.fill;
    out.stroke = instance.stroke;
    out.geometry = instance.geometry;
    out.pattern = instance.pattern;
    return out;
}

/// Distance to a rounded rectangle inscribed in the quad.
fn rect_distance(p: vec2<f32>, radius: f32) -> f32 {
    let r = clamp(radius, 0.0, 0.5);
    let q = abs(p) - (vec2<f32>(0.5, 0.5) - vec2<f32>(r, r));
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

/// Distance to a regular polygon of `sides`, inscribed in the quad.
///
/// Measured to the nearest edge's plane rather than to the outline itself,
/// which is exact inside the shape and a hair short of it just outside a
/// vertex. At the widths anything here is stroked at, the difference is well
/// under a pixel.
fn polygon_distance(p: vec2<f32>, sides: f32) -> f32 {
    let n = max(sides, 3.0);
    let segment = TAU / n;
    // Point up. The fold below measures to the nearest edge's *normal*, so
    // aligning "up" with a normal puts a flat edge on top and a vertex at the
    // bottom — which is an upside-down triangle. Half a segment past that is a
    // vertex, which is how every shape in the reference sits.
    let angle = atan2(p.y, p.x) - TAU * 0.25 + segment * 0.5;
    let folded = angle - segment * round(angle / segment);
    return length(p) * cos(folded) - 0.5 * cos(segment * 0.5);
}

/// Distance to the nearest line of a grid of `cells` across the quad.
fn grid_distance(p: vec2<f32>, cells: f32) -> f32 {
    let n = max(cells, 1.0);
    let q = p * n;
    let to_line = abs(q - round(q));
    return min(to_line.x, to_line.y) / n;
}

/// Where a point sits around the shape, from zero at the top and increasing
/// clockwise, so an arc authored as "a quarter turn from the top" is what
/// appears.
fn outline_turn(p: vec2<f32>) -> f32 {
    return fract((atan2(p.x, p.y) / TAU) + 1.0);
}

/// How much of the outline survives its sweep and its dashes, at this point.
///
/// Both are angular, so both are answered from where the point sits around the
/// shape. A shape with neither returns 1 and costs two comparisons.
///
/// `t` and `turn_width` are measured by the caller rather than here, because
/// this is reached under a branch and a derivative may only be taken in uniform
/// control flow -- see the note in `fs_main`.
fn outline_pattern(t: f32, turn_width: f32, pattern: vec4<f32>) -> f32 {
    let dashes = pattern.x;
    let turns = pattern.w;
    if dashes <= 0.0 && turns >= 1.0 {
        return 1.0;
    }
    var alive = 1.0;
    if turns < 1.0 {
        let travelled = fract(t - pattern.z + 1.0);
        if travelled > max(turns, 0.0) {
            alive = 0.0;
        }
    }
    if dashes > 0.0 {
        // One dash is `dashes` turns wide, so the phase advances that much
        // faster than the turn it is measured from.
        //
        // Clamped because `fract` turns over once per dash and its derivative
        // spikes at the seam; unclamped, one dash boundary per revolution
        // smears across the whole gap.
        let soft = min(turn_width * dashes, 0.25);
        let duty = clamp(pattern.y, 0.0, 1.0);
        alive = alive * smoothstep(duty + soft, duty - soft, fract(t * dashes));
    }
    return alive;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let kind = i32(in.geometry.x + 0.5);
    let p = in.local;

    var distance: f32;
    var closed = true;
    if kind == KIND_ELLIPSE {
        distance = length(p) - 0.5;
    } else if kind == KIND_POLYGON {
        distance = polygon_distance(p, in.geometry.y);
    } else if kind == KIND_GRID {
        distance = grid_distance(p, in.geometry.y);
        // A grid is lines, not an enclosure: there is no inside to fill, and
        // the distance is already unsigned.
        closed = false;
    } else {
        distance = rect_distance(p, in.geometry.w);
    }

    // The edge's own width on screen, so the antialiasing is a pixel wide
    // wherever the quad landed and however far the camera has zoomed.
    //
    // This and the turn below are the shader's two derivatives, and both are
    // taken here, at the top of the function, because WGSL allows a derivative
    // only in uniform control flow: a quad's fragments have to reach it
    // together for the difference between them to mean anything. Taken inside
    // the stroke branch instead -- which is where the dash pattern wants it --
    // it is a validation error that a browser enforces and native does not, so
    // the pipeline compiled here and was rejected there, leaving every shape
    // undrawn.
    let soft = max(fwidth(distance), 1e-6);
    let turn = outline_turn(p);
    let turn_width = fwidth(turn);

    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if closed {
        let inside = 1.0 - smoothstep(-soft, soft, distance);
        color = vec4<f32>(in.fill.rgb, in.fill.a * inside);
    }

    let width = in.geometry.z;
    if width > 0.0 {
        // A band centred on the edge, so a stroke grows evenly either side of
        // the outline rather than eating the fill or floating off it.
        let half = width * 0.5;
        var edge = 1.0 - smoothstep(half - soft, half + soft, abs(distance));
        if closed {
            edge = edge * outline_pattern(turn, turn_width, in.pattern);
        }
        let alpha = in.stroke.a * edge;
        // Over the fill rather than added to it, so a translucent stroke on a
        // translucent fill is the stroke's colour and not a mixture.
        color = vec4<f32>(
            mix(color.rgb, in.stroke.rgb, alpha),
            color.a + alpha * (1.0 - color.a),
        );
    }

    if color.a <= 0.0 {
        discard;
    }
    return vec4<f32>(color.rgb * color.a, color.a);
}
