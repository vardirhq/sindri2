struct Uniforms {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var sprite_texture: texture_2d<f32>;

@group(0) @binding(2)
var sprite_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) tint: vec4<f32>,
    // x, y, width, height in normalized texture space.
    @location(7) uv_rect: vec4<f32>,
    @location(8) color_multiply: vec4<f32>,
    @location(9) color_offset: vec4<f32>,
    // Brightness at each corner, from the quad's own [0,0] round to [0,1].
    @location(10) corner_shade: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) color_multiply: vec4<f32>,
    @location(3) color_offset: vec4<f32>,
    @location(4) shade: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    var output: VertexOutput;
    output.position = uniforms.view_projection * model * vec4<f32>(input.position, 1.0);
    // The quad's own coordinates run 0..1; the rect says which part of
    // the texture that maps onto, so the whole texture is 0, 0, 1, 1.
    output.uv = input.uv_rect.xy + input.uv * input.uv_rect.zw;
    output.tint = input.tint;
    output.color_multiply = input.color_multiply;
    output.color_offset = input.color_offset;
    // The quad's own coordinates run -0.5..0.5, so this is where the vertex
    // sits across it. Bilinear between the four corner values, interpolated
    // over the face for free by the rasteriser.
    let across = input.position.xy + vec2<f32>(0.5, 0.5);
    let lower = mix(input.corner_shade.x, input.corner_shade.y, across.x);
    let upper = mix(input.corner_shade.w, input.corner_shade.z, across.x);
    output.shade = mix(lower, upper, across.y);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(sprite_texture, sprite_sampler, input.uv);
    let colour = sampled * input.tint * input.color_multiply + input.color_offset;
    return vec4<f32>(colour.rgb * input.shade, colour.a);
}

// Opaque geometry, which writes depth.
//
// The discard is what keeps a cutout honest: a fully transparent pixel that
// wrote depth would hide whatever is behind it while showing nothing itself,
// so the shape a texture cuts out would punch a hole in the world.
@fragment
fn fs_solid(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(sprite_texture, sprite_sampler, input.uv);
    let color = sampled * input.tint * input.color_multiply + input.color_offset;
    if color.a < 0.5 {
        discard;
    }
    return vec4<f32>(color.rgb * input.shade, 1.0);
}
