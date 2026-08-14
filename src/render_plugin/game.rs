use crate::app_state::AppState;
use crate::slint_support::frame_exchange::{BackBufferPool, ControlMessage, FrameChannels};
use crate::{
    Camera, DarknessState, EffectManagerState, MapRendererState, MinimapRendererState,
    RendererState, SceneColorState, SpriteSceneState, TranslucentPlayerPassState,
    UnifiedSpriteBatchState, WeatherState, WindowSurface, game_files,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rendering::scene::{EffectManager, UnifiedSpriteBatch, unified_batch::SpriteScene};

use crate::ecs::components::HoverName;
use crate::ecs::interaction::HoveredEntity;
use crate::resources::{DebugLog, FrameMetrics};

pub struct GameWorldRenderPlugin;

/// `draw_frame` (and the metrics fold that must follow it) in the `Last` schedule.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameWorldDrawSet;

/// Keeps `draw_frame` under Bevy's system-parameter arity limit.
#[derive(SystemParam)]
pub struct DrawFrameDebug<'w> {
    pub metrics: ResMut<'w, FrameMetrics>,
    pub log: ResMut<'w, DebugLog>,
    pub time: Res<'w, Time>,
}

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
                    .run_if(resource_exists::<FrameChannels>)
                    .in_set(GameWorldDrawSet),
            )
            .add_systems(
                Last,
                crate::resources::finish_frame_metrics.after(GameWorldDrawSet),
            )
            .add_systems(
                Last,
                crate::sys_timing::collect_system_timings
                    .after(crate::resources::finish_frame_metrics),
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
    existing_scene: Option<Res<SpriteSceneState>>,
    existing_batch: Option<Res<UnifiedSpriteBatchState>>,
    existing_effects: Option<Res<EffectManagerState>>,
    _existing_portrait: Option<Res<crate::resources::PlayerPortraitState>>,
    existing_character_preview: Option<Res<crate::resources::CharacterCreatorPreviewState>>,
    existing_translucent_players: Option<Res<TranslucentPlayerPassState>>,
    existing_scene_color: Option<Res<SceneColorState>>,
    existing_darkness: Option<Res<DarknessState>>,
    existing_weather: Option<Res<WeatherState>>,
) {
    let (files, renderer, camera) = match (files, renderer, camera) {
        (Some(f), Some(r), Some(c)) => (f, r, c),
        _ => return,
    };

    // The sprite scene (shared atlas + stores) and the single instance batch
    // are one unit: create them together so every store allocates from the
    // same atlas and the main scene draws everything in a single call.
    let needs_sprite_scene = existing_scene.is_none() || existing_batch.is_none();

    if needs_sprite_scene {
        let scene = SpriteScene::new(&renderer.device, &renderer.queue, &files.inner().archive());
        let batch = UnifiedSpriteBatch::new(&renderer.device, &scene);

        commands.insert_resource(crate::resources::PlayerPortraitState::new(
            &renderer, &scene,
        ));
        commands.insert_resource(crate::resources::ProfilePortraitState::new(
            &renderer, &scene,
        ));
        commands.insert_resource(crate::resources::LobbyPortraitRenderer::new(
            &renderer, &scene,
        ));

        let (gender, hair_style, hair_color, armor_id, version) = existing_character_preview
            .as_ref()
            .map(|preview| {
                (
                    preview.gender,
                    preview.hair_style,
                    preview.hair_color,
                    preview.armor_id,
                    preview.version,
                )
            })
            .unwrap_or((1, 0, 0, 1, 0));
        commands.insert_resource(crate::resources::CharacterCreatorPreviewState::with_target(
            &renderer, &scene, gender, hair_style, hair_color, armor_id, version,
        ));

        commands.insert_resource(SpriteSceneState { scene });
        commands.insert_resource(UnifiedSpriteBatchState { batch });
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

    if existing_translucent_players.is_none() {
        commands.insert_resource(TranslucentPlayerPassState {
            color_texture: rendering::texture::Texture::create_render_texture(
                &renderer.device,
                "translucent_player_color",
                camera.camera.camera.width as u32,
                camera.camera.camera.height as u32,
                wgpu::TextureFormat::Rgba8Unorm,
            ),
            depth_texture: rendering::texture::Texture::create_depth_texture(
                &renderer.device,
                camera.camera.camera.width as u32,
                camera.camera.camera.height as u32,
                "translucent_player_depth",
            ),
            composite_bind_group: None,
        });
    }

    if existing_scene_color.is_none() {
        commands.insert_resource(SceneColorState {
            color_texture: rendering::texture::Texture::create_render_texture(
                &renderer.device,
                "scene_color",
                camera.camera.camera.width as u32,
                camera.camera.camera.height as u32,
                wgpu::TextureFormat::Rgba8Unorm,
            ),
        });
    }

    if existing_darkness.is_none() {
        let mut darkness_renderer =
            rendering::scene::darkness::DarknessRenderer::new(&renderer.device, &renderer.queue);
        let archive = files.inner().archive();
        darkness_renderer.set_masks(
            &renderer.device,
            &renderer.queue,
            crate::lighting::load_light_mask(archive, "mask101"),
            crate::lighting::load_light_mask(archive, "mask102"),
        );
        commands.insert_resource(DarknessState {
            renderer: darkness_renderer,
            metadata: None,
            sources: Vec::new(),
            map_id: 0,
            is_dark_map: false,
            composite_bind_group: None,
            last_light_level: None,
        });
    }

    if existing_weather.is_none() {
        let renderer = crate::weather::load_weather_assets(files.inner().archive()).map(|assets| {
            rendering::scene::weather::WeatherRenderer::new(
                &renderer.device,
                &renderer.queue,
                &assets,
            )
        });
        commands.insert_resource(WeatherState {
            renderer,
            mode: rendering::scene::weather::WeatherMode::None,
        });
    }
}

fn needs_render_managers(
    files: Option<Res<game_files::GameFiles>>,
    renderer: Option<Res<RendererState>>,
    camera: Option<Res<Camera>>,
    existing_scene: Option<Res<SpriteSceneState>>,
    existing_batch: Option<Res<UnifiedSpriteBatchState>>,
    existing_effects: Option<Res<EffectManagerState>>,
    existing_translucent_players: Option<Res<TranslucentPlayerPassState>>,
    existing_scene_color: Option<Res<SceneColorState>>,
    existing_darkness: Option<Res<DarknessState>>,
    existing_weather: Option<Res<WeatherState>>,
) -> bool {
    files.is_some()
        && renderer.is_some()
        && camera.is_some()
        && (existing_scene.is_none()
            || existing_batch.is_none()
            || existing_effects.is_none()
            || existing_translucent_players.is_none()
            || existing_scene_color.is_none()
            || existing_darkness.is_none()
            || existing_weather.is_none())
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
    minimap: Option<ResMut<MinimapRendererState>>,
    translucent_players: Option<ResMut<TranslucentPlayerPassState>>,
    scene_color: Option<ResMut<SceneColorState>>,
    darkness_state: Option<ResMut<DarknessState>>,
) {
    if !pending.dirty || pending.width == 0 || pending.height == 0 {
        return;
    }

    window_surface.width = pending.width;
    window_surface.height = pending.height;
    window_surface.scale_factor = pending.scale;

    let RendererState { device, scene, .. } = &mut *renderer_state;
    scene.resize_depth_texture(device, pending.width, pending.height);

    camera
        .camera
        .resize((pending.width, pending.height).into(), pending.scale);

    if let Some(mut minimap) = minimap {
        let zoom = minimap.config.zoom;
        minimap
            .camera
            .resize((pending.width, pending.height).into(), zoom);
    }

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

    if let Some(mut translucent_players) = translucent_players {
        translucent_players.color_texture = rendering::texture::Texture::create_render_texture(
            &renderer_state.device,
            "translucent_player_color",
            pending.width,
            pending.height,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        translucent_players.depth_texture = rendering::texture::Texture::create_depth_texture(
            &renderer_state.device,
            pending.width,
            pending.height,
            "translucent_player_depth",
        );
        translucent_players.composite_bind_group = None;
    }

    if let Some(mut scene_color) = scene_color {
        scene_color.color_texture = rendering::texture::Texture::create_render_texture(
            &renderer_state.device,
            "scene_color",
            pending.width,
            pending.height,
            wgpu::TextureFormat::Rgba8Unorm,
        );
    }

    if let Some(mut darkness) = darkness_state {
        darkness.composite_bind_group = None;
    }

    pending.dirty = false;
}

fn draw_frame(
    window_surface: NonSendMut<WindowSurface>,
    render_hardware: Res<RendererState>,
    mut camera: ResMut<Camera>,
    mut map_renderer_state: Option<ResMut<MapRendererState>>,
    sprite_batch_state: Option<Res<UnifiedSpriteBatchState>>,
    effect_manager_state: Option<Res<EffectManagerState>>,
    mut minimap_renderer_state: Option<ResMut<MinimapRendererState>>,
    mut translucent_player_pass_state: Option<ResMut<TranslucentPlayerPassState>>,
    scene_color_state: Option<Res<SceneColorState>>,
    mut darkness_state: Option<ResMut<DarknessState>>,
    mut weather_state: Option<ResMut<WeatherState>>,
    channels: Res<FrameChannels>,
    mut pool: ResMut<BackBufferPool>,
    mut pending: ResMut<PendingResize>,
    mut debug: DrawFrameDebug,
) {
    let draw_start = std::time::Instant::now();
    debug.metrics.last_draw_us = 0;
    debug.metrics.last_queue_submits = 0;
    debug.metrics.last_draw_passes = 0;
    debug.metrics.last_texture_handoffs = 0;

    if window_surface.width == 0 || window_surface.height == 0 {
        return;
    }

    let mut passes = 0u32;

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
                debug.log.push(format!(
                    "render target resized to {}x{} (scale {:.2})",
                    width, height, scale
                ));
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

    // Deferred writes go through the staging belts first so the copy commands
    // are recorded before the passes that draw them.
    camera.camera.flush_pending(&mut encoder);
    if let Some(mr) = map_renderer_state.as_mut() {
        mr.map_renderer.flush_pending(&mut encoder);
    }
    if let Some(sb) = sprite_batch_state.as_ref() {
        sb.batch.flush_pending(&mut encoder);
    }
    if let Some(em) = effect_manager_state.as_ref() {
        em.effect_manager.flush_pending(&mut encoder);
    }
    if let Some(mm) = minimap_renderer_state.as_mut() {
        mm.flush_pending(&mut encoder);
    }

    // Update the darkness uniform for this frame.
    if let Some(darkness) = darkness_state.as_deref_mut() {
        if darkness.needs_composite() {
            darkness.renderer.update_uniform(
                [camera.camera.position().x, camera.camera.position().y],
                camera.camera.zoom(),
                [camera.camera.camera.width, camera.camera.camera.height],
                &darkness.sources,
            );
            darkness.renderer.flush_pending(&mut encoder);
        }
    }

    // (Global texture uploader removed; direct queue submissions now occur at load time.)

    // Background pass: draw while not InGame, and also as a fallback when InGame but no map is loaded yet
    let color_load_op = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
    // The darkness composite is a fullscreen pass over an offscreen scene;
    // maps without a light map or dark flag render straight to the back buffer.
    let scene_target = match (&scene_color_state, &darkness_state) {
        (Some(scene_color), Some(darkness)) if darkness.needs_composite() => {
            &scene_color.color_texture.view
        }
        _ => &view,
    };

    // world scene pass (only runs while InGame)
    {
        // wgpu requires every bind group / vertex buffer referenced by a render
        // pass to outlive it, so borrow the renderer resources for the whole pass.
        let map_renderer = map_renderer_state.as_ref().map(|m| &m.map_renderer);
        let sprite_batch = sprite_batch_state.as_ref().map(|sb| &sb.batch);
        let effect_manager = effect_manager_state.as_ref().map(|em| &em.effect_manager);

        passes += 1;
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: scene_target,
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
        if let Some(m) = map_renderer {
            m.render(&mut render_pass);
        }
        if let Some(sb) = sprite_batch {
            sb.render(&mut render_pass);
        }
        if let Some(em) = effect_manager {
            em.render(&mut render_pass, &camera.camera.camera_bind_group);
        }

        // Transparent walls (sotp.dat bit 0x80) composite with screen blending after
        // the opaque scene so windows/fences blend with everything behind them.
        // Depth testing keeps correct occlusion with the rest of the world.
        if let Some(m) = map_renderer {
            if m.has_screen_blend_walls() {
                render_pass.set_pipeline(&render_hardware.scene.screen_blend_pipeline);
                m.render_screen_blend(&mut render_pass);
            }
        }
    }

    if let (Some(sb), Some(translucent_player_pass_state)) =
        (&sprite_batch_state, &mut translucent_player_pass_state)
    {
        if sb.batch.translucent_count() > 0 {
            passes += 1;
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Translucent Player Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &translucent_player_pass_state.color_texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &translucent_player_pass_state.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            render_pass.set_stencil_reference(0);
            render_pass.set_pipeline(&render_hardware.scene.translucent_player_pipeline);
            render_pass.set_bind_group(1, &camera.camera.camera_bind_group, &[]);
            sb.batch.render(&mut render_pass);
        }
    }

    if let (Some(sb), Some(translucent_player_pass_state)) =
        (&sprite_batch_state, &mut translucent_player_pass_state)
    {
        if sb.batch.translucent_count() > 0 {
            let composite_bind_group =
                if translucent_player_pass_state.composite_bind_group.is_none() {
                    let bind_group = render_hardware
                        .scene
                        .create_translucent_player_composite_bind_group(
                            &render_hardware.device,
                            &translucent_player_pass_state.color_texture.view,
                            &translucent_player_pass_state.depth_texture.view,
                        );
                    translucent_player_pass_state.composite_bind_group = Some(bind_group.clone());
                    bind_group
                } else {
                    translucent_player_pass_state
                        .composite_bind_group
                        .clone()
                        .unwrap()
                };

            passes += 1;
            let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Translucent Player Composite Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            composite_pass
                .set_pipeline(&render_hardware.scene.translucent_player_composite_pipeline);
            composite_pass.set_bind_group(0, &composite_bind_group, &[]);
            composite_pass.draw(0..3, 0..1);
        }
    }

    // Blend the offscreen scene into the back buffer with the darkness overlay.
    if let (Some(scene_color), Some(darkness_state)) = (&scene_color_state, &mut darkness_state) {
        if darkness_state.needs_composite() {
            let darkness_bind_group = if darkness_state.composite_bind_group.is_none() {
                let bind_group = darkness_state.renderer.create_bind_group(
                    &render_hardware.device,
                    &render_hardware.scene.darkness_bind_group_layout,
                    &scene_color.color_texture.view,
                );
                darkness_state.composite_bind_group = Some(bind_group.clone());
                bind_group
            } else {
                darkness_state.composite_bind_group.clone().unwrap()
            };

            passes += 1;
            let mut darkness_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Darkness Composite Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            darkness_pass.set_pipeline(&render_hardware.scene.darkness_pipeline);
            darkness_pass.set_bind_group(0, &darkness_bind_group, &[]);
            darkness_pass.draw(0..3, 0..1);
        }
    }

    // Draw weather above the darkened world.
    if let Some(weather_state) = weather_state.as_deref_mut() {
        if let Some(weather) = &mut weather_state.renderer {
            weather.update(
                debug.time.delta_secs(),
                [camera.camera.camera.width, camera.camera.camera.height],
            );
            weather.flush_pending(&mut encoder);
            if weather.is_active() && weather.instance_count() > 0 {
                passes += 1;
                let weather_bind_group = weather.create_bind_group(
                    &render_hardware.device,
                    &render_hardware.scene.weather_bind_group_layout,
                );
                let mut weather_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Weather Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                });
                weather_pass.set_pipeline(&render_hardware.scene.weather_pipeline);
                weather_pass.set_bind_group(0, &weather_bind_group, &[]);
                weather_pass.set_vertex_buffer(0, weather.vertex_buffer().slice(..));
                weather_pass.set_vertex_buffer(1, weather.instance_buffer().slice(..));
                weather_pass.draw(0..6, 0..weather.instance_count());
            }
        }
    }

    if let Some(minimap) = minimap_renderer_state
        .as_ref()
        .filter(|minimap| minimap.visible)
    {
        passes += 1;
        let mut minimap_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Minimap Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &render_hardware.scene.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        minimap
            .renderer
            .render(&mut minimap_pass, &minimap.camera.camera_bind_group);
    }

    debug.metrics.last_map_instances = map_renderer_state
        .as_ref()
        .map_or(0, |m| m.map_renderer.instance_count() as u32);
    debug.metrics.last_sprite_instances = sprite_batch_state.as_ref().map_or(0, |sb| sb.batch.live_len() as u32);
    debug.metrics.last_effect_instances = effect_manager_state
        .as_ref()
        .map_or(0, |em| em.effect_manager.instance_count() as u32);
    debug.metrics.last_weather_instances = weather_state
        .as_ref()
        .and_then(|w| w.renderer.as_ref())
        .map_or(0, |w| w.instance_count());
    debug.metrics.last_minimap_tiles = minimap_renderer_state
        .as_ref()
        .map_or(0, |m| m.renderer.tile_count() as u32);
    debug.metrics.last_minimap_markers = minimap_renderer_state
        .as_ref()
        .map_or(0, |m| m.renderer.marker_count() as u32);
    debug.metrics.last_draw_passes = passes;
    debug.metrics.last_queue_submits = 1;
    debug.metrics.last_texture_handoffs = 1;

    camera.camera.finish_uploads();
    if let Some(mr) = map_renderer_state.as_mut() {
        mr.map_renderer.finish_uploads();
    }
    if let Some(darkness) = darkness_state.as_mut() {
        darkness.renderer.finish_uploads();
    }
    if let Some(weather) = weather_state.as_mut() {
        if let Some(w) = &mut weather.renderer {
            w.finish_uploads();
        }
    }
    if let Some(sb) = sprite_batch_state.as_ref() {
        sb.batch.finish_uploads();
    }
    if let Some(em) = effect_manager_state.as_ref() {
        em.effect_manager.finish_uploads();
    }
    if let Some(mm) = minimap_renderer_state.as_mut() {
        mm.finish_uploads();
    }
    render_hardware.queue.submit([encoder.finish()]);

    camera.camera.recall_uploads();
    if let Some(mr) = map_renderer_state.as_mut() {
        mr.map_renderer.recall_uploads();
    }
    if let Some(darkness) = darkness_state.as_mut() {
        darkness.renderer.recall_uploads();
    }
    if let Some(weather) = weather_state.as_mut() {
        if let Some(w) = &mut weather.renderer {
            w.recall_uploads();
        }
    }
    if let Some(sb) = sprite_batch_state.as_ref() {
        sb.batch.recall_uploads();
    }
    if let Some(em) = effect_manager_state.as_ref() {
        em.effect_manager.recall_uploads();
    }
    if let Some(mm) = minimap_renderer_state.as_mut() {
        mm.recall_uploads();
    }

    debug.metrics.last_draw_us = draw_start.elapsed().as_micros() as u64;

    // Publish only the newest completed frame; recycle any unpublished older one.
    let mut latest_front_buffer = channels
        .latest_front_buffer
        .lock()
        .expect("latest front buffer mutex poisoned");
    if let Some(stale_texture) = latest_front_buffer.replace(back) {
        let _ = channels
            .control_tx
            .try_send(ControlMessage::ReleaseFrontBufferTexture {
                texture: stale_texture,
            });
    }
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
