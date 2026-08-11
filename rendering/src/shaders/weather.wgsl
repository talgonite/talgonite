// Screen-space weather overlay: instanced quads for snow particles and rain
// rows, sampled from the snow atlas or the rain texture.

struct WeatherUniform {
    viewport: vec2<f32>,
    _pad0: vec2<f32>,
}

@group(0) @binding(0)
var t_snow: texture_2d<f32>;
@group(0) @binding(1)
var t_rain: texture_2d<f32>;
@group(0) @binding(2)
var s_snow: sampler;
@group(0) @binding(3)
var s_rain: sampler;
@group(0) @binding(4)
var<uniform> u_weather: WeatherUniform;

struct VertexInput {
    @location(6) corner: vec2<f32>,
}

struct InstanceInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) kind: u32,
    @location(5) _pad: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) kind: u32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let screen = instance.pos + vertex.corner * instance.size;
    let clip = vec2<f32>(
        screen.x / u_weather.viewport.x * 2.0 - 1.0,
        1.0 - screen.y / u_weather.viewport.y * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    out.uv = mix(instance.uv_min, instance.uv_max, vertex.corner);
    out.kind = instance.kind;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.kind == 0u {
        return textureSample(t_snow, s_snow, in.uv);
    } else {
        return textureSample(t_rain, s_rain, in.uv);
    }
}
