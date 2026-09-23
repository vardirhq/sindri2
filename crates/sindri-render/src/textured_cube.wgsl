struct Uniforms {
    model_view_projection: mat4x4<f32>,
    model: mat4x4<f32>,
    ambient: vec4<f32>,
    directional_direction: vec4<f32>,
    directional_color: vec4<f32>,
    light_view_projection: mat4x4<f32>,
    shadow: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    camera_position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var cube_texture: texture_2d<f32>;

@group(0) @binding(2)
var cube_sampler: sampler;

@group(0) @binding(3)
var shadow_map: texture_depth_2d;

@group(0) @binding(4)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(14) ambient_occlusion: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) ambient_occlusion: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.model_view_projection * vec4<f32>(input.position, 1.0);
    output.world_position = (uniforms.model * vec4<f32>(input.position, 1.0)).xyz;
    output.uv = input.uv;
    output.ambient_occlusion = input.ambient_occlusion;
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
    let shadow_clip = uniforms.light_view_projection * vec4<f32>(input.world_position, 1.0);
    let shadow_ndc = shadow_clip.xyz / shadow_clip.w;
    let shadow_uv = vec2<f32>(shadow_ndc.x * 0.5 + 0.5, 0.5 - shadow_ndc.y * 0.5);
    let sampled_visibility = textureSampleCompare(
        shadow_map,
        shadow_sampler,
        shadow_uv,
        shadow_ndc.z - uniforms.shadow.x,
    );
    let inside_shadow_map = all(shadow_uv >= vec2<f32>(0.0))
        && all(shadow_uv <= vec2<f32>(1.0))
        && shadow_ndc.z >= 0.0
        && shadow_ndc.z <= 1.0;
    let receives_shadow = uniforms.shadow.y > 0.5 && diffuse > 0.0 && inside_shadow_map;
    let visibility = select(1.0, sampled_visibility, receives_shadow);
    let directional = uniforms.directional_color.rgb
        * uniforms.directional_direction.a * diffuse * visibility;
    let contact_visibility = mix(1.0, input.ambient_occlusion, uniforms.shadow.z);
    let lit = sampled.rgb * (ambient + directional) * contact_visibility;
    let camera_distance = distance(input.world_position, uniforms.camera_position.xyz);
    let fog_distance = max(camera_distance - uniforms.fog_params.x, 0.0);
    let distance_fog = clamp(fog_distance / max(uniforms.fog_params.y, 0.0001), 0.0, 1.0);
    let exponential_fog = 1.0 - exp(-uniforms.fog_params.z * fog_distance);
    let height_fog = clamp(
        (uniforms.fog_params.w - input.world_position.y) * uniforms.camera_position.w,
        0.0,
        1.0,
    );
    let fog_amount = clamp(max(distance_fog, exponential_fog) + height_fog, 0.0, 1.0)
        * uniforms.fog_color.a;
    return vec4<f32>(mix(lit, uniforms.fog_color.rgb, fog_amount), sampled.a);
}
