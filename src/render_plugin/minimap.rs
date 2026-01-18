use bevy::prelude::*;
use glam::UVec2;
use rendering::scene::CameraState;
use rendering::scene::map::renderer::MapRenderer;

/// Resource holding an offscreen texture containing the latest minimap snapshot.
#[derive(Resource)]
pub struct MinimapTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: UVec2,
}

impl MinimapTexture {
    pub fn new(device: &wgpu::Device, size: UVec2, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("minimap.offscreen"),
            size: wgpu::Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            size,
        }
    }
}

/// Helper: render map into offscreen texture at a chosen scale that fits entire bounds.
pub fn render_minimap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    map: &MapRenderer,
    scene_pipeline: &wgpu::RenderPipeline,
    size: UVec2,
) -> Option<wgpu::CommandBuffer> {
    let (min, max) = map.bounds()?;
    let map_w = (max.x - min.x).max(1.0);
    let map_h = (max.y - min.y).max(1.0);
    let scale_x = size.x as f32 / map_w;
    let scale_y = size.y as f32 / map_h;
    let scale = scale_x.min(scale_y); // uniform scale to keep aspect

    // Create a dedicated camera for minimap
    let mut cam = CameraState::new(size, device);
    cam.set_zoom(queue, scale);
    // Move camera so that min corner maps to (0,0) in final
    cam.set_screen_offset(queue, -min.x, -min.y);

    // Begin offscreen render
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("minimap.scratch"),
        size: wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("minimap.encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("minimap.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.,
                        g: 0.,
                        b: 0.,
                        a: 0.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(scene_pipeline);
        pass.set_bind_group(1, &cam.camera_bind_group, &[]);
        map.render(&mut pass);
    }
    Some(encoder.finish())
}

/// Copies the rendered minimap texture into a buffer and returns its data (blocking).
#[derive(Resource, Clone, Copy, Debug)]
pub struct MinimapConfig {
    /// Desired pixels per world tile on the minimap (uniform for width/height)
    pub pixels_per_tile: f32,
    /// Minimum final edge size (after fitting whole map)
    pub min_size: u32,
    /// Maximum final edge size (to avoid giant textures)
    pub max_size: u32,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            pixels_per_tile: 8.0,
            min_size: 96,
            max_size: 512,
        }
    }
}

/// Compute a target offscreen texture size for a map given config.
pub fn compute_dynamic_minimap_size(map: &MapRenderer, cfg: MinimapConfig) -> UVec2 {
    if let Some((min, max)) = map.bounds() {
        let map_w = (max.x - min.x).max(1.0);
        let map_h = (max.y - min.y).max(1.0);
        // Convert world-space width/height (already in screen pixel units of isometric projection) into tile counts approximation
        // TILE_WIDTH is 56 across two tiles diagonally; we approximate tile span by dividing by 56
        let approx_tiles_w = map_w / rendering::scene::TILE_WIDTH as f32;
        let approx_tiles_h = map_h / rendering::scene::TILE_HEIGHT as f32; // vertical uses tile height directly
        let px_w = (approx_tiles_w * cfg.pixels_per_tile).ceil() as u32;
        let px_h = (approx_tiles_h * cfg.pixels_per_tile).ceil() as u32;
        let w = px_w.clamp(cfg.min_size, cfg.max_size);
        let h = px_h.clamp(cfg.min_size, cfg.max_size);
        return UVec2::new(w.max(1), h.max(1));
    }
    UVec2::new(cfg.min_size, cfg.min_size)
}

pub fn read_minimap_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    map: &MapRenderer,
    scene_pipeline: &wgpu::RenderPipeline,
    color_format: wgpu::TextureFormat,
    desired_size: Option<UVec2>,
    cfg: MinimapConfig,
) -> Option<Vec<u8>> {
    let size = desired_size.unwrap_or_else(|| compute_dynamic_minimap_size(map, cfg));
    // 1. Create render target
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("minimap.gpu"),
        size: wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: color_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[color_format],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut cam = CameraState::new(size, device, 1.0);
    if let Some((min, max)) = map.bounds() {
        let map_w = (max.x - min.x).max(1.0);
        let map_h = (max.y - min.y).max(1.0);
        // Uniform scale so entire map fits within target while preserving aspect
        let scale = (size.x as f32 / map_w).min(size.y as f32 / map_h);
        cam.set_zoom(queue, scale);
        // Camera position in this renderer acts like a world-space center (after its internal center translation).
        // To center the map we place camera at the map's center rather than translating min to (0,0).
        let center_x = min.x + map_w * 0.5;
        let center_y = min.y + map_h * 0.5;
        cam.set_screen_offset(queue, center_x, center_y);
    }

    // 3. Create a small depth texture matching pipeline expectation (Depth32Float) since pipeline uses depth
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("minimap.depth"),
        size: wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rendering::texture::Texture::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    // 3b. Render pass
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("minimap.encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("minimap.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(scene_pipeline);
        pass.set_bind_group(1, &cam.camera_bind_group, &[]);
        map.render(&mut pass);
    }

    // 4. Copy to buffer (align bytes_per_row to 256)
    let bpr_unpadded = size.x * 4;
    let align = 256u32;
    let padded_bpr = ((bpr_unpadded + align - 1) / align) * align;
    let buffer_size = padded_bpr as u64 * size.y as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("minimap.readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(size.y),
            },
        },
        wgpu::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    // 5. Map & extract
    let slice = readback.slice(..);
    let (sender, receiver) = crossbeam_channel::bounded(1);
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = sender.send(r);
    });
    // Poll until done
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        return None;
    }
    if receiver.recv().ok()?.is_err() {
        return None;
    }
    let data = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((size.x * size.y * 4) as usize);
    for row in 0..size.y {
        let start = (row as u64 * padded_bpr as u64) as usize;
        rgba.extend_from_slice(&data[start..start + bpr_unpadded as usize]);
    }
    drop(data);
    readback.unmap();

    // 6. Encode PNG
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut encoder_png = png::Encoder::new(&mut cursor, size.x, size.y);
    encoder_png.set_color(png::ColorType::Rgba);
    encoder_png.set_depth(png::BitDepth::Eight);
    if let Ok(mut writer) = encoder_png.write_header() {
        let _ = writer.write_image_data(&rgba);
    }
    let png_bytes = cursor.into_inner();
    if png_bytes.is_empty() {
        // Fallback to simple CPU visualization (should rarely happen)
        return cpu_minimap_fallback(map, size);
    }
    Some(png_bytes)
}

fn cpu_minimap_fallback(map: &MapRenderer, size: UVec2) -> Option<Vec<u8>> {
    let (min, max) = map.bounds()?;
    let map_w = (max.x - min.x).max(1.0);
    let map_h = (max.y - min.y).max(1.0);
    let scale_x = size.x as f32 / map_w;
    let scale_y = size.y as f32 / map_h;
    let mut img = vec![0u8; (size.x * size.y * 4) as usize];
    for inst in map.floor_instances() {
        let p = inst.position.truncate();
        let tx = (p.x - min.x) * scale_x;
        let ty = (p.y - min.y) * scale_y;
        let tw = rendering::scene::TILE_WIDTH as f32 * scale_x;
        let th = rendering::scene::TILE_HEIGHT as f32 * scale_y;
        let color = (inst.palette_offset * 255.0) as u8;
        let (r, g, b) = (color, 255u8.saturating_sub(color), (color / 2));
        let x0 = tx as i32;
        let y0 = ty as i32;
        let x1 = (tx + tw).ceil() as i32;
        let y1 = (ty + th).ceil() as i32;
        for y in y0.max(0)..y1.min(size.y as i32) {
            for x in x0.max(0)..x1.min(size.x as i32) {
                let idx = ((y as u32 * size.x + x as u32) * 4) as usize;
                img[idx] = r;
                img[idx + 1] = g;
                img[idx + 2] = b;
                img[idx + 3] = 255;
            }
        }
    }
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut encoder_png = png::Encoder::new(&mut cursor, size.x, size.y);
    encoder_png.set_color(png::ColorType::Rgba);
    encoder_png.set_depth(png::BitDepth::Eight);
    if let Ok(mut writer) = encoder_png.write_header() {
        let _ = writer.write_image_data(&img);
    }
    Some(cursor.into_inner())
}
