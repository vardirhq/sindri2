struct Uniforms {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) face: vec4<f32>,
    @location(7) outline: vec4<f32>,
    // x, y, width, height in normalized texture space.
    @location(8) uv_rect: vec4<f32>,
    // How the field is read:
    //   x  half-width of the outline, in stored field units
    //   y  extra softness added to the edge, in stored field units
    //   z  1 when the atlas holds this glyph's own colours instead of a field
    //   w  unused
    @location(9) shape: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) face: vec4<f32>,
    @location(2) outline: vec4<f32>,
    @location(3) shape: vec4<f32>,
}

// Where a glyph's edge is stored. Half, so the field has the same room either
// side of it; `sindri_render::glyph::EDGE` is the same constant.
const EDGE: f32 = 0.5;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var output: VertexOutput;
    output.position = uniforms.view_projection * model * vec4<f32>(input.position, 1.0);
    // The quad's own coordinates run 0..1; the rect says which part of the
    // atlas that maps onto.
    output.uv = input.uv_rect.xy + input.uv * input.uv_rect.zw;
    output.face = input.face;
    output.outline = input.outline;
    output.shape = input.shape;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(atlas_texture, atlas_sampler, input.uv);
    // A colour glyph is a picture with no edge to find, so it is drawn as it is
    // and the face colour only scales it.
    if (input.shape.z > 0.5) {
        return sampled * input.face;
    }

    let distance = sampled.a;
    // How much of the field one pixel of the screen covers right now. This is
    // the whole reason the atlas can be one bake: the edge is found at whatever
    // size the glyph is actually being drawn, rather than blurred at the size it
    // was rasterised. Floored so a glyph seen edge-on cannot divide by nothing.
    let pixel = max(fwidth(distance), 0.00001);
    let soften = pixel + input.shape.y;

    // Two thresholds. The face begins at the glyph's own edge; the outline
    // begins a stroke's width outside it, so an outline of nothing puts the two
    // in the same place and the arithmetic below collapses to the face alone.
    let face_edge = EDGE;
    let outline_edge = EDGE - input.shape.x;
    let in_face = smoothstep(face_edge - soften, face_edge + soften, distance);
    let in_ink = smoothstep(outline_edge - soften, outline_edge + soften, distance);

    let rgb = mix(input.outline.rgb, input.face.rgb, in_face);
    let alpha = in_ink * mix(input.outline.a, input.face.a, in_face);
    return vec4<f32>(rgb, alpha);
}
