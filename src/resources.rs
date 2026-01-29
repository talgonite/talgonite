use bevy::prelude::*;
use rendering::scene::map::renderer::MapRenderer;
use rendering::scene::{CameraState, EffectManager, Scene, creatures, items, players};
use wgpu;

#[derive(Resource, Default)]
pub struct PlayerAttributes {
    pub current_hp: u32,
    pub max_hp: u32,
    pub current_mp: u32,
    pub max_mp: u32,
}

#[derive(Resource)]
pub struct RendererState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub scene: Scene,
}

#[derive(Resource)]
pub struct Camera {
    pub camera: CameraState,
}

#[derive(Resource)]
pub struct MapRendererState {
    pub map_renderer: MapRenderer,
}

#[derive(Resource)]
pub struct CreatureAssetStoreState {
    pub store: creatures::CreatureAssetStore,
}

#[derive(Resource)]
pub struct CreatureBatchState {
    pub batch: creatures::CreatureBatch,
}

#[derive(Resource)]
pub struct PlayerAssetStoreState {
    pub store: players::PlayerAssetStore,
}

#[derive(Resource)]
pub struct PlayerBatchState {
    pub batch: players::PlayerBatch,
}

#[derive(Resource)]
pub struct ItemAssetStoreState {
    pub store: items::ItemAssetStore,
}

#[derive(Resource)]
pub struct ItemBatchState {
    pub batch: items::ItemBatch,
}

/// Per-tile spawn order counters for item z-ordering.
/// Map-scoped: auto-cleared when map changes via Bevy resource removal.
#[derive(Resource, Default)]
pub struct ItemTileCounters {
    pub counters: std::collections::HashMap<(u16, u16), u8>,
}

impl ItemTileCounters {
    pub fn next_order(&mut self, x: u16, y: u16) -> u8 {
        let counter = self.counters.entry((x, y)).or_insert(0);
        let order = *counter;
        *counter = counter.wrapping_add(1);
        order
    }
}

#[derive(Resource)]
pub struct EffectManagerState {
    pub effect_manager: EffectManager,
}

#[derive(Resource, Default)]
pub struct LobbyPortraits {
    pub textures: std::collections::HashMap<String, wgpu::Texture>,
    pub version: u32,
}

#[derive(Resource)]
pub struct PlayerPortraitState {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth_texture: rendering::texture::Texture,
    pub batch: players::PlayerBatch,
    pub camera: CameraState,
    pub dirty: bool,
    pub version: u32,
}

#[derive(Resource)]
pub struct ProfilePortraitState {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth_texture: rendering::texture::Texture,
    pub batch: players::PlayerBatch,
    pub camera: CameraState,
    pub dirty: bool,
    pub version: u32,
}

pub struct WindowSurface {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

#[derive(Resource)]
pub struct ZoomState {
    pub user_zoom: f32,
    pub dpi_scale: f32,
    pub display_size: (u32, u32),
    pub render_size: (u32, u32),
    pub camera_zoom: f32,
    pub is_pixel_perfect: bool,
}

impl ZoomState {
    const TARGET_RENDER_HEIGHT: u32 = 600;

    pub fn new(display_w: u32, display_h: u32, dpi_scale: f32, zoom: f32) -> Self {
        let initial_zoom = if zoom == 1.0 {
            Self::compute_initial_zoom(display_h)
        } else {
            zoom
        };

        let mut state = Self {
            user_zoom: initial_zoom,
            dpi_scale,
            display_size: (display_w, display_h),
            render_size: (display_w, display_h),
            camera_zoom: 1.0,
            is_pixel_perfect: true,
        };
        state.recalculate();
        state
    }

    fn compute_initial_zoom(display_height: u32) -> f32 {
        let ideal_zoom = display_height as f32 / Self::TARGET_RENDER_HEIGHT as f32;
        let rounded = ideal_zoom.round().max(1.0);
        rounded.clamp(1.0, 5.0)
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.user_zoom = zoom.clamp(0.1, 5.0);
        self.recalculate();
    }

    pub fn set_display_size(&mut self, w: u32, h: u32) {
        self.display_size = (w, h);
        self.recalculate();
    }

    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale;
    }

    pub fn cursor_to_render_scale(&self) -> f32 {
        self.dpi_scale / self.user_zoom.max(1.0)
    }

    fn recalculate(&mut self) {
        let zoom = self.user_zoom.clamp(0.1, 5.0);

        const MIN_RENDER_DIM: u32 = 320;

        if zoom < 1.0 {
            self.render_size = self.display_size;
            self.camera_zoom = zoom;
            self.is_pixel_perfect = false;
        } else {
            let render_w = ((self.display_size.0 as f32 / zoom).round() as u32).max(MIN_RENDER_DIM);
            let render_h =
                ((self.display_size.1 as f32 / zoom).round() as u32).max(MIN_RENDER_DIM / 2);

            self.render_size = (render_w, render_h);
            self.camera_zoom = 1.0;

            let frac = zoom.fract();
            self.is_pixel_perfect = frac < 0.01 || frac > 0.99;
        }
    }
}
