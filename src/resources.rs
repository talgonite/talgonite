use bevy::prelude::*;
use rendering::scene::map::renderer::MapRenderer;
use rendering::scene::{CameraState, EffectManager, Scene, creatures, items, minimap, players};
use wgpu;

use std::ops::{Deref, DerefMut};

#[derive(Resource, Default)]
pub struct PlayerAttributes {
    pub current_hp: u32,
    pub max_hp: u32,
    pub current_mp: u32,
    pub max_mp: u32,
}

#[derive(Resource, Clone, Debug)]
pub struct StorageConfig {
    pub root: std::path::PathBuf,
}

impl StorageConfig {
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    pub fn data_arx_path(&self) -> std::path::PathBuf {
        self.root.join("data.arx")
    }

    pub fn settings_path(&self) -> std::path::PathBuf {
        self.root.join("settings.toml")
    }

    pub fn server_dir(&self, server_id: u32) -> std::path::PathBuf {
        let path = self.root.join("servers").join(server_id.to_string());
        let _ = std::fs::create_dir_all(&path);
        path
    }

    pub fn server_characters_dir(&self, server_id: u32) -> std::path::PathBuf {
        let path = self.server_dir(server_id).join("characters");
        let _ = std::fs::create_dir_all(&path);
        path
    }

    pub fn server_maps_dir(&self, server_id: u32) -> std::path::PathBuf {
        let path = self.server_dir(server_id).join("maps");
        let _ = std::fs::create_dir_all(&path);
        path
    }

    pub fn server_metafile_dir(&self, server_id: u32) -> std::path::PathBuf {
        let path = self.server_dir(server_id).join("metafile");
        let _ = std::fs::create_dir_all(&path);
        path
    }

    pub fn server_character_settings_path(
        &self,
        server_id: u32,
        username: &str,
    ) -> std::path::PathBuf {
        self.server_characters_dir(server_id)
            .join(format!("{}.toml", username))
    }

    pub fn server_map_path(&self, server_id: u32, map_id: u16) -> std::path::PathBuf {
        self.server_maps_dir(server_id)
            .join(format!("lod{:03}.map", map_id))
    }
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

#[derive(Clone, Copy, Debug)]
pub struct MinimapViewConfig {
    pub zoom: f32,
    pub layout: minimap::MinimapLayout,
}

impl Default for MinimapViewConfig {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            layout: minimap::MinimapLayout::default(),
        }
    }
}

#[derive(Resource)]
pub struct MinimapRendererState {
    pub renderer: minimap::MinimapRenderer,
    pub camera: CameraState,
    pub config: MinimapViewConfig,
    pub assets: crate::minimap_assets::MinimapAssets,
    pub visible: bool,
}

#[derive(Resource, Debug)]
pub struct MinimapCacheState {
    pub map_id: u16,
    pub map_width: u8,
    pub map_height: u8,
    pub topology_dirty: bool,
    pub tile_atlas_indices: Vec<u8>,
}

impl MinimapCacheState {
    pub fn new(map_id: u16, map_width: u8, map_height: u8) -> Self {
        Self {
            map_id,
            map_width,
            map_height,
            topology_dirty: true,
            tile_atlas_indices: Vec::new(),
        }
    }

    pub fn mark_topology_dirty(&mut self) {
        self.topology_dirty = true;
    }
}

#[derive(Debug)]
pub struct MinimapMarkerEntry {
    pub handle: minimap::MinimapMarkerHandle,
    pub kind: crate::ecs::components::MinimapMarkerKind,
}

#[derive(Resource, Default, Debug)]
pub struct MinimapMarkerSyncState {
    pub handles: std::collections::HashMap<Entity, MinimapMarkerEntry>,
}

impl MinimapRendererState {
    pub fn new(
        renderer_state: &RendererState,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        assets: crate::minimap_assets::MinimapAssets,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let config = MinimapViewConfig::default();
        let renderer = minimap::MinimapRenderer::new(
            &renderer_state.device,
            &renderer_state.queue,
            camera_bind_group_layout,
            assets.tiles_ktx2,
            assets.player_icon_ktx2,
            assets.creature_icon_ktx2,
            config.layout,
        )?;
        let mut camera = CameraState::new(
            glam::UVec2::new(width, height),
            &renderer_state.device,
            config.zoom,
        );
        camera.set_screen_offset(&renderer_state.queue, 0.0, 0.0);

        Ok(Self {
            renderer,
            camera,
            config,
            assets,
            visible: false,
        })
    }
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
pub struct LobbyPortraitRenderer {
    pub batch: players::PlayerBatch,
    pub depth_texture: rendering::texture::Texture,
    pub camera: CameraState,
}

pub struct PortraitRenderTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub depth_texture: rendering::texture::Texture,
    pub batch: players::PlayerBatch,
    pub camera: CameraState,
    pub dirty: bool,
    pub version: u32,
}

#[derive(Resource)]
pub struct PlayerPortraitState {
    pub target: PortraitRenderTarget,
}

#[derive(Resource)]
pub struct ProfilePortraitState {
    pub target: PortraitRenderTarget,
}

impl PortraitRenderTarget {
    pub fn new(
        renderer: &RendererState,
        store: &players::PlayerAssetStore,
        label: &str,
        size: u32,
        camera_offset_y: f32,
    ) -> Self {
        let color_label = format!("{label}_color");
        let depth_label = format!("{label}_depth");
        let texture = rendering::texture::Texture::create_render_texture(
            &renderer.device,
            &color_label,
            size,
            size,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer.device,
            size,
            size,
            &depth_label,
        );
        let mut camera =
            rendering::scene::CameraState::new(glam::UVec2::new(size, size), &renderer.device, 1.0);
        camera.set_screen_offset(&renderer.queue, 0.0, camera_offset_y);

        Self {
            texture: texture.texture,
            view: texture.view,
            depth_texture,
            batch: players::PlayerBatch::new(&renderer.device, store),
            camera,
            dirty: true,
            version: 0,
        }
    }
}

impl PlayerPortraitState {
    pub fn new(renderer: &RendererState, store: &players::PlayerAssetStore) -> Self {
        Self {
            target: PortraitRenderTarget::new(renderer, store, "player_portrait", 64, -42.0),
        }
    }
}

impl ProfilePortraitState {
    pub fn new(renderer: &RendererState, store: &players::PlayerAssetStore) -> Self {
        Self {
            target: PortraitRenderTarget::new(renderer, store, "profile_portrait", 128, -32.0),
        }
    }
}

impl LobbyPortraitRenderer {
    pub fn new(renderer: &RendererState, store: &players::PlayerAssetStore) -> Self {
        let portrait_size = 64;
        let depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer.device,
            portrait_size,
            portrait_size,
            "lobby_portrait_depth",
        );
        let mut camera = rendering::scene::CameraState::new(
            glam::UVec2::new(portrait_size, portrait_size),
            &renderer.device,
            1.0,
        );
        camera.set_screen_offset(&renderer.queue, 0.0, -42.0);

        Self {
            batch: players::PlayerBatch::new(&renderer.device, store),
            depth_texture,
            camera,
        }
    }
}

impl Deref for PlayerPortraitState {
    type Target = PortraitRenderTarget;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl DerefMut for PlayerPortraitState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

impl Deref for ProfilePortraitState {
    type Target = PortraitRenderTarget;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl DerefMut for ProfilePortraitState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

#[derive(Resource)]
pub struct TranslucentPlayerPassState {
    pub color_texture: rendering::texture::Texture,
    pub depth_texture: rendering::texture::Texture,
}

#[derive(Resource)]
pub struct CharacterCreatorPreviewState {
    pub target: Option<PortraitRenderTarget>,
    pub gender: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub armor_id: u16,
    pub dirty: bool,
    pub version: u32,
}

impl Default for CharacterCreatorPreviewState {
    fn default() -> Self {
        Self {
            target: None,
            gender: 1,
            hair_style: 0,
            hair_color: 0,
            armor_id: 1,
            dirty: true,
            version: 0,
        }
    }
}

impl CharacterCreatorPreviewState {
    pub fn with_target(
        renderer: &RendererState,
        store: &players::PlayerAssetStore,
        gender: u8,
        hair_style: u8,
        hair_color: u8,
        armor_id: u16,
        version: u32,
    ) -> Self {
        Self {
            target: Some(PortraitRenderTarget::new(
                renderer,
                store,
                "character_creator_portrait",
                64,
                -42.0,
            )),
            gender,
            hair_style,
            hair_color,
            armor_id,
            dirty: true,
            version,
        }
    }
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
    pub high_quality_scaling: bool,
}

impl ZoomState {
    const TARGET_RENDER_HEIGHT: u32 = 600;

    pub fn new(
        display_w: u32,
        display_h: u32,
        dpi_scale: f32,
        zoom: f32,
        high_quality_scaling: bool,
    ) -> Self {
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
            high_quality_scaling,
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

    pub fn set_high_quality_scaling(&mut self, enabled: bool) {
        self.high_quality_scaling = enabled;
        self.recalculate();
    }

    pub fn cursor_to_render_scale(&self) -> f32 {
        if self.high_quality_scaling {
            self.dpi_scale
        } else {
            self.dpi_scale / self.user_zoom.max(1.0)
        }
    }

    pub fn display_scale(&self) -> f32 {
        if self.high_quality_scaling {
            1.0
        } else if self.is_pixel_perfect {
            self.user_zoom
        } else {
            1.0
        }
    }

    fn recalculate(&mut self) {
        let zoom = self.user_zoom.clamp(0.1, 5.0);

        const MIN_RENDER_DIM: u32 = 320;

        if self.high_quality_scaling {
            self.render_size = self.display_size;
            self.camera_zoom = zoom;
            self.is_pixel_perfect = true; // Always native, so no interest in "blowing up" pixel-perfectly
        } else if zoom < 1.0 {
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
