use crate::app_state::AppState;
use crate::slint_support::frame_exchange::{BackBufferPool, ControlMessage, FrameChannels};
use crate::{
    Camera, CreatureAssetStoreState, CreatureBatchState, EffectManagerState, ItemAssetStoreState,
    ItemBatchState, MapRendererState, PlayerAssetStoreState, PlayerBatchState, RendererState,
    WindowSurface, game_files,
};
use async_std::task::block_on;
use bevy::prelude::*;
use rendering::scene::{EffectManager, creatures, items, players};

use crate::ecs::components::HoverName;
use crate::ecs::interaction::HoveredEntity;

pub struct GameWorldRenderPlugin;

impl Plugin for GameWorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenderManagersPlugin)
            .init_resource::<PendingResize>()
            .init_resource::<CurrentHoverLabel>()
            .add_systems(
                PreUpdate,
                apply_pending_resize
                    .run_if(resource_exists::<RendererState>)
                    .run_if(resource_exists::<Camera>),
            )
            .add_systems(
                PostUpdate,
                update_hover_labels.run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Last,
                draw_frame
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<FrameChannels>),
            );
    }
}

pub struct RenderManagersPlugin;

impl Plugin for RenderManagersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            init_render_managers_after_gamefiles
                .run_if(in_state(AppState::MainMenu))
                .run_if(needs_render_managers),
        )
        .add_systems(
            Update,
            init_render_managers_after_gamefiles
                .run_if(in_state(AppState::InGame))
                .run_if(needs_render_managers),
        );
    }
}

#[allow(dead_code)]
pub struct WebUi {
    // Placeholder for future webview integration
}

// Initialize GPU-backed managers (creatures/players) once assets are installed
fn init_render_managers_after_gamefiles(
    mut commands: Commands,
    files: Option<Res<game_files::GameFiles>>,
    renderer: Option<Res<RendererState>>,
    camera: Option<Res<Camera>>,
    existing_creatures: Option<Res<CreatureAssetStoreState>>,
    existing_players: Option<Res<PlayerAssetStoreState>>,
    existing_items: Option<Res<ItemAssetStoreState>>,
    existing_effects: Option<Res<EffectManagerState>>,
    _existing_portrait: Option<Res<crate::resources::PlayerPortraitState>>,
) {
    let (files, renderer, camera) = match (files, renderer, camera) {
        (Some(f), Some(r), Some(c)) => (f, r, c),
        _ => return,
    };

    if existing_creatures.is_none() {
        let store = block_on(creatures::CreatureAssetStore::new(
            &renderer.device,
            &renderer.queue,
            &files.inner().archive(),
        ));
        let batch = creatures::CreatureBatch::new(&renderer.device, &store);
        commands.insert_resource(CreatureAssetStoreState { store });
        commands.insert_resource(CreatureBatchState { batch });
    }

    if existing_players.is_none() {
        let store = players::PlayerAssetStore::new(
            &renderer.device,
            &renderer.queue,
            &files.inner().archive(),
        );
        let batch = players::PlayerBatch::new(&renderer.device, &store);

        // Portrait initialization
        let portrait_size = 64;
        let texture = rendering::texture::Texture::create_render_texture(
            &renderer.device,
            "player_portrait",
            portrait_size,
            portrait_size,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer.device,
            portrait_size,
            portrait_size,
            "portrait_depth",
        );
        let mut portrait_camera = rendering::scene::CameraState::new(
            glam::UVec2::new(portrait_size, portrait_size),
            &renderer.device,
            1.0,
        );
        // Center on head/upper torso
        portrait_camera.set_screen_offset(&renderer.queue, 0.0, -42.0);

        let portrait_batch = players::PlayerBatch::new(&renderer.device, &store);

        commands.insert_resource(crate::resources::PlayerPortraitState {
            view: texture.view,
            texture: texture.texture,
            depth_texture,
            batch: portrait_batch,
            camera: portrait_camera,
            dirty: true,
            version: 0,
        });

        // Profile Portrait initialization (larger render for profile panel)
        let profile_size = 128;
        let p_texture = rendering::texture::Texture::create_render_texture(
            &renderer.device,
            "profile_portrait",
            profile_size,
            profile_size,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        let p_depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer.device,
            profile_size,
            profile_size,
            "profile_portrait_depth",
        );
        let mut p_camera = rendering::scene::CameraState::new(
            glam::UVec2::new(profile_size, profile_size),
            &renderer.device,
            1.0,
        );
        // Center on the middle of the player
        p_camera.set_screen_offset(&renderer.queue, 0.0, -32.0);

        let p_batch = players::PlayerBatch::new(&renderer.device, &store);

        commands.insert_resource(crate::resources::ProfilePortraitState {
            view: p_texture.view,
            texture: p_texture.texture,
            depth_texture: p_depth_texture,
            batch: p_batch,
            camera: p_camera,
            dirty: true,
            version: 0,
        });

        commands.insert_resource(PlayerAssetStoreState { store });
        commands.insert_resource(PlayerBatchState { batch });
    }

    if existing_items.is_none() {
        let store =
            items::ItemAssetStore::new(&renderer.device, &renderer.queue, &files.inner().archive());
        let batch = items::ItemBatch::new(&renderer.device, &store);
        commands.insert_resource(ItemAssetStoreState { store });
        commands.insert_resource(ItemBatchState { batch });
    }

    if existing_effects.is_none() {
        commands.insert_resource(EffectManagerState {
            effect_manager: EffectManager::new(
                &renderer.device,
                &renderer.queue,
                &files.inner().archive(),
                &camera.camera.bind_group_layout,
            ),
        });
    }
}

fn needs_render_managers(
    files: Option<Res<game_files::GameFiles>>,
    renderer: Option<Res<RendererState>>,
    camera: Option<Res<Camera>>,
    existing_creatures: Option<Res<CreatureAssetStoreState>>,
    existing_players: Option<Res<PlayerAssetStoreState>>,
    existing_items: Option<Res<ItemAssetStoreState>>,
    existing_effects: Option<Res<EffectManagerState>>,
) -> bool {
    files.is_some()
        && renderer.is_some()
        && camera.is_some()
        && (existing_creatures.is_none()
            || existing_players.is_none()
            || existing_items.is_none()
            || existing_effects.is_none())
}

#[derive(Resource, Default)]
pub struct PendingResize {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub dirty: bool,
}

fn apply_pending_resize(
    mut pending: ResMut<PendingResize>,
    mut window_surface: NonSendMut<WindowSurface>,
    mut renderer_state: ResMut<RendererState>,
    mut camera: ResMut<Camera>,
    _web_ui: Option<NonSend<WebUi>>,
    mut pool: ResMut<BackBufferPool>,
) {
    if !pending.dirty || pending.width == 0 || pending.height == 0 {
        return;
    }

    window_surface.width = pending.width;
    window_surface.height = pending.height;
    window_surface.scale_factor = pending.scale;

    let RendererState { device, scene, .. } = &mut *renderer_state;
    scene.resize_depth_texture(device, pending.width, pending.height);

    camera.camera.resize(
        &renderer_state.queue,
        (pending.width, pending.height).into(),
        pending.scale,
    );

    // Reallocate pool textures to new resolution so next frame can render immediately
    pool.0.clear();
    for label in ["Back Buffer", "Inflight Buffer", "Front Seed"] {
        let tex = renderer_state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: pending.width,
                    height: pending.height,
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
        pool.0.push(tex);
    }

    pending.dirty = false;
}

fn draw_frame(
    window_surface: NonSendMut<WindowSurface>,
    render_hardware: Res<RendererState>,
    camera: Res<Camera>,
    map_renderer_state: Option<Res<MapRendererState>>,
    creature_batch_state: Option<Res<CreatureBatchState>>,
    item_batch_state: Option<Res<ItemBatchState>>,
    player_batch_state: Option<Res<PlayerBatchState>>,
    effect_manager_state: Option<Res<EffectManagerState>>,
    channels: Res<FrameChannels>,
    mut pool: ResMut<BackBufferPool>,
    mut pending: ResMut<PendingResize>,
) {
    if window_surface.width == 0 || window_surface.height == 0 {
        return;
    }

    // Drain control messages: handle ResizeBuffers by marking PendingResize and skipping frame
    while let Ok(msg) = channels.control_rx.try_recv() {
        match msg {
            ControlMessage::ResizeBuffers {
                width,
                height,
                scale,
            } => {
                pending.width = width;
                pending.height = height;
                pending.scale = scale;
                pending.dirty = true;
                return;
            }
            ControlMessage::ReleaseFrontBufferTexture { texture } => {
                // Discard textures that no longer match the current surface size.
                if texture.width() == window_surface.width
                    && texture.height() == window_surface.height
                {
                    pool.0.push(texture);
                }
            }
        }
    }

    // Acquire a back buffer from the pool (provided by UI via ReleaseFrontBufferTexture)
    let back = loop {
        match pool.0.pop() {
            Some(t) if t.width() == window_surface.width && t.height() == window_surface.height => {
                break t;
            }
            Some(_) => {
                // Drop mismatched texture and keep looking for a valid one.
            }
            None => {
                // Fallback: allocate a fresh texture that matches the current surface.
                let tex = render_hardware
                    .device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("GameFrameFallback"),
                        size: wgpu::Extent3d {
                            width: window_surface.width,
                            height: window_surface.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_SRC
                            | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                break tex;
            }
        }
    };
    let view = back.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = render_hardware
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    // (Global texture uploader removed; direct queue submissions now occur at load time.)

    // Background pass: draw while not InGame, and also as a fallback when InGame but no map is loaded yet
    let color_load_op = wgpu::LoadOp::Clear(wgpu::Color::BLACK);

    // world scene pass (only runs while InGame)
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_load_op,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &render_hardware.scene.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        render_pass.set_stencil_reference(0);
        render_pass.set_pipeline(&render_hardware.scene.pipeline);
        render_pass.set_bind_group(1, &camera.camera.camera_bind_group, &[]);
        if let Some(m) = map_renderer_state {
            m.map_renderer.render(&mut render_pass);
        }
        if let Some(im) = &item_batch_state {
            im.batch.render(&mut render_pass);
        }
        if let Some(cm) = creature_batch_state {
            cm.batch.render(&mut render_pass);
        }
        if let Some(pb) = &player_batch_state {
            pb.batch.render(&mut render_pass);
        }
        if let Some(em) = &effect_manager_state {
            em.effect_manager
                .render(&mut render_pass, &camera.camera.camera_bind_group);
        }
    }

    render_hardware.queue.submit([encoder.finish()]);

    // Send the just-rendered back buffer to UI as front buffer
    let _ = channels.front_buffer_tx.try_send(back);
}

/// Track which entity currently has a hover label so we can remove it when hover changes.
#[derive(Resource, Default)]
pub struct CurrentHoverLabel(pub Option<Entity>);

/// System to manage HoverLabel components for hovered entities.
/// Adds a HoverLabel to the currently hovered entity (if it has a HoverName),
/// and removes it when the entity is no longer hovered.
fn update_hover_labels(
    mut commands: Commands,
    hovered_entity: Res<HoveredEntity>,
    mut current_label: ResMut<CurrentHoverLabel>,
    query: Query<&HoverName>,
) {
    let new_hovered = hovered_entity.0;

    // If the hovered entity changed, remove the old label
    if current_label.0 != new_hovered {
        if let Some(old_entity) = current_label.0 {
            // Only try to remove if the entity still exists
            if let Ok(mut entity_commands) = commands.get_entity(old_entity) {
                entity_commands.remove::<crate::ecs::components::HoverLabel>();
            }
        }
        current_label.0 = None;
    }

    // If there's a new hovered entity with a HoverName, add a HoverLabel
    if let Some(entity) = new_hovered {
        if let Ok(hover_name) = query.get(entity) {
            commands
                .entity(entity)
                .insert(crate::ecs::components::HoverLabel::new(
                    &hover_name.name,
                    hover_name.color,
                ));
            current_label.0 = Some(entity);
        }
    }
}
