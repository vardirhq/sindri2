// Bloom: the light a bright thing throws into the dark around it.
//
// Three passes over a full-screen triangle. The bright pass keeps what is
// brighter than a threshold, the blur pass spreads it, and the composite adds
// the result back over the scene. The chain runs at a quarter resolution
// because a blur is the one thing that is *better* for being cheap: the
// downsample is itself a blur, and the taps that follow reach four times as far
// across the picture for the same cost.
//
// Everything here is linear. The scene target is sRGB, so sampling it decodes;
// the chain's own targets are float, so nothing is re-encoded or clamped between
// passes; the composite writes to an sRGB target, which encodes once at the end.

struct Params {
    // xy: one texel of the source, in UV. zw: threshold and knee.
    source: vec4<f32>,
    // xy: blur direction, in texels. z: how much glow to add. w: unused.
    blur: vec4<f32>,
    // x: exposure stops, y: contrast, z: saturation, w: vignette.
    grade: vec4<f32>,
    // x: tone-map mode, y: bloom enabled. zw reserved.
    tone: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var source_texture: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

/// One triangle covering the viewport, built from the vertex index alone.
///
/// A triangle rather than two: the seam down the diagonal of a quad is a real
/// cost in a pass that runs three times a frame over every pixel, and there are
/// no vertex buffers to bind at all this way.
@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> VertexOutput {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    var out: VertexOutput;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, 1.0 - (y + 1.0) * 0.5);
    return out;
}

/// What is left of a colour once everything below the threshold is taken away.
///
/// Soft rather than a straight cutoff. A hard threshold makes the glow appear
/// and vanish as a thing's brightness crosses it, which on anything that pulses
/// or fades reads as a flicker; the knee is a band either side where the
/// contribution ramps in instead.
fn above_threshold(color: vec3<f32>, threshold: f32, knee: f32) -> vec3<f32> {
    let brightness = max(color.r, max(color.g, color.b));
    let width = max(knee, 1e-4);
    var soft = clamp(brightness - threshold + width, 0.0, 2.0 * width);
    soft = soft * soft / (4.0 * width);
    let kept = max(soft, brightness - threshold) / max(brightness, 1e-4);
    return color * kept;
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    if (params.tone.x < 0.5) {
        return color;
    }
    if (params.tone.x < 1.5) {
        return color / (vec3<f32>(1.0) + color);
    }
    // Narkowicz ACES approximation: compact, stable, and sufficient for a
    // world presentation curve without pulling a colour-management system in.
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn grade_world(color: vec3<f32>) -> vec3<f32> {
    var graded = color * exp2(params.grade.x);
    graded = tone_map(graded);
    graded = (graded - vec3<f32>(0.5)) * params.grade.y + vec3<f32>(0.5);
    let luma = dot(graded, vec3<f32>(0.2126, 0.7152, 0.0722));
    graded = mix(vec3<f32>(luma), graded, params.grade.z);
    return max(graded, vec3<f32>(0.0));
}

@fragment
fn fs_bright(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = grade_world(textureSample(source_texture, source_sampler, in.uv).rgb);
    return vec4<f32>(above_threshold(color, params.source.z, params.source.w), 1.0);
}

// A nine-tap Gaussian, run once across and once down. Separable, so the two
// passes together cost eighteen taps rather than the eighty-one a single
// two-dimensional kernel of the same reach would.
const WEIGHTS: array<f32, 5> = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);

@fragment
fn fs_blur(in: VertexOutput) -> @location(0) vec4<f32> {
    let step = params.blur.xy * params.source.xy;
    var total = textureSample(source_texture, source_sampler, in.uv).rgb * WEIGHTS[0];
    for (var i = 1; i < 5; i = i + 1) {
        let offset = step * f32(i);
        total = total + textureSample(source_texture, source_sampler, in.uv + offset).rgb
            * WEIGHTS[i];
        total = total + textureSample(source_texture, source_sampler, in.uv - offset).rgb
            * WEIGHTS[i];
    }
    return vec4<f32>(total, 1.0);
}

@group(1) @binding(0) var glow_texture: texture_2d<f32>;

/// The scene with its own light added back over it.
///
/// Added rather than mixed: a glow is light arriving at the eye on top of what
/// is already there, so a bright edge over black gets brighter and the black
/// around it lifts. Mixing would wash the edge out towards the average instead,
/// which is fog.
@fragment
fn fs_composite(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(source_texture, source_sampler, in.uv);
    let glow = textureSample(glow_texture, source_sampler, in.uv).rgb * params.tone.y;
    var color = grade_world(scene.rgb) + glow * params.blur.z;

    let centered = in.uv * 2.0 - vec2<f32>(1.0);
    let edge = smoothstep(0.25, 1.15, dot(centered, centered));
    color = color * (1.0 - edge * params.grade.w);
    return vec4<f32>(color, scene.a);
}
