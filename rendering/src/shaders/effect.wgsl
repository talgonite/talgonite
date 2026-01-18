struct Camera {
    view_proj: mat4x4<f32>,
    position: vec2<f32>,
    xray_size: f32,
    tint: vec3<f32>,
}
@group(1) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}
struct InstanceInput {
    @location(5) position: vec3<f32>,
    @location(6) tex_min: vec2<f32>,
    @location(7) tex_max: vec2<f32>,
    @location(8) sprite_size: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let scaled_position = model.position * instance.sprite_size;
    let position = vec3<f32>(scaled_position, 0.0) + instance.position;

    var out: VertexOutput;
    out.tex_coords = mix(instance.tex_min, instance.tex_max, model.tex_coords);
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    if color.a < 0.01 {
        discard;
    }
    return color * vec4<f32>(1.0, 1.0, 1.0, 0.5);
}
