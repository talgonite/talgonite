// Fullscreen darkness composite: max-combines ambient, HEA static light, and
// lantern masks, then mixes the scene color toward the darkness color.

struct LightSource {
    screen: vec2<f32>,
    mask_layer: u32,
    _pad: u32,
}

struct DarknessUniform {
    ambient: f32,
    light_count: u32,
    _pad0: u32,
    _pad1: u32,
    color: vec3<f32>,
    _pad2: f32,
    hea_offset: vec2<f32>,
    hea_size: vec2<f32>,
    mask_sizes: vec4<f32>,
    camera_pos: vec2<f32>,
    viewport: vec2<f32>,
    zoom: f32,
    _pad3: f32,
    _pad4: f32,
    lights: array<LightSource, 64>,
}

@group(0) @binding(0)
var t_scene: texture_2d<f32>;
@group(0) @binding(1)
var t_hea: texture_2d<f32>;
@group(0) @binding(2)
var s_hea: sampler;
@group(0) @binding(3)
var t_mask_small: texture_2d<f32>;
@group(0) @binding(4)
var t_mask_large: texture_2d<f32>;
@group(0) @binding(5)
var s_mask: sampler;
@group(0) @binding(6)
var<uniform> u_darkness: DarknessUniform;

const MAX_LIGHT: f32 = 32.0;
const HALF_TILE_HEIGHT: f32 = 14.0;

struct FullscreenVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> FullscreenVertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );

    var out: FullscreenVertexOutput;
    out.clip_position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    return out;
}

// Inverse of the scene's tile_to_screen transform.
fn screen_to_world(screen: vec2<f32>) -> vec2<f32> {
    return (screen - u_darkness.viewport * 0.5) / u_darkness.zoom
        + u_darkness.camera_pos
        + vec2<f32>(0.0, HALF_TILE_HEIGHT);
}

fn mask_value(uv: vec2<f32>, layer: u32) -> f32 {
    var size: vec2<f32>;
    if layer == 0u {
        size = u_darkness.mask_sizes.xy;
    } else {
        size = u_darkness.mask_sizes.zw;
    }

    if size.x <= 0.0 || size.y <= 0.0 {
        return 0.0;
    }

    let mask_uv = (uv + size * 0.5) / size;
    if layer == 0u {
        return textureSample(t_mask_small, s_mask, mask_uv).r * 255.0;
    } else {
        return textureSample(t_mask_large, s_mask, mask_uv).r * 255.0;
    }
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(position.xy);
    let screen = vec2<f32>(position.xy);

    // Ambient + static HEA light at this pixel's world position.
    let world = screen_to_world(screen);
    // Sample texel centers (uv = texel/size sits on a filter boundary);
    // out-of-bounds texels read as fully dark.
    let hea_texel = world + u_darkness.hea_offset;
    let hea_uv = (hea_texel + vec2<f32>(0.5, 0.5)) / u_darkness.hea_size;
    let sampled_light = textureSample(t_hea, s_hea, hea_uv).r * 255.0;
    let hea_in_bounds = hea_texel.x >= 0.0
        && hea_texel.y >= 0.0
        && hea_texel.x < u_darkness.hea_size.x
        && hea_texel.y < u_darkness.hea_size.y;
    let static_light = select(0.0, sampled_light, hea_in_bounds);
    var light = max(u_darkness.ambient, static_light);

    // Dynamic light sources, stamped in screen space.
    for (var i = 0u; i < u_darkness.light_count; i = i + 1u) {
        let src = u_darkness.lights[i];
        light = max(light, mask_value(screen - src.screen, src.mask_layer));
    }

    let darkness = clamp(1.0 - light / MAX_LIGHT, 0.0, 1.0);
    let scene = textureLoad(t_scene, coord, 0);
    return vec4<f32>(mix(scene.rgb, u_darkness.color, darkness), scene.a);
}
