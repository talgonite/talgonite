//! Rendering notifier callbacks for Slint/Bevy frame exchange.

use bevy::prelude::*;

use crate::WindowSurface;
use crate::app_state::AppState;
use crate::resources::ZoomState;
use crate::slint_support::frame_exchange::{BackBufferPool, ControlMessage, FrameChannels};

/// Handle BeforeRendering: sync display size/zoom and exchange frame textures.
pub fn handle_before_rendering<W: slint::ComponentHandle>(
    app: &mut App,
    slint_app: &W,
    get_texture_width: impl Fn(&W) -> u32,
    get_texture_height: impl Fn(&W) -> u32,
    get_texture_scale: impl Fn(&W) -> f32,
    set_pixelated_filtering: impl Fn(&W, bool),
    get_texture: impl Fn(&W) -> slint::Image,
    set_texture: impl Fn(&W, slint::Image),
) {
    let display_width = get_texture_width(slint_app);
    let display_height = get_texture_height(slint_app);
    let dpi_scale = get_texture_scale(slint_app);

    if display_width > 0 && display_height > 0 {
        if let Some(mut zoom_state) = app.world_mut().get_resource_mut::<ZoomState>() {
            if zoom_state.display_size != (display_width, display_height) {
                zoom_state.set_display_size(display_width, display_height);
            }
            if (zoom_state.dpi_scale - dpi_scale).abs() > 0.001 {
                zoom_state.set_dpi_scale(dpi_scale);
            }
        }
    }

    let (render_size, is_pixel_perfect, camera_zoom) = app
        .world()
        .get_resource::<ZoomState>()
        .map(|zs| (zs.render_size, zs.is_pixel_perfect, zs.camera_zoom))
        .unwrap_or(((display_width, display_height), true, 1.0));

    set_pixelated_filtering(slint_app, is_pixel_perfect);

    if let Some(ch) = app.world().get_resource::<FrameChannels>() {
        let in_game = app
            .world()
            .get_resource::<State<AppState>>()
            .map(|s| *s.get() == AppState::InGame)
            .unwrap_or(false);

        if in_game && render_size.0 > 0 && render_size.1 > 0 {
            let needs_resize = app
                .world()
                .get_non_send_resource::<WindowSurface>()
                .map(|surface| {
                    surface.width != render_size.0
                        || surface.height != render_size.1
                        || (surface.scale_factor - camera_zoom).abs() > 0.001
                })
                .unwrap_or(true);

            if needs_resize {
                tracing::info!(
                    "Resizing render target: {}x{} (display: {}x{}, zoom: {:.2}x, camera_zoom: {:.2})",
                    render_size.0,
                    render_size.1,
                    display_width,
                    display_height,
                    app.world()
                        .get_resource::<ZoomState>()
                        .map(|zs| zs.user_zoom)
                        .unwrap_or(1.0),
                    camera_zoom
                );

                let _ = ch.control_tx.try_send(ControlMessage::ResizeBuffers {
                    width: render_size.0,
                    height: render_size.1,
                    scale: camera_zoom,
                });
            }
        }

        if let Ok(new_texture) = ch.front_buffer_rx.try_recv() {
            let current = get_texture(slint_app);
            if let Some(old) = current.to_wgpu_28_texture() {
                let _ = ch
                    .control_tx
                    .try_send(ControlMessage::ReleaseFrontBufferTexture { texture: old });
            }
            if let Ok(image) = new_texture.try_into() {
                set_texture(slint_app, image);
            }
        }
    }
}

/// Seed the back buffer pool with initial textures.
pub fn seed_back_buffers(app: &mut App, device: &wgpu::Device, width: u32, height: u32) {
    // Grab control sender clone without holding a mutable borrow to the World
    let ctrl_sender = app
        .world()
        .get_resource::<FrameChannels>()
        .map(|c| c.control_tx.clone());

    if let Some(mut pool) = app.world_mut().get_resource_mut::<BackBufferPool>() {
        if pool.0.is_empty() {
            let mut seeded = Vec::new();
            for label in ["Front Buffer", "Back Buffer", "Inflight Buffer"] {
                let tex = device.create_texture(&wgpu::TextureDescriptor {
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
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                seeded.push(tex);
            }
            if let Some(tx) = ctrl_sender {
                for tex in seeded.into_iter() {
                    let _ = tx.try_send(ControlMessage::ReleaseFrontBufferTexture { texture: tex });
                }
            } else {
                pool.0.extend(seeded.into_iter());
            }
        }
    }
}
