struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.72),
        vec2<f32>(-0.68, -0.55),
        vec2<f32>(0.68, -0.55),
    );
    var colors = array<vec3<f32>, 3>(
        vec3<f32>(0.96, 0.45, 0.18),
        vec3<f32>(0.22, 0.78, 0.94),
        vec3<f32>(0.65, 0.32, 0.95),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[index], 0.0, 1.0);
    output.color = colors[index];
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}

