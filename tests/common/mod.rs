use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use formats::game_files::ArxArchive;
use glam::UVec2;
use packets::server;
use std::sync::Arc;
use std::time::Duration;
use talgonite::{
    Camera, PlayerBatchState, RendererState,
    events::{EntityEvent, MapEvent},
};
use wgpu;

pub struct TestScene {
    app: App,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    _archive: ArxArchive,
    maps_dir: String,
    next_entity_id: u32,
}

impl TestScene {
    pub fn new(archive_path: &str, maps_dir: &str) -> Self {
        let archive = ArxArchive::new(archive_path).expect("Failed to open archive");

        let (device, queue) = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
                ..Default::default()
            });

            let adapters = instance.enumerate_adapters(wgpu::Backends::all());
            let adapter = adapters.into_iter().next().expect("No adapters found");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("Test Device"),
                    required_features: wgpu::Features::PUSH_CONSTANTS,
                    required_limits: wgpu::Limits {
                        max_push_constant_size: 16,
                        ..Default::default()
                    },
                    memory_hints: Default::default(),
                    ..Default::default()
                })
                .await
                .expect("Failed to create device");

            (Arc::new(device), Arc::new(queue))
        });

        let surface_format = wgpu::TextureFormat::Rgba8Unorm;
        let scene = rendering::scene::Scene::new(&device, 800, 600, surface_format);
        let camera = rendering::scene::CameraState::new(UVec2::new(800, 600), &device, 1.0);

        let mut app = App::new();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.add_plugins(MinimalPlugins).add_plugins((
            talgonite::CorePlugin,
            talgonite::render_plugin::game::RenderManagersPlugin,
        ));

        app.insert_resource(talgonite::game_files::GameFiles::from_archive(
            archive.clone(),
        ));

        app.insert_resource(talgonite::settings::Settings {
            music_volume: 0.0,
            sfx_volume: 0.0,
            scale: 2.,
            xray_size: talgonite::settings::XRaySize::Off,
            key_bindings: talgonite::settings::KeyBindings::default(),
            servers: vec![],
            current_server_id: None,
            saved_credentials: vec![],
        });

        app.insert_resource(RendererState {
            device: (*device).clone(),
            queue: (*queue).clone(),
            scene,
        })
        .insert_resource(Camera { camera });

        app.finish();
        app.cleanup();

        // Run one update to initialize render managers via RenderManagersPlugin
        app.update();

        Self {
            app,
            device,
            queue,
            _archive: archive,
            maps_dir: maps_dir.to_string(),
            next_entity_id: 1,
        }
    }

    pub fn update(&mut self) {
        self.app.update();
    }

    pub fn advance_time(&mut self, duration: Duration) {
        self.app
            .insert_resource(TimeUpdateStrategy::ManualDuration(duration));
        self.app.update();
        self.app
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    }

    pub fn set_map_info(&mut self, map_info: server::MapInfo, map_data: Arc<[u8]>) {
        let mut map_events = self.app.world_mut().resource_mut::<Messages<MapEvent>>();
        map_events.write(MapEvent::SetInfo(map_info, map_data));
    }

    pub fn load_map(&mut self, map_id: u16, width: u8, height: u8) {
        let map_path = format!("{}/lod{}.map", self.maps_dir, map_id);
        let map_data =
            std::fs::read(&map_path).unwrap_or_else(|_| panic!("Failed to load map: {}", map_path));

        let map_info = server::MapInfo {
            map_id,
            check_sum: 0,
            flags: 0,
            width,
            height,
            name: format!("lod{}", map_id),
        };

        self.set_map_info(map_info, Arc::from(map_data.into_boxed_slice()));
    }

    pub fn display_entities(&mut self, entities: server::DisplayVisibleEntities) {
        let mut entity_events = self.app.world_mut().resource_mut::<Messages<EntityEvent>>();
        entity_events.write(EntityEvent::DisplayEntities(entities));
    }

    pub fn display_player(&mut self, player: server::display_player::DisplayPlayer) {
        let mut entity_events = self.app.world_mut().resource_mut::<Messages<EntityEvent>>();
        entity_events.write(EntityEvent::DisplayPlayer(player));
    }

    pub fn send_player_action(&mut self, action: talgonite::events::PlayerAction) {
        let mut actions = self
            .app
            .world_mut()
            .resource_mut::<Messages<talgonite::events::PlayerAction>>();
        actions.write(action);
    }

    pub fn set_local_player_id(&mut self, id: u32) {
        let mut session_events = self
            .app
            .world_mut()
            .resource_mut::<Messages<talgonite::events::SessionEvent>>();
        session_events.write(talgonite::events::SessionEvent::PlayerId(id));
    }

    pub fn next_entity_id(&mut self) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    pub fn center_camera_on_tile(&mut self, x: f32, y: f32) {
        let mut camera = self.app.world_mut().resource_mut::<Camera>();
        camera.camera.set_position(&self.queue, x, y);
    }

    pub fn set_light_tint(&mut self, r: f32, g: f32, b: f32) {
        let mut camera = self.app.world_mut().resource_mut::<Camera>();
        camera.camera.set_tint(&self.queue, r, g, b);
    }

    pub fn set_light_level(&mut self, kind: packets::server::LightLevelKind) {
        let mut map_events = self.app.world_mut().resource_mut::<Messages<MapEvent>>();
        map_events.write(MapEvent::SetLightLevel(kind));
    }

    pub fn capture(&mut self, width: u32, height: u32) -> Vec<u8> {
        {
            let mut camera = self.app.world_mut().resource_mut::<Camera>();
            camera
                .camera
                .resize(&self.queue, UVec2::new(width, height), 1.0);
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Output Texture"),
            size: wgpu::Extent3d {
                width,
                height,
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

        let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            let world = self.app.world();
            let renderer_state = world.resource::<RendererState>();
            let camera = world.resource::<Camera>();

            render_pass.set_pipeline(&renderer_state.scene.pipeline);
            render_pass.set_bind_group(1, &camera.camera.camera_bind_group, &[]);

            if let Some(map_renderer_state) = world.get_resource::<talgonite::MapRendererState>() {
                map_renderer_state.map_renderer.render(&mut render_pass);
            }

            if let Some(item_manager_state) = world.get_resource::<ItemManagerState>() {
                item_manager_state.item_manager.render(&mut render_pass);
            }

            if let Some(creature_manager_state) = world.get_resource::<CreatureManagerState>() {
                creature_manager_state
                    .creature_manager
                    .render(&mut render_pass);
            }

            if let Some(player_batch_state) = world.get_resource::<PlayerBatchState>() {
                player_batch_state.batch.render(&mut render_pass);
            }
        }

        let bytes_per_pixel = 4;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = 256u32;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;

        let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Output Buffer"),
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
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        let data = buffer_slice.get_mapped_range();

        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let offset = (row * padded_bytes_per_row) as usize;
            rgba_data.extend_from_slice(&data[offset..offset + unpadded_bytes_per_row as usize]);
        }

        drop(data);
        output_buffer.unmap();

        let mut png_data = Vec::new();
        let mut encoder = png::Encoder::new(&mut png_data, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().expect("Failed to write PNG header");
        writer
            .write_image_data(&rgba_data)
            .expect("Failed to write PNG data");
        writer.finish().expect("Failed to finish PNG");

        png_data
    }
}
