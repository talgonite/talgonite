//! GPU initialization for the Slint/Bevy bridge.

use bevy::prelude::*;
use rendering::scene::Scene;

use crate::resources::ZoomState;
use crate::slint_support::frame_exchange::{BackBufferPool, FrameChannels};
use crate::{Camera, RendererState, WindowSurface};

use super::SlintGpuReady;

/// Initialize full renderer stack (surface, scene, camera) using Slint's provided wgpu context + window.
pub fn initialize_gpu_world(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    window: &slint::Window,
    texture_format: wgpu::TextureFormat,
) {
    // Avoid double init if already present.
    if world.contains_resource::<RendererState>() {
        return;
    }

    let size = window.size();

    tracing::info!(
        "Initializing Slint GPU world with size {}x{} (scale factor {})",
        size.width,
        size.height,
        window.scale_factor()
    );

    let mut scene = Scene::new(device, size.width, size.height, texture_format);
    scene.resize_depth_texture(device, size.width, size.height);
    let camera = rendering::scene::CameraState::new(
        (size.width, size.height).into(),
        device,
        window.scale_factor(),
    );

    world.insert_resource(RendererState {
        device: device.clone(),
        queue: queue.clone(),
        scene,
    });
    // Safety: surface tied to window lifetime which matches app lifetime.
    world.insert_non_send_resource(WindowSurface {
        width: size.width,
        height: size.height,
        scale_factor: window.scale_factor(),
    });
    world.insert_resource(Camera { camera });
    let (initial_zoom, high_quality_scaling) = world
        .get_resource::<crate::settings_types::Settings>()
        .map(|s| (s.graphics.scale, s.graphics.high_quality_scaling))
        .unwrap_or((1.0, true));
    world.insert_resource(ZoomState::new(
        size.width,
        size.height,
        window.scale_factor(),
        initial_zoom,
        high_quality_scaling,
    ));
    // Initialize frame channels & an empty pool (textures allocated lazily after notifier knows desired size)
    if !world.contains_resource::<FrameChannels>() {
        world.insert_resource(FrameChannels::new());
    }
    world.init_resource::<BackBufferPool>();
    // Seed frame buffers and channels will be handled from main.rs after this call.
    if let Some(mut ready) = world.get_resource_mut::<SlintGpuReady>() {
        ready.0 = true;
    } else {
        world.insert_resource(SlintGpuReady(true));
    }
    tracing::info!("Slint GPU world initialized (surface + scene + camera)");
}
