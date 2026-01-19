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
    @location(9) palette_offset: f32,
    @location(10) dye_v_offset: f32,
    @location(11) flags: u32,
    @location(12) tint: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) palette_offset: f32,
    @location(2) dye_v_offset: f32,
    @location(3) world_position: vec2<f32>,
    @location(4) flags: u32,
    @location(5) instance_z: f32,
    @location(6) local_y: f32,
    @location(7) normalized_y: f32,
    @location(8) tint: vec3<f32>,
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
    out.palette_offset = instance.palette_offset;
    out.dye_v_offset = instance.dye_v_offset;
    out.world_position = position.xy;
    out.flags = instance.flags;
    out.instance_z = position.z;
    out.local_y = scaled_position.y;
    out.normalized_y = model.tex_coords.y;
    out.tint = instance.tint;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_palette: texture_2d<f32>;
@group(0) @binding(3)
var s_palette: sampler;
@group(0) @binding(4)
var t_dye_palette: texture_2d<f32>;

@group(2) @binding(0)
var t_depth: texture_depth_2d;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.palette_offset < 0.0 {
        discard; 
    }
    
    let color_index = textureSampleBaseClampToEdge(t_diffuse, s_diffuse, in.tex_coords).r;
    if color_index == 0 {
        discard;
    }
    
    let palette_uv = vec2<f32>(color_index, in.palette_offset);
    var final_color = textureSampleBaseClampToEdge(t_palette, s_palette, palette_uv);

    if (in.dye_v_offset >= 0.0) {
        let dye_uv = vec2<f32>(color_index, in.dye_v_offset);
        let dye_color = textureSampleBaseClampToEdge(t_dye_palette, s_palette, dye_uv);
        final_color = mix(final_color, dye_color, dye_color.a);
    }

    // Desaturation effect based on tile distance
    let delta = in.world_position - camera.position;

    // Convert screen pixels to tile coordinates (isometric projection)
    // TILE_WIDTH_HALF = 28.0, TILE_HEIGHT_HALF = 14.0
    let a = delta.x / 28.0;
    let b = delta.y / 14.0;
    let tile_dx = (a + b) * 0.5;
    let tile_dy = (b - a) * 0.5;
    
    // Use Euclidean distance (circle in tile space, ellipse on screen)
    let dist = length(vec2<f32>(tile_dx, tile_dy));

    // X-Ray effect for local player (only when xray_size > 0)
    if (in.flags & 1u) != 0u && camera.xray_size > 0.0 {
        // Use screen coords to calculate player position
        let a = camera.position.x / 28.0;
        let b = camera.position.y / 14.0;
        let player_tile_x = (a + b) * 0.5;
        let player_tile_y = (b - a) * 0.5;
        let z_to_check = (player_tile_x
            + player_tile_y
            + 0.75 /* Offset to be in front of player */
            ) / 65536.0 + 0.000001;

        // Calculate player Z
        let wall_z = in.instance_z;

        if wall_z > z_to_check {
            var dist_from_bottom = 1000.0;
            if in.normalized_y > 0.001 {
                dist_from_bottom = in.local_y * (1.0 - in.normalized_y) / in.normalized_y;
            }
            // Fade out X-ray over the bottom 20 pixels (from 10px to 30px)
            let bottom_fade = smoothstep(10.0, 30.0, dist_from_bottom);

            let xray_pos = delta - vec2<f32>(0.0, -20.0);
            // Scale ellipse dimensions by xray_size (base: 20px width, 30px height)
            let scaled_width = 20.0 * camera.xray_size;
            let scaled_height = 30.0 * camera.xray_size;
            let xray_dist = length(vec2<f32>(xray_pos.x / scaled_width, xray_pos.y / scaled_height));

            // Bayer 8x8 ordered dither for smooth transparency
            let bayer_8x8 = array<f32, 64>(
                0.0, 32.0, 8.0, 40.0, 2.0, 34.0, 10.0, 42.0,
                48.0, 16.0, 56.0, 24.0, 50.0, 18.0, 58.0, 26.0,
                12.0, 44.0, 4.0, 36.0, 14.0, 46.0, 6.0, 38.0,
                60.0, 28.0, 52.0, 20.0, 62.0, 30.0, 54.0, 22.0,
                3.0, 35.0, 11.0, 43.0, 1.0, 33.0, 9.0, 41.0,
                51.0, 19.0, 59.0, 27.0, 49.0, 17.0, 57.0, 25.0,
                15.0, 47.0, 7.0, 39.0, 13.0, 45.0, 5.0, 37.0,
                63.0, 31.0, 55.0, 23.0, 61.0, 29.0, 53.0, 21.0
            );
            let coord = vec2<u32>(abs(in.world_position));
            let bayer_idx = (coord.y % 8u) * 8u + (coord.x % 8u);
            let threshold = bayer_8x8[bayer_idx] / 64.0;

            let opacity = mix(1.0, smoothstep(0.7, 1.4, xray_dist), bottom_fade);

            if opacity < threshold {
                discard;
            }
        }
    }
    
    let inner_radius = 9.0;
    let outer_radius = 13.0;
    let factor = smoothstep(inner_radius, outer_radius, dist);

    // Convert to grayscale
    let gray = dot(final_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    // Mix between full color and slightly dimmed grayscale
    var out_rgb = mix(final_color.rgb, vec3<f32>(gray) * 0.5, factor);

    return vec4<f32>(out_rgb.rgb + camera.tint + in.tint, final_color.a);
}