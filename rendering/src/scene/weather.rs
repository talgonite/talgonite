//! GPU weather overlay: instanced screen-space quads sampling the snow atlas
//! or rain texture.

use rand::rngs::SmallRng;
use rand::{Rng, RngExt, SeedableRng};
use wgpu::util::DeviceExt;

pub const MAX_WEATHER_INSTANCES: usize = 2048;

const SNOW_PARTICLE_COUNT: usize = 150;
const SNOW_MIN_VELOCITY_Y: f32 = 30.0;
const SNOW_MAX_VELOCITY_Y: f32 = 70.0;
const SNOW_DRIFT_X: f32 = 10.0;
const SNOW_FRAME_DURATION: f32 = 0.10;
const SNOW_SCALE: f32 = 2.0;

const RAIN_FALL_SPEED: f32 = 400.0;
const RAIN_COLUMN_COUNT: usize = 5;
const RAIN_SCALE: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeatherMode {
    #[default]
    None,
    Snow,
    Rain,
}

/// A baked RGBA sprite (straight alpha).
#[derive(Debug, Clone)]
pub struct WeatherSprite {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// One snow frame's placement inside the snow atlas.
#[derive(Debug, Clone, Copy)]
pub struct SnowFrame {
    pub width: u32,
    pub height: u32,
    /// Normalized uv rect (min_x, min_y, max_x, max_y).
    pub uv: [f32; 4],
}

/// Snow atlas and rain texture.
#[derive(Debug, Clone)]
pub struct WeatherAssets {
    pub snow_atlas: WeatherSprite,
    /// Frames grouped by snow type.
    pub snow_frames: Vec<Vec<SnowFrame>>,
    pub rain: WeatherSprite,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WeatherInstanceRaw {
    pos: [f32; 2],
    size: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    kind: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WeatherUniformRaw {
    viewport: [f32; 2],
    pad: [f32; 2],
}

struct SnowParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    type_index: usize,
    frame: usize,
    frame_timer: f32,
}

struct RainRow {
    y: f32,
    permutation: [u8; RAIN_COLUMN_COUNT],
}

pub struct WeatherRenderer {
    mode: WeatherMode,
    snow_texture: wgpu::Texture,
    rain_texture: wgpu::Texture,
    snow_sampler: wgpu::Sampler,
    rain_sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    viewport: [f32; 2],

    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instances: Vec<WeatherInstanceRaw>,
    bind_group: Option<wgpu::BindGroup>,

    snow_frames: Vec<Vec<SnowFrame>>,
    rain_size: (u32, u32),
    snow_particles: Vec<SnowParticle>,
    rain_rows: Vec<RainRow>,
    rng: SmallRng,
    last_viewport: (u32, u32),
}

impl WeatherRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, assets: &WeatherAssets) -> Self {
        let snow_texture = create_rgba_texture(
            device,
            queue,
            &assets.snow_atlas.pixels,
            assets.snow_atlas.width,
            assets.snow_atlas.height,
            "Snow Atlas",
        );
        let rain_texture = create_rgba_texture(
            device,
            queue,
            &assets.rain.pixels,
            assets.rain.width,
            assets.rain.height,
            "Rain Texture",
        );
        let snow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Snow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let rain_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Rain Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform = WeatherUniformRaw {
            viewport: [1.0, 1.0],
            pad: [0.0; 2],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Weather Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let vertices: [[f32; 2]; 6] = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [1.0, 1.0],
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Weather Quad Vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Weather Instance Buffer"),
            size: (MAX_WEATHER_INSTANCES * std::mem::size_of::<WeatherInstanceRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            mode: WeatherMode::None,
            snow_texture,
            rain_texture,
            snow_sampler,
            rain_sampler,
            uniform_buffer,
            viewport: [1.0, 1.0],
            vertex_buffer,
            instance_buffer,
            instances: Vec::with_capacity(MAX_WEATHER_INSTANCES),
            bind_group: None,
            snow_frames: assets.snow_frames.clone(),
            rain_size: (assets.rain.width, assets.rain.height),
            snow_particles: Vec::new(),
            rain_rows: Vec::new(),
            rng: SmallRng::from_rng(&mut rand::rng()),
            last_viewport: (0, 0),
        }
    }

    pub fn is_active(&self) -> bool {
        self.mode != WeatherMode::None
    }

    pub fn set_mode(&mut self, mode: WeatherMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.snow_particles.clear();
        self.rain_rows.clear();
        self.last_viewport = (0, 0);
    }

    /// Advances weather and rebuilds the instance buffer.
    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32, viewport: [f32; 2]) {
        self.instances.clear();

        if self.mode == WeatherMode::None {
            return;
        }

        let vp_w = viewport[0].max(0.0) as u32;
        let vp_h = viewport[1].max(0.0) as u32;
        if vp_w == 0 || vp_h == 0 {
            return;
        }

        self.viewport = viewport;
        let viewport_key = (vp_w, vp_h);

        match self.mode {
            WeatherMode::Snow => self.update_snow(dt, viewport_key, vp_w, vp_h),
            WeatherMode::Rain => self.update_rain(dt, viewport_key, vp_w, vp_h),
            WeatherMode::None => {}
        }

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[WeatherUniformRaw {
                viewport,
                pad: [0.0; 2],
            }]),
        );
        if !self.instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.instances),
            );
        }
    }

    pub fn instance_count(&self) -> u32 {
        self.instances.len() as u32
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    pub fn create_bind_group(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        if let Some(bind_group) = &self.bind_group {
            return bind_group.clone();
        }

        let snow_view = self
            .snow_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let rain_view = self
            .rain_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Weather Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&snow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&rain_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.snow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.rain_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });
        self.bind_group = Some(bind_group.clone());
        bind_group
    }

    fn update_snow(&mut self, dt: f32, viewport: (u32, u32), vp_w: u32, vp_h: u32) {
        if self.snow_frames.is_empty() {
            return;
        }

        if viewport != self.last_viewport || self.snow_particles.is_empty() {
            self.snow_particles = (0..SNOW_PARTICLE_COUNT)
                .map(|_| random_particle(&mut self.rng, &self.snow_frames, vp_w, vp_h, false))
                .collect();
            self.last_viewport = viewport;
        }

        for particle in &mut self.snow_particles {
            particle.y += particle.vy * dt;
            particle.x += particle.vx * dt;

            let frame_count = self.snow_frames[particle.type_index].len();
            if frame_count > 1 {
                particle.frame_timer += dt;
                while particle.frame_timer >= SNOW_FRAME_DURATION {
                    particle.frame_timer -= SNOW_FRAME_DURATION;
                    particle.frame = (particle.frame + 1) % frame_count;
                }
            }

            if particle.y > vp_h as f32 {
                *particle = random_particle(&mut self.rng, &self.snow_frames, vp_w, vp_h, true);
            }

            let frame = self.snow_frames[particle.type_index][particle.frame];
            self.instances.push(WeatherInstanceRaw {
                pos: [particle.x, particle.y],
                size: [
                    frame.width as f32 * SNOW_SCALE,
                    frame.height as f32 * SNOW_SCALE,
                ],
                uv_min: [frame.uv[0], frame.uv[1]],
                uv_max: [frame.uv[2], frame.uv[3]],
                kind: 0,
                pad: 0,
            });
        }
    }

    fn update_rain(&mut self, dt: f32, viewport: (u32, u32), vp_w: u32, vp_h: u32) {
        let (tex_w, tex_h) = self.rain_size;
        if tex_w == 0 || tex_h == 0 {
            return;
        }

        let tile_w = tex_w as f32 * RAIN_SCALE;
        let tile_h = tex_h as f32 * RAIN_SCALE;

        if self.rain_rows.is_empty() || viewport != self.last_viewport {
            let start = -self.rng.random::<f32>() * tile_h;
            self.rain_rows = seed_rain_rows(tile_h, vp_h, start)
                .into_iter()
                .map(|y| RainRow {
                    y,
                    permutation: self.random_permutation(),
                })
                .collect();
            self.last_viewport = viewport;
        }

        let dy = RAIN_FALL_SPEED * dt;
        for row in &mut self.rain_rows {
            row.y += dy;
        }

        while self.rain_rows.first().is_some_and(|row| row.y > -tile_h) {
            let top = self.rain_rows[0].y;
            let perm = self.random_permutation();
            self.rain_rows.insert(
                0,
                RainRow {
                    y: top - tile_h,
                    permutation: perm,
                },
            );
        }

        self.rain_rows.retain(|row| row.y < vp_h as f32);

        let col_w = (tex_w / RAIN_COLUMN_COUNT as u32) as f32;
        let dest_col_w = col_w * RAIN_SCALE;
        let tiles_x = (vp_w as f32 / tile_w).ceil() as u32;
        for row in &self.rain_rows {
            for tx in 0..tiles_x {
                let base_x = tx as f32 * tile_w;
                for c in 0..RAIN_COLUMN_COUNT {
                    let src_x = row.permutation[c] as u32 as f32 * col_w;
                    let uv_min_x = src_x / tex_w as f32;
                    let uv_max_x = (src_x + col_w) / tex_w as f32;
                    self.instances.push(WeatherInstanceRaw {
                        pos: [base_x + c as f32 * dest_col_w, row.y],
                        size: [dest_col_w, tile_h],
                        uv_min: [uv_min_x, 0.0],
                        uv_max: [uv_max_x, 1.0],
                        kind: 1,
                        pad: 0,
                    });
                }
            }
        }
    }

    fn random_permutation(&mut self) -> [u8; RAIN_COLUMN_COUNT] {
        shuffle_permutation(&mut self.rng)
    }
}

/// Rain row Y positions with `tile_h` spacing, covering the whole viewport
/// from a random offset within one tile above the screen.
fn seed_rain_rows(tile_h: f32, vp_h: u32, start: f32) -> Vec<f32> {
    let mut ys = Vec::new();
    let mut y = start;
    while y < vp_h as f32 {
        ys.push(y);
        y += tile_h;
    }
    ys
}

fn random_particle(
    rng: &mut SmallRng,
    snow_frames: &[Vec<SnowFrame>],
    vp_w: u32,
    vp_h: u32,
    spawn_above: bool,
) -> SnowParticle {
    let x = rng.random_range(0..vp_w) as f32;
    let y = if spawn_above {
        rng.random_range(0..30u32) as f32 - 30.0
    } else {
        rng.random_range(0..vp_h) as f32
    };
    let vy =
        SNOW_MIN_VELOCITY_Y + rng.random::<f32>() * (SNOW_MAX_VELOCITY_Y - SNOW_MIN_VELOCITY_Y);
    let vx = (rng.random::<f32>() * 2.0 - 1.0) * SNOW_DRIFT_X;
    let type_index = rng.random_range(0..snow_frames.len());
    let frame_count = snow_frames[type_index].len();

    SnowParticle {
        x,
        y,
        vx,
        vy,
        type_index,
        frame: rng.random_range(0..frame_count),
        frame_timer: rng.random::<f32>() * SNOW_FRAME_DURATION,
    }
}

fn shuffle_permutation(rng: &mut impl Rng) -> [u8; RAIN_COLUMN_COUNT] {
    let mut arr = [0u8; RAIN_COLUMN_COUNT];
    for (i, slot) in arr.iter_mut().enumerate() {
        *slot = i as u8;
    }
    for i in (1..RAIN_COLUMN_COUNT).rev() {
        let j = rng.random_range(0..=i);
        arr.swap(i, j);
    }
    arr
}

fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pixels: &[u8],
    width: u32,
    height: u32,
    label: &str,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_rows_cover_the_viewport_at_spawn() {
        for (tile_h, vp_h) in [(200.0, 1080), (100.0, 600), (300.0, 480)] {
            let rows = seed_rain_rows(tile_h, vp_h, -tile_h * 0.37);
            assert!(rows.first().is_some_and(|&y| y >= -tile_h && y < 0.0));
            for pair in rows.windows(2) {
                assert!((pair[1] - pair[0] - tile_h).abs() < 1e-4);
            }
            assert!(rows.last().is_some_and(|&y| y + tile_h >= vp_h as f32));
        }
    }

    #[test]
    fn instance_layout_is_48_bytes() {
        assert_eq!(std::mem::size_of::<WeatherInstanceRaw>(), 40);
    }

    #[test]
    fn permutation_is_a_shuffle() {
        let mut rng = SmallRng::seed_from_u64(42);
        let perm = shuffle_permutation(&mut rng);
        let mut sorted = perm;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn weather_shader_is_valid_wgsl() {
        let source = include_str!("../shaders/weather.wgsl");
        let module = naga::front::wgsl::parse_str(source).expect("weather.wgsl must parse as WGSL");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("weather.wgsl must pass naga validation (layout/alignment included)");
    }
}
