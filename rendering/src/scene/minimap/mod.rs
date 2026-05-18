use glam::{UVec2, Vec2};
use wgpu::util::DeviceExt;

use crate::{
    Instance, InstanceBatch, InstanceRaw, SharedInstanceBatch, Vertex, make_quad, texture,
};

const TILE_GRID_DIMENSION: u32 = 4;
const TILE_VERTEX_SIZE: u32 = 32;
const MARKER_VERTEX_SIZE: u32 = 64;

pub const DUAL_MASK_TOP_LEFT: u8 = 1;
pub const DUAL_MASK_TOP_RIGHT: u8 = 2;
pub const DUAL_MASK_BOTTOM_RIGHT: u8 = 4;
pub const DUAL_MASK_BOTTOM_LEFT: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapLayout {
    pub tile_size: Vec2,
    pub marker_size: Vec2,
    pub overlay_alpha: f32,
}

impl Default for MinimapLayout {
    fn default() -> Self {
        Self {
            tile_size: Vec2::new(32.0, 16.0),
            marker_size: Vec2::new(18.0, 18.0),
            overlay_alpha: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapSlice {
    pub tex_min: Vec2,
    pub tex_max: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapTile {
    pub position: Vec2,
    pub atlas_index: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapMarker {
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimapMarkerLayer {
    Player,
    Creature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimapMarkerHandle {
    index: usize,
    layer: MinimapMarkerLayer,
}

impl MinimapMarkerHandle {
    pub fn layer(&self) -> MinimapMarkerLayer {
        self.layer
    }
}

pub struct MinimapRenderer {
    pipeline: wgpu::RenderPipeline,
    tile_bind_group: wgpu::BindGroup,
    player_bind_group: wgpu::BindGroup,
    creature_bind_group: wgpu::BindGroup,
    tile_vertices: Vec<Vertex>,
    atlas_slices: [Option<MinimapSlice>; 16],
    tile_batch: Option<InstanceBatch>,
    player_markers: SharedInstanceBatch,
    creature_markers: SharedInstanceBatch,
    layout: MinimapLayout,
}

impl MinimapRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        tile_bytes: &[u8],
        player_icon_bytes: &[u8],
        creature_icon_bytes: &[u8],
        layout: MinimapLayout,
    ) -> anyhow::Result<Self> {
        let (tile_width, tile_height, _) = texture::Texture::load_ktx2(tile_bytes)?;
        let tile_texture =
            texture::Texture::from_ktx2_rgba8(device, queue, "minimap_tiles", tile_bytes)?;
        let player_texture = texture::Texture::from_ktx2_rgba8(
            device,
            queue,
            "minimap_player_icon",
            player_icon_bytes,
        )?;
        let creature_texture = texture::Texture::from_ktx2_rgba8(
            device,
            queue,
            "minimap_creature_icon",
            creature_icon_bytes,
        )?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Minimap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/minimap.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("minimap_bind_group_layout"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Minimap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, camera_bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Minimap Pipeline"),
            layout: Some(&pipeline_layout),
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
                    format: wgpu::TextureFormat::Rgba8Unorm,
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
        });

        let alpha_buffer = |label: &str| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&[layout.overlay_alpha]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };

        let tile_alpha = alpha_buffer("minimap_tile_alpha");
        let player_alpha = alpha_buffer("minimap_player_alpha");
        let creature_alpha = alpha_buffer("minimap_creature_alpha");

        let make_bind_group = |texture: &texture::Texture, alpha: &wgpu::Buffer, label: &str| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: alpha.as_entire_binding(),
                    },
                ],
                label: Some(label),
            })
        };

        let tile_bind_group =
            make_bind_group(&tile_texture, &tile_alpha, "minimap_tile_bind_group");
        let player_bind_group =
            make_bind_group(&player_texture, &player_alpha, "minimap_player_bind_group");
        let creature_bind_group = make_bind_group(
            &creature_texture,
            &creature_alpha,
            "minimap_creature_bind_group",
        );

        let atlas_slices = atlas_slice_regions(UVec2::new(tile_width, tile_height));
        let tile_vertices = make_quad(TILE_VERTEX_SIZE, TILE_VERTEX_SIZE).to_vec();
        let marker_vertices = make_quad(MARKER_VERTEX_SIZE, MARKER_VERTEX_SIZE).to_vec();

        let player_markers =
            SharedInstanceBatch::new(device, marker_vertices.clone(), player_bind_group.clone());
        let creature_markers =
            SharedInstanceBatch::new(device, marker_vertices.clone(), creature_bind_group.clone());

        Ok(Self {
            pipeline,
            tile_bind_group,
            player_bind_group,
            creature_bind_group,
            tile_vertices,
            atlas_slices,
            tile_batch: None,
            player_markers,
            creature_markers,
            layout,
        })
    }

    pub fn layout(&self) -> MinimapLayout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: MinimapLayout) {
        self.layout = layout;
    }

    pub fn rebuild_tiles(
        &mut self,
        device: &wgpu::Device,
        tiles: impl IntoIterator<Item = MinimapTile>,
    ) {
        let instances = tiles
            .into_iter()
            .flat_map(|tile| self.tile_instances(tile))
            .collect::<Vec<_>>();

        self.tile_batch = Some(InstanceBatch::new(
            device,
            instances,
            self.tile_vertices.clone(),
            self.tile_bind_group.clone(),
        ));
    }

    pub fn clear_tiles(&mut self) {
        self.tile_batch = None;
    }

    pub fn clear_markers(&self) {
        self.player_markers.clear();
        self.creature_markers.clear();
    }

    pub fn clear(&mut self) {
        self.clear_tiles();
        self.clear_markers();
    }

    pub fn add_player_marker(
        &self,
        queue: &wgpu::Queue,
        marker: MinimapMarker,
    ) -> Option<MinimapMarkerHandle> {
        let index = self
            .player_markers
            .add(queue, self.marker_instance(marker.position))?;

        Some(MinimapMarkerHandle {
            index,
            layer: MinimapMarkerLayer::Player,
        })
    }

    pub fn add_creature_marker(
        &self,
        queue: &wgpu::Queue,
        marker: MinimapMarker,
    ) -> Option<MinimapMarkerHandle> {
        let index = self
            .creature_markers
            .add(queue, self.marker_instance(marker.position))?;

        Some(MinimapMarkerHandle {
            index,
            layer: MinimapMarkerLayer::Creature,
        })
    }

    pub fn update_marker(
        &self,
        queue: &wgpu::Queue,
        handle: MinimapMarkerHandle,
        marker: MinimapMarker,
    ) {
        match handle.layer {
            MinimapMarkerLayer::Player => self.player_markers.update(
                queue,
                handle.index,
                self.marker_instance(marker.position),
            ),
            MinimapMarkerLayer::Creature => self.creature_markers.update(
                queue,
                handle.index,
                self.marker_instance(marker.position),
            ),
        }
    }

    pub fn remove_marker(&self, queue: &wgpu::Queue, handle: MinimapMarkerHandle) {
        match handle.layer {
            MinimapMarkerLayer::Player => self.player_markers.remove(queue, handle.index),
            MinimapMarkerLayer::Creature => self.creature_markers.remove(queue, handle.index),
        }
    }

    pub fn render(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, camera_bind_group, &[]);

        if let Some(tile_batch) = &self.tile_batch {
            render_pass.set_bind_group(0, &tile_batch.bind_group, &[]);
            render_pass.set_vertex_buffer(0, tile_batch.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, tile_batch.instance_buffer.slice(..));
            render_pass.draw(
                0..tile_batch.vertices.len() as u32,
                0..tile_batch.instances.len() as u32,
            );
        }

        render_shared_batch(render_pass, &self.player_markers, &self.player_bind_group);
        render_shared_batch(
            render_pass,
            &self.creature_markers,
            &self.creature_bind_group,
        );
    }

    fn tile_instances(&self, tile: MinimapTile) -> Vec<Instance> {
        let tile_size = self.layout.tile_size;
        let Some(slice) = self.atlas_slices[tile.atlas_index as usize] else {
            return Vec::new();
        };

        vec![Instance {
            position: (tile.position - tile_size * 0.5).extend(0.9999),
            tex_min: slice.tex_min,
            tex_max: slice.tex_max,
            sprite_size: Vec2::new(
                tile_size.x / TILE_VERTEX_SIZE as f32,
                tile_size.y / TILE_VERTEX_SIZE as f32,
            ),
            ..Default::default()
        }]
    }

    fn marker_instance(&self, position: Vec2) -> Instance {
        let half_marker = self.layout.marker_size * 0.5;

        Instance {
            position: (position - half_marker).extend(1.0),
            tex_min: Vec2::ZERO,
            tex_max: Vec2::ONE,
            sprite_size: Vec2::new(
                self.layout.marker_size.x / MARKER_VERTEX_SIZE as f32,
                self.layout.marker_size.y / MARKER_VERTEX_SIZE as f32,
            ),
            ..Default::default()
        }
    }
}

pub fn minimap_tile_position(tile: Vec2, center: Vec2, layout: MinimapLayout) -> Vec2 {
    let delta = tile - center;
    Vec2::new(
        (delta.x - delta.y) * (layout.tile_size.x * 0.5),
        (delta.x + delta.y) * (layout.tile_size.y * 0.5),
    )
}

pub fn minimap_marker_position(tile: Vec2, center: Vec2, layout: MinimapLayout) -> Vec2 {
    minimap_tile_position(tile, center, layout) - Vec2::new(0.0, layout.tile_size.y * 0.5)
}

fn atlas_slice_regions(texture_size: UVec2) -> [Option<MinimapSlice>; 16] {
    let cell_size = Vec2::new(
        1.0 / TILE_GRID_DIMENSION as f32,
        1.0 / TILE_GRID_DIMENSION as f32,
    );

    let mut slices = [None; 16];

    for atlas_index in 0_u8..=15 {
        let col = atlas_index as u32 % TILE_GRID_DIMENSION;
        let row = atlas_index as u32 / TILE_GRID_DIMENSION;
        let tex_min = Vec2::new(col as f32 * cell_size.x, row as f32 * cell_size.y);
        let tex_max = tex_min + cell_size;
        slices[atlas_index as usize] = Some(MinimapSlice { tex_min, tex_max });
    }

    debug_assert!(texture_size.x > 0 && texture_size.y > 0);

    slices
}

fn dual_mask_cell(dual_mask: u8) -> (u32, u32) {
    debug_assert!(dual_mask < 16);

    match dual_mask {
        0 => (0, 3),
        1 => (3, 3),
        2 => (0, 2),
        3 => (1, 2),
        4 => (1, 3),
        5 => (0, 1),
        6 => (3, 0),
        7 => (2, 0),
        8 => (0, 0),
        9 => (3, 2),
        10 => (2, 3),
        11 => (3, 1),
        12 => (1, 0),
        13 => (2, 2),
        14 => (1, 1),
        15 => (2, 1),
        _ => unreachable!(),
    }
}

pub fn minimap_tile_atlas_index(dual_mask: u8) -> u8 {
    let (col, row) = dual_mask_cell(dual_mask);
    (row * TILE_GRID_DIMENSION + col) as u8
}

fn render_shared_batch(
    render_pass: &mut wgpu::RenderPass<'_>,
    batch: &SharedInstanceBatch,
    bind_group: &wgpu::BindGroup,
) {
    let instance_count = batch.len();
    if instance_count == 0 {
        return;
    }

    render_pass.set_bind_group(0, bind_group, &[]);
    render_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
    render_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
    render_pass.draw(0..batch.vertices.len() as u32, 0..instance_count as u32);
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::{
        DUAL_MASK_BOTTOM_LEFT, DUAL_MASK_BOTTOM_RIGHT, DUAL_MASK_TOP_LEFT, DUAL_MASK_TOP_RIGHT,
        MinimapLayout, atlas_slice_regions, dual_mask_cell, minimap_marker_position,
        minimap_tile_atlas_index, minimap_tile_position,
    };

    #[test]
    fn atlas_slice_regions_include_all_sixteen_cells() {
        let slices = atlas_slice_regions(glam::UVec2::new(128, 128));

        assert_eq!(slices.len(), 16);
        assert!(slices[0].is_some());
        assert_eq!(slices.iter().flatten().count(), 16);
    }

    #[test]
    fn dual_mask_cells_follow_godot_atlas_layout() {
        assert_eq!(dual_mask_cell(0), (0, 3));
        assert_eq!(dual_mask_cell(1), (3, 3));
        assert_eq!(dual_mask_cell(2), (0, 2));
        assert_eq!(dual_mask_cell(4), (1, 3));
        assert_eq!(dual_mask_cell(8), (0, 0));
        assert_eq!(dual_mask_cell(15), (2, 1));
    }

    #[test]
    fn dual_masks_map_to_expected_row_major_atlas_indices() {
        assert_eq!(minimap_tile_atlas_index(0), 12);
        assert_eq!(minimap_tile_atlas_index(1), 15);
        assert_eq!(minimap_tile_atlas_index(6), 3);
        assert_eq!(minimap_tile_atlas_index(15), 6);
    }

    #[test]
    fn atlas_slice_regions_use_row_major_cells() {
        let slice = atlas_slice_regions(glam::UVec2::new(128, 128))[6].unwrap();

        assert_eq!(slice.tex_min, Vec2::new(0.5, 0.25));
        assert_eq!(slice.tex_max, Vec2::new(0.75, 0.5));
    }

    #[test]
    fn lattice_positions_follow_isometric_half_step_layout() {
        let tile_center = Vec2::ZERO;
        let layout = MinimapLayout::default();

        assert_eq!(
            minimap_tile_position(Vec2::new(0.5, 0.0), tile_center, layout),
            Vec2::new(layout.tile_size.x * 0.25, layout.tile_size.y * 0.25),
        );
        assert_eq!(
            minimap_tile_position(Vec2::new(0.0, 0.5), tile_center, layout),
            Vec2::new(-(layout.tile_size.x * 0.25), layout.tile_size.y * 0.25),
        );
    }

    #[test]
    fn dual_mask_constants_cover_all_four_corner_bits() {
        assert_eq!(
            DUAL_MASK_TOP_LEFT
                | DUAL_MASK_TOP_RIGHT
                | DUAL_MASK_BOTTOM_RIGHT
                | DUAL_MASK_BOTTOM_LEFT,
            0b1111,
        );
    }

    #[test]
    fn minimap_positions_are_center_relative() {
        let layout = MinimapLayout::default();
        let center = Vec2::new(10.0, 10.0);

        assert_eq!(minimap_tile_position(center, center, layout), Vec2::ZERO);
        assert_eq!(
            minimap_marker_position(center, center, layout),
            Vec2::new(0.0, -(layout.tile_size.y * 0.5)),
        );
    }

    #[test]
    fn minimap_tiles_support_sub_tile_centering() {
        let layout = MinimapLayout::default();
        let center = Vec2::new(10.25, 10.75);

        assert_eq!(
            minimap_tile_position(Vec2::new(10.0, 11.0), center, layout),
            Vec2::new(-8.0, 0.0),
        );
    }

    #[test]
    fn minimap_positions_follow_isometric_axes() {
        let layout = MinimapLayout::default();
        let center = Vec2::new(10.0, 10.0);

        assert_eq!(
            minimap_tile_position(Vec2::new(11.0, 10.0), center, layout),
            Vec2::new(layout.tile_size.x * 0.5, layout.tile_size.y * 0.5),
        );
        assert_eq!(
            minimap_tile_position(Vec2::new(10.0, 11.0), center, layout),
            Vec2::new(-(layout.tile_size.x * 0.5), layout.tile_size.y * 0.5),
        );
    }
}
