//! GPU darkness overlay: ambient + HEA static light + lantern masks are
//! combined in a fullscreen composite pass over the offscreen scene color.

use crate::scene::constants::TILE_WIDTH_HALF;
use wgpu::util::DeviceExt;

/// Maximum dynamic light sources per frame (shader iterates a fixed array).
pub const MAX_LIGHTS: usize = 64;

/// Lantern falloff mask; pixels are light intensities (0..=32).
#[derive(Debug, Clone)]
pub struct LightMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// A screen-space dynamic light; `mask_layer` is 0 (small) or 1 (large).
#[derive(Debug, Clone, Copy)]
pub struct LightSource {
    pub screen_x: f32,
    pub screen_y: f32,
    pub mask_layer: u8,
}

/// Rasterized HEA light map (row-major R8 intensities) and its screen origin.
#[derive(Debug, Clone)]
pub struct HeaData {
    pub width: u32,
    pub height: u32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub pixels: Vec<u8>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightSourceRaw {
    screen: [f32; 2],
    mask_layer: u32,
    pad: u32,
}

/// Mirrors `DarknessUniform` in `shaders/darkness.wgsl`.
#[repr(C)]
#[repr(align(16))]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DarknessUniformRaw {
    ambient: f32,
    light_count: u32,
    pad0: u32,
    pad1: u32,
    color: [f32; 3],
    pad2: f32,
    hea_offset: [f32; 2],
    hea_size: [f32; 2],
    mask_sizes: [f32; 4],
    camera_pos: [f32; 2],
    viewport: [f32; 2],
    zoom: f32,
    pad3: f32,
    pad4: f32,
    pad5: f32,
    lights: [LightSourceRaw; MAX_LIGHTS],
}

impl Default for DarknessUniformRaw {
    fn default() -> Self {
        Self {
            ambient: 32.0,
            light_count: 0,
            pad0: 0,
            pad1: 0,
            color: [0.0; 3],
            pad2: 0.0,
            hea_offset: [0.0; 2],
            hea_size: [1.0, 1.0],
            mask_sizes: [0.0; 4],
            camera_pos: [0.0; 2],
            viewport: [1.0, 1.0],
            zoom: 1.0,
            pad3: 0.0,
            pad4: 0.0,
            pad5: 0.0,
            lights: [LightSourceRaw {
                screen: [0.0; 2],
                mask_layer: 0,
                pad: 0,
            }; MAX_LIGHTS],
        }
    }
}

pub struct DarknessRenderer {
    hea: Option<(u32, u32, wgpu::Texture)>,
    mask_small: Option<wgpu::Texture>,
    mask_large: Option<wgpu::Texture>,
    fallback: wgpu::Texture,
    sampler_hea: wgpu::Sampler,
    sampler_mask: wgpu::Sampler,
    uniform: DarknessUniformRaw,
    uniform_buffer: wgpu::Buffer,
    pending_uniform: Option<DarknessUniformRaw>,
    belt: wgpu::util::StagingBelt,
}

impl DarknessRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let fallback = create_r8_texture(device, queue, &[0], 1, 1, "Darkness Fallback");

        let sampler_hea = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HEA Light Map Sampler"),
            // ClampToEdge avoids the optional ClampToBorder feature; the shader
            // handles out-of-bounds texels.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let sampler_mask = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Lantern Mask Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform = DarknessUniformRaw::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Darkness Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            hea: None,
            mask_small: None,
            mask_large: None,
            fallback,
            sampler_hea,
            sampler_mask,
            uniform,
            uniform_buffer,
            pending_uniform: None,
            belt: wgpu::util::StagingBelt::new(device.clone(), 1024 * 1024),
        }
    }

    /// Whether an HEA light map is loaded for the current map.
    pub fn has_hea(&self) -> bool {
        self.hea.is_some()
    }

    /// Sets the ambient overlay: `alpha` 0 = bright, 1 = dark.
    pub fn set_ambient(&mut self, alpha: f32, color: [u8; 3]) {
        self.uniform.ambient = 32.0 * (1.0 - alpha.clamp(0.0, 1.0));
        self.uniform.color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
    }

    /// Sets the map's HEA light map; `map_height` shifts the HEA x origin.
    pub fn set_map(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        map_height: u8,
        hea: Option<HeaData>,
    ) {
        self.hea = hea.as_ref().map(|data| {
            let texture = create_r8_texture(
                device,
                queue,
                &data.pixels,
                data.width,
                data.height,
                "HEA Light Map",
            );
            (data.width, data.height, texture)
        });

        match &hea {
            Some(data) => {
                self.uniform.hea_size = [data.width as f32, data.height as f32];
                // HEA pixels are authored against tile centers; the camera
                // tracks tile origins, so add the half tile back.
                self.uniform.hea_offset = [
                    (map_height as f32 - 1.0) * TILE_WIDTH_HALF as f32
                        + data.screen_width
                        + TILE_WIDTH_HALF as f32,
                    data.screen_height,
                ];
            }
            None => {
                self.uniform.hea_size = [1.0, 1.0];
                self.uniform.hea_offset = [0.0, 0.0];
            }
        }
    }

    /// Sets the lantern masks; absent layers are skipped via `mask_sizes`.
    pub fn set_masks(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        small: Option<LightMask>,
        large: Option<LightMask>,
    ) {
        self.mask_small = small.as_ref().map(|mask| {
            create_r8_texture(
                device,
                queue,
                &mask.pixels,
                mask.width,
                mask.height,
                "Lantern Mask Small",
            )
        });
        self.mask_large = large.as_ref().map(|mask| {
            create_r8_texture(
                device,
                queue,
                &mask.pixels,
                mask.width,
                mask.height,
                "Lantern Mask Large",
            )
        });

        self.uniform.mask_sizes = [
            small.as_ref().map_or(0.0, |m| m.width as f32),
            small.as_ref().map_or(0.0, |m| m.height as f32),
            large.as_ref().map_or(0.0, |m| m.width as f32),
            large.as_ref().map_or(0.0, |m| m.height as f32),
        ];
    }

    /// Uploads the per-frame camera pose, viewport, and light sources.
    pub fn update_uniform(
        &mut self,
        camera_pos: [f32; 2],
        zoom: f32,
        viewport: [f32; 2],
        sources: &[LightSource],
    ) {
        fill_uniform(&mut self.uniform, camera_pos, zoom, viewport, sources);
        self.pending_uniform = Some(self.uniform);
    }

    /// Copies the pending uniform into the staging belt on `encoder`; call
    /// before the darkness composite pass.
    pub fn flush_pending(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(uniform) = self.pending_uniform.take() else {
            return;
        };
        let size = std::mem::size_of::<DarknessUniformRaw>() as u64;
        let mut view = self.belt.write_buffer(
            encoder,
            &self.uniform_buffer,
            0,
            wgpu::BufferSize::new(size).expect("darkness uniform size is nonzero"),
        );
        view.copy_from_slice(bytemuck::bytes_of(&uniform));
    }

    pub fn finish_uploads(&mut self) {
        self.belt.finish();
    }

    pub fn recall_uploads(&mut self) {
        self.belt.recall();
    }

    /// Builds the composite pass bind group.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        scene_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let fallback_view = self
            .fallback
            .create_view(&wgpu::TextureViewDescriptor::default());
        let hea_view = self
            .hea
            .as_ref()
            .map(|(_, _, texture)| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or_else(|| fallback_view.clone());
        let small_view = self
            .mask_small
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or_else(|| fallback_view.clone());
        let large_view = self
            .mask_large
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or_else(|| fallback_view.clone());

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Darkness Composite Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&hea_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_hea),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&small_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&large_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_mask),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        })
    }
}

fn fill_uniform(
    uniform: &mut DarknessUniformRaw,
    camera_pos: [f32; 2],
    zoom: f32,
    viewport: [f32; 2],
    sources: &[LightSource],
) {
    uniform.camera_pos = camera_pos;
    uniform.zoom = if zoom > 0.0 { zoom } else { 1.0 };
    uniform.viewport = viewport;
    uniform.light_count = sources.len().min(MAX_LIGHTS) as u32;

    for (i, source) in sources.iter().take(MAX_LIGHTS).enumerate() {
        uniform.lights[i] = LightSourceRaw {
            screen: [source.screen_x, source.screen_y],
            mask_layer: source.mask_layer as u32,
            pad: 0,
        };
    }
}

fn create_r8_texture(
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
        format: wgpu::TextureFormat::R8Unorm,
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
            bytes_per_row: Some(width),
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
    fn darkness_shader_is_valid_wgsl() {
        let source = include_str!("../shaders/darkness.wgsl");
        let module =
            naga::front::wgsl::parse_str(source).expect("darkness.wgsl must parse as WGSL");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("darkness.wgsl must pass naga validation (layout/alignment included)");
        let _ = info;
    }

    #[test]
    fn uniform_layout_matches_shader_struct() {
        // Must match DarknessUniform in darkness.wgsl: 96-byte header + 64 * 16-byte lights.
        assert_eq!(std::mem::size_of::<LightSourceRaw>(), 16);
        assert_eq!(std::mem::size_of::<DarknessUniformRaw>(), 96 + 64 * 16);
        assert_eq!(std::mem::align_of::<DarknessUniformRaw>(), 16);
    }

    #[test]
    fn ambient_maps_to_intensity_and_sources_fill() {
        let mut uniform = DarknessUniformRaw::default();

        uniform.ambient = 32.0 * (1.0 - 1.0);
        uniform.color = [6.0 / 255.0, 11.0 / 255.0, 60.0 / 255.0];
        assert_eq!(uniform.ambient, 0.0);

        fill_uniform(
            &mut uniform,
            [100.0, 200.0],
            2.0,
            [1920.0, 1080.0],
            &[
                LightSource {
                    screen_x: 10.0,
                    screen_y: 20.0,
                    mask_layer: 0,
                },
                LightSource {
                    screen_x: 30.0,
                    screen_y: 40.0,
                    mask_layer: 1,
                },
            ],
        );
        assert_eq!(uniform.light_count, 2);
        assert_eq!(uniform.camera_pos, [100.0, 200.0]);
        assert_eq!(uniform.zoom, 2.0);
        assert_eq!(uniform.lights[0].screen, [10.0, 20.0]);
        assert_eq!(uniform.lights[0].mask_layer, 0);
        assert_eq!(uniform.lights[1].mask_layer, 1);
    }
}
