struct Uniforms {
    model_view_projection: mat4x4<f32>,
    model: mat4x4<f32>,
    ambient: vec4<f32>,
    directional_direction: vec4<f32>,
    directional_color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var cube_texture: texture_2d<f32>;

@group(0) @binding(2)
var cube_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_position: vec3<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.model_view_projection * vec4<f32>(input.position, 1.0);
    output.world_position = (uniforms.model * vec4<f32>(input.position, 1.0)).xyz;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(cube_texture, cube_sampler, input.uv);
    let dx = dpdx(input.world_position);
    let dy = dpdy(input.world_position);
    let normal = normalize(cross(dx, dy));
    let light_direction = normalize(uniforms.directional_direction.xyz);
    let diffuse = max(dot(normal, -light_direction), 0.0);
    let ambient = uniforms.ambient.rgb * uniforms.ambient.a;
    let directional =
        uniforms.directional_color.rgb * uniforms.directional_direction.a * diffuse;
    return vec4<f32>(sampled.rgb * (ambient + directional), sampled.a);
}
