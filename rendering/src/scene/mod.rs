use crate::texture;
use crate::{Camera, CameraUniform, Instance, InstanceRaw, Vertex};
use glam::UVec2;
use wgpu::util::DeviceExt;
use wgpu::{self};

// pub mod adapters;
pub mod constants;
pub mod creatures;
pub mod effects;
pub mod items;
pub mod map;
pub mod players;

pub use effects::{EffectHandle, EffectManager};
// pub mod sprite;
// pub mod sprite_manager;
pub mod texture_atlas;
pub mod texture_bind;
pub mod utils;

pub use constants::*;
pub use map::animations::{
    AnimationInstanceData, InstanceReference, WorldAnimation, WorldAnimationInstanceData,
};
pub use utils::{get_isometric_coordinate, screen_to_iso_tile, tile_to_screen};

const WALL_ATLAS_WIDTH: usize = 2048;
const WALL_ATLAS_HEIGHT: usize = 4096;

pub struct CameraState {
    size: UVec2,
    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl CameraState {
    pub fn new(size: UVec2, device: &wgpu::Device, zoom: f32) -> Self {
        let camera = Camera::new(size.x as f32, size.y as f32, zoom);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        Self {
            size,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            bind_group_layout: camera_bind_group_layout,
        }
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, new_size: UVec2, scale: f32) {
        self.size = new_size;
        self.camera.width = new_size.x as f32;
        self.camera.height = new_size.y as f32;
        self.camera.zoom = scale;
        self.update(queue);
    }

    pub fn set_position(&mut self, queue: &wgpu::Queue, x: f32, y: f32) {
        let pos = get_isometric_coordinate(x, y);
        self.camera.position = (pos * self.camera.zoom).round() / self.camera.zoom;
        self.update(queue);
    }

    // Directly set a screen-space offset (bypasses isometric conversion) and update the GPU buffer.
    pub fn set_screen_offset(&mut self, queue: &wgpu::Queue, x: f32, y: f32) {
        self.camera.position = glam::Vec2::new(x, y);
        self.update(queue);
    }

    pub fn set_zoom(&mut self, queue: &wgpu::Queue, zoom: f32) {
        self.camera.zoom = zoom;
        self.update(queue);
    }

    pub fn zoom(&self) -> f32 {
        self.camera.zoom
    }

    pub fn position(&self) -> glam::Vec2 {
        self.camera.position
    }

    pub fn set_position_world(&mut self, queue: &wgpu::Queue, x: f32, y: f32) {
        self.set_position(queue, x, y);
    }

    pub fn set_magnification(&mut self, queue: &wgpu::Queue, mag: f32) {
        self.set_zoom(queue, 1.0 / mag.max(0.01));
    }

    pub fn set_tint(&mut self, queue: &wgpu::Queue, r: f32, g: f32, b: f32) {
        self.camera_uniform.tint[0] = r;
        self.camera_uniform.tint[1] = g;
        self.camera_uniform.tint[2] = b;
        self.update(queue);
    }

    pub fn set_xray_size(&mut self, queue: &wgpu::Queue, size: f32) {
        self.camera_uniform.xray_size = size;
        self.update(queue);
    }

    fn update(&mut self, queue: &wgpu::Queue) {
        self.camera_uniform.update_view_proj(&self.camera);
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }
}

pub struct Scene {
    pub pipeline: wgpu::RenderPipeline,
    pub depth_texture: texture::Texture,
    pub depth_bind_group_layout: wgpu::BindGroupLayout,
    pub depth_bind_group: wgpu::BindGroup,
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        texture_format: wgpu::TextureFormat,
    ) -> Self {
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let depth_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                }],
                label: Some("depth_bind_group_layout"),
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
                immediate_size: 0,
            });

        let depth_texture =
            texture::Texture::create_depth_texture(&device, width, height, "scene_depth");

        let depth_sample_view = depth_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("depth_sample_view"),
                aspect: wgpu::TextureAspect::DepthOnly,
                ..Default::default()
            });

        let depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &depth_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth_sample_view),
            }],
            label: Some("depth_bind_group"),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc(), InstanceRaw::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
        });

        Self {
            pipeline: render_pipeline,
            depth_texture,
            depth_bind_group_layout,
            depth_bind_group,
        }
    }

    pub fn resize_depth_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.depth_texture =
            texture::Texture::create_depth_texture(device, width, height, "scene_depth");
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::map::{floor::FloorTile, wall::WallSide};
    use glam::Vec2;

    #[test]
    fn test_simple_map() {
        let wall_height = 100 as f32;

        let width = 2;
        let height = 2;

        let tile_positions = [(-28., -14.), (0., 0.), (-56., 0.), (-28., 14.)];
        let left_wall_positions = [(-28., -86.), (0., -72.), (-56., -72.), (-28., -58.)];
        let right_wall_positions = [(0., -86.), (28., -72.), (-28., -72.), (0., -58.)];

        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;

                assert_eq!(
                    FloorTile::get_position(x as f32, y as f32),
                    Vec2::new(tile_positions[i].0, tile_positions[i].1),
                    "Floor tile position at ({x}, {y})",
                );
                assert_eq!(
                    WallSide::Left.get_position(x as f32, y as f32, wall_height),
                    Vec2::new(left_wall_positions[i].0, left_wall_positions[i].1),
                    "Left wall position at ({x}, {y})",
                );
                assert_eq!(
                    WallSide::Right.get_position(x as f32, y as f32, wall_height),
                    Vec2::new(right_wall_positions[i].0, right_wall_positions[i].1),
                    "Right wall position at ({x}, {y})",
                );
            }
        }
    }

    #[test]
    fn test_wider_map() {
        let wall_height = 47 as f32;

        let width = 4;
        let height = 1;

        let tile_positions = [(-28., -14.), (0., 0.), (28., 14.), (56., 28.)];
        let left_wall_positions = [(-28., -33.), (0., -19.), (28., -5.), (56., 9.)];
        let right_wall_positions = [(0., -33.), (28., -19.), (56., -5.), (84., 9.)];

        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;

                assert_eq!(
                    FloorTile::get_position(x as f32, y as f32),
                    Vec2::new(tile_positions[i].0, tile_positions[i].1),
                    "Floor tile position at ({x}, {y})",
                );
                assert_eq!(
                    WallSide::Left.get_position(x as f32, y as f32, wall_height),
                    Vec2::new(left_wall_positions[i].0, left_wall_positions[i].1),
                    "Left wall position at ({x}, {y})",
                );
                assert_eq!(
                    WallSide::Right.get_position(x as f32, y as f32, wall_height),
                    Vec2::new(right_wall_positions[i].0, right_wall_positions[i].1),
                    "Right wall position at ({x}, {y})",
                );
            }
        }
    }
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    diffuse_texture: &texture::Texture,
    palette_texture: &texture::Texture,
    dye_texture_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&palette_texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&palette_texture.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&dye_texture_view),
            },
        ],
        label: None,
    })
}
