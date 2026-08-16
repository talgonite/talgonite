use bevy::prelude::*;
use rendering::scene::map::renderer::MapRenderer;
use rendering::scene::{
    CameraState, EffectManager, Scene, UnifiedSpriteBatch, minimap, players,
    unified_batch::SpriteScene,
};
use wgpu;

use std::ops::{Deref, DerefMut};

#[derive(Resource, Default, Debug, Clone)]
pub struct PlayerAttributes {
    pub current_hp: u32,
    pub max_hp: u32,
    pub current_mp: u32,
    pub max_mp: u32,
    pub level: u8,
    pub ability: u8,
    pub str: u8,
    pub int: u8,
    pub wis: u8,
    pub con: u8,
    pub dex: u8,
    pub unspent_points: u8,
    pub max_weight: i16,
    pub current_weight: i16,
    pub total_exp: u32,
    pub to_next_level: u32,
    pub total_ability: u32,
    pub to_next_ability: u32,
    pub game_points: u32,
    pub gold: u32,
    pub blind: bool,
    pub has_unread_mail: bool,
    pub offense_element: u8,
    pub defense_element: u8,
    pub magic_resistance: u8,
    pub ac: i8,
    pub dmg: u8,
    pub hit: u8,
}

impl PlayerAttributes {
    pub fn update(&mut self, attrs: &packets::server::Attributes) {
        if let Some(vitality) = &attrs.vitality {
            self.current_hp = vitality.current_hp;
            self.current_mp = vitality.current_mp;
        }
        if let Some(primary) = &attrs.primary {
            self.max_hp = primary.maximum_hp;
            self.max_mp = primary.maximum_mp;
            self.level = primary.level;
            self.ability = primary.ability;
            self.str = primary.str;
            self.int = primary.int;
            self.wis = primary.wis;
            self.con = primary.con;
            self.dex = primary.dex;
            self.unspent_points = primary.unspent_points;
            self.max_weight = primary.max_weight;
            self.current_weight = primary.current_weight;
        }
        if let Some(exp_gold) = &attrs.exp_gold {
            self.total_exp = exp_gold.total_exp;
            self.to_next_level = exp_gold.to_next_level;
            self.total_ability = exp_gold.total_ability;
            self.to_next_ability = exp_gold.to_next_ability;
            self.game_points = exp_gold.game_points;
            self.gold = exp_gold.gold;
        }
        if let Some(secondary) = &attrs.secondary {
            self.blind = secondary.blind;
            self.has_unread_mail = secondary.has_unread_mail;
            self.offense_element = secondary.offense_element;
            self.defense_element = secondary.defense_element;
            self.magic_resistance = secondary.magic_resistance;
            self.ac = secondary.ac;
            self.dmg = secondary.dmg;
            self.hit = secondary.hit;
        }
    }
}

/// Per-frame counters for the debug console. Systems bump the `acc_*` fields
/// or write `last_*` directly; `finish_frame_metrics` folds accumulators and
/// GPU-batch deltas into the `last_*` snapshot consumed by the console.
#[derive(Resource, Default)]
pub struct FrameMetrics {
    pub frame_count: u64,
    pub last_fps: f32,
    pub last_update_us: u64,
    pub last_draw_us: u64,
    pub last_draw_passes: u32,
    pub last_queue_submits: u32,
    pub last_map_instances: u32,
    pub last_sprite_instances: u32,
    pub last_effect_instances: u32,
    pub last_weather_instances: u32,
    pub last_minimap_tiles: u32,
    pub last_minimap_markers: u32,
    pub last_texture_handoffs: u64,
    pub last_repaints: u64,
    pub last_instance_updates: u64,
    pub last_instance_writes: u64,
    pub last_instance_dedup_skips: u64,
    pub last_instance_adds: u64,
    pub last_instance_removes: u64,
    pub last_slint_sets_attempted: u64,
    pub last_slint_sets_sent: u64,
    pub last_slint_model_rebuilds: u64,
    pub last_slint_core_syncs: u64,
    pub last_top_systems: Vec<(String, u64)>,
    pub acc_slint_sets_attempted: u64,
    pub acc_slint_sets_sent: u64,
    pub acc_slint_model_rebuilds: u64,
    pub acc_slint_core_syncs: u64,
    pub acc_repaints: u64,
    prev_instance_updates: u64,
    prev_instance_writes: u64,
    prev_instance_dedup_skips: u64,
    prev_instance_adds: u64,
    prev_instance_removes: u64,
}

/// Pending log lines for the debug console window.
#[derive(Resource, Default)]
pub struct DebugLog {
    pending: std::collections::VecDeque<String>,
}

impl DebugLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.pending.push_back(line.into());
    }

    pub fn drain(&mut self) -> Vec<String> {
        self.pending.drain(..).collect()
    }
}

/// Folds per-frame accumulators and GPU-batch deltas into the `last_*` fields
/// the debug console displays. Runs at the end of every Bevy update.
pub fn finish_frame_metrics(
    mut metrics: ResMut<FrameMetrics>,
    sprite_batch: Option<Res<UnifiedSpriteBatchState>>,
    effect_manager: Option<Res<EffectManagerState>>,
    minimap: Option<Res<MinimapRendererState>>,
) {
    metrics.frame_count += 1;

    let mut combined = rendering::instance::InstanceBatchStatsSnapshot::default();
    if let Some(batch) = sprite_batch {
        let snap = batch.batch.stats();
        combined.updates += snap.updates;
        combined.writes += snap.writes;
        combined.dedup_skips += snap.dedup_skips;
        combined.adds += snap.adds;
        combined.removes += snap.removes;
    }
    if let Some(effects) = effect_manager {
        let snap = effects.effect_manager.stats();
        combined.updates += snap.updates;
        combined.writes += snap.writes;
        combined.dedup_skips += snap.dedup_skips;
        combined.adds += snap.adds;
        combined.removes += snap.removes;
    }
    if let Some(minimap) = minimap {
        let snap = minimap.renderer.marker_stats();
        combined.updates += snap.updates;
        combined.writes += snap.writes;
        combined.dedup_skips += snap.dedup_skips;
        combined.adds += snap.adds;
        combined.removes += snap.removes;
    }

    metrics.last_instance_updates = combined
        .updates
        .saturating_sub(metrics.prev_instance_updates);
    metrics.last_instance_writes = combined.writes.saturating_sub(metrics.prev_instance_writes);
    metrics.last_instance_dedup_skips = combined
        .dedup_skips
        .saturating_sub(metrics.prev_instance_dedup_skips);
    metrics.last_instance_adds = combined.adds.saturating_sub(metrics.prev_instance_adds);
    metrics.last_instance_removes = combined
        .removes
        .saturating_sub(metrics.prev_instance_removes);
    metrics.prev_instance_updates = combined.updates;
    metrics.prev_instance_writes = combined.writes;
    metrics.prev_instance_dedup_skips = combined.dedup_skips;
    metrics.prev_instance_adds = combined.adds;
    metrics.prev_instance_removes = combined.removes;

    metrics.last_fps = if metrics.last_update_us > 0 {
        1_000_000.0 / metrics.last_update_us as f32
    } else {
        0.0
    };
    metrics.last_slint_sets_attempted = metrics.acc_slint_sets_attempted;
    metrics.last_slint_sets_sent = metrics.acc_slint_sets_sent;
    metrics.last_slint_model_rebuilds = metrics.acc_slint_model_rebuilds;
    metrics.last_slint_core_syncs = metrics.acc_slint_core_syncs;
    metrics.last_repaints = metrics.acc_repaints;
    metrics.acc_slint_sets_attempted = 0;
    metrics.acc_slint_sets_sent = 0;
    metrics.acc_slint_model_rebuilds = 0;
    metrics.acc_slint_core_syncs = 0;
    metrics.acc_repaints = 0;
}

#[derive(Resource)]
pub struct SpriteSceneState {
    pub scene: SpriteScene,
}

#[derive(Resource)]
pub struct UnifiedSpriteBatchState {
    pub batch: UnifiedSpriteBatch,
}

#[derive(Resource, Clone, Debug)]
pub struct StorageConfig {
    pub root: std::path::PathBuf,
}

impl StorageConfig {
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }

    pub fn data_squashfs_path(&self) -> std::path::PathBuf {
        self.root.join("data.squashfs")
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
            assets.tiles_width,
            assets.tiles_height,
            assets.marker_icon_ktx2,
            assets.marker_icon_width,
            assets.marker_icon_height,
            config.layout,
        )?;
        let camera = CameraState::new(
            UVec2::new(width, height),
            &renderer_state.device,
            config.zoom,
        );

        Ok(Self {
            renderer,
            camera,
            config,
            assets,
            visible: false,
        })
    }

    pub fn flush_pending(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.renderer.flush_pending(encoder);
        self.camera.flush_pending(encoder);
    }

    pub fn finish_uploads(&mut self) {
        self.renderer.finish_uploads();
        self.camera.finish_uploads();
    }

    pub fn recall_uploads(&mut self) {
        self.renderer.recall_uploads();
        self.camera.recall_uploads();
    }
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
        scene: &SpriteScene,
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
        let camera = rendering::scene::CameraState::new_with_screen_offset(
            UVec2::new(size, size),
            &renderer.device,
            1.0,
            Vec2::new(0.0, camera_offset_y),
        );

        Self {
            texture: texture.texture,
            view: texture.view,
            depth_texture,
            batch: players::PlayerBatch::new(&renderer.device, scene),
            camera,
            dirty: true,
            version: 0,
        }
    }
}

impl PlayerPortraitState {
    pub fn new(renderer: &RendererState, scene: &SpriteScene) -> Self {
        Self {
            target: PortraitRenderTarget::new(renderer, scene, "player_portrait", 64, -42.0),
        }
    }
}

impl ProfilePortraitState {
    pub fn new(renderer: &RendererState, scene: &SpriteScene) -> Self {
        Self {
            target: PortraitRenderTarget::new(renderer, scene, "profile_portrait", 128, -32.0),
        }
    }
}

impl LobbyPortraitRenderer {
    pub fn new(renderer: &RendererState, scene: &SpriteScene) -> Self {
        let portrait_size = 64;
        let depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer.device,
            portrait_size,
            portrait_size,
            "lobby_portrait_depth",
        );
        let camera = rendering::scene::CameraState::new_with_screen_offset(
            UVec2::new(portrait_size, portrait_size),
            &renderer.device,
            1.0,
            Vec2::new(0.0, -42.0),
        );

        Self {
            batch: players::PlayerBatch::new(&renderer.device, scene),
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
    pub composite_bind_group: Option<wgpu::BindGroup>,
}

/// Offscreen world target for the darkness composite.
#[derive(Resource)]
pub struct SceneColorState {
    pub color_texture: rendering::texture::Texture,
}

/// Darkness renderer plus per-frame light sources and metadata.
#[derive(Resource)]
pub struct DarknessState {
    pub renderer: rendering::scene::darkness::DarknessRenderer,
    pub metadata: Option<crate::lighting::LightMetadata>,
    pub sources: Vec<rendering::scene::darkness::LightSource>,
    pub map_id: u16,
    pub is_dark_map: bool,
    /// Cached composite bind group, invalidated on resize and map changes.
    pub composite_bind_group: Option<wgpu::BindGroup>,
    /// Most recent light level from the server, reapplied on map changes.
    pub last_light_level: Option<u8>,
}

impl DarknessState {
    /// Whether the darkness composite must run for the current map.
    pub fn needs_composite(&self) -> bool {
        self.renderer.has_hea() || self.is_dark_map
    }
}

/// Weather overlay; `renderer` is `None` when assets are missing.
#[derive(Resource)]
pub struct WeatherState {
    pub renderer: Option<rendering::scene::weather::WeatherRenderer>,
    pub mode: rendering::scene::weather::WeatherMode,
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
        scene: &SpriteScene,
        gender: u8,
        hair_style: u8,
        hair_color: u8,
        armor_id: u16,
        version: u32,
    ) -> Self {
        Self {
            target: Some(PortraitRenderTarget::new(
                renderer,
                scene,
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
        let display_w = self.display_size.0.max(1) as f32;
        self.dpi_scale * self.render_size.0 as f32 / display_w
    }

    pub fn display_scale(&self) -> f32 {
        self.display_size.0.max(1) as f32 / self.render_size.0.max(1) as f32
    }

    fn recalculate(&mut self) {
        let zoom = self.user_zoom.clamp(0.1, 5.0);

        if zoom < 0.5 {
            self.render_size = self.display_size;
            self.camera_zoom = zoom;
            return;
        }

        let render_w = (self.display_size.0 as f32 / zoom).round().max(1.0);
        let render_h = (self.display_size.1 as f32 / zoom).round().max(1.0);
        self.render_size = (render_w as u32, render_h as u32);
        self.camera_zoom = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::ZoomState;

    fn state(display: (u32, u32), zoom: f32) -> ZoomState {
        ZoomState::new(display.0, display.1, 1.0, zoom, true)
    }

    #[test]
    fn fractional_zoom_renders_at_display_over_zoom() {
        let s = state((1280, 800), 1.5);
        assert_eq!(s.render_size, (853, 533));
        assert_eq!(s.camera_zoom, 1.0);
        assert!((s.display_scale() - 1280.0 / 853.0).abs() < 1e-3);
    }

    #[test]
    fn integer_zoom_renders_at_exact_scale() {
        let s = state((1280, 800), 2.0);
        assert_eq!(s.render_size, (640, 400));
        assert_eq!(s.camera_zoom, 1.0);
        assert!((s.display_scale() - 2.0).abs() < 1e-3);
    }

    #[test]
    fn zoom_out_renders_larger_than_display() {
        let s = state((1280, 800), 0.5);
        assert_eq!(s.render_size, (2560, 1600));
        assert_eq!(s.camera_zoom, 1.0);
        assert!((s.display_scale() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn deep_zoom_out_falls_back_to_native_render() {
        let s = state((1280, 800), 0.25);
        assert_eq!(s.render_size, (1280, 800));
        assert!((s.camera_zoom - 0.25).abs() < 1e-3);
    }

    #[test]
    fn cursor_scale_matches_render_ratio() {
        let mut s = state((1280, 800), 1.5);
        s.dpi_scale = 2.0;
        let expected = 2.0 * 853.0 / 1280.0;
        assert!((s.cursor_to_render_scale() - expected).abs() < 1e-3);
    }
}
