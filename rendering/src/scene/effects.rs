use std::collections::HashMap;

use bincode::config::Configuration;
use etagere::Allocation;
use formats::efa::EfaFile;
use formats::epf::EpfImage;
use formats::game_files::ArxArchive;
use glam::Vec2;

use crate::instance::InstanceFlag;
use crate::scene::texture_atlas::TextureAtlas;
use crate::scene::utils::calculate_tile_z;
use crate::scene::{TILE_HEIGHT, get_isometric_coordinate};
use crate::{Instance, InstanceRaw, SharedInstanceBatch, Vertex, make_quad, texture};

const ATLAS_WIDTH: usize = 2048;
const ATLAS_HEIGHT: usize = 2048;
const VERTEX_SIZE: usize = 512;

pub struct EffectFrameSequence {
    pub frame_indices: Vec<usize>,
}

struct LoadedEffect {
    allocations: Vec<Option<Allocation>>,
    frame_widths: Vec<u16>,
    frame_heights: Vec<u16>,
    frame_offsets: Vec<(i16, i16)>,
    frame_interval_ms: usize,
    frame_sequence: Vec<usize>,
    /// Sheet dimensions for EPF-based positioning (0,0 for EFA which uses direct offsets)
    sheet_width: u16,
    sheet_height: u16,
}

#[derive(Clone)]
pub struct EffectHandle {
    pub instance_index: usize,
    pub effect_id: u16,
    pub frame_count: usize,
    pub frame_interval_ms: usize,
}

pub struct EffectManager {
    loaded_effects: HashMap<u16, LoadedEffect>,
    frame_sequences: Vec<EffectFrameSequence>,
    palette_data: Option<Vec<u8>>,
    palette_indices: rangemap::RangeMap<u16, u16>,
    instances: SharedInstanceBatch,
    atlas: TextureAtlas,
    pipeline: wgpu::RenderPipeline,
}

impl EffectManager {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        archive: &ArxArchive,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let diffuse_texture = texture::Texture::from_data(
            device,
            queue,
            "effect_atlas",
            ATLAS_WIDTH as u32,
            ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::Rgba8Unorm,
            &vec![0; ATLAS_WIDTH * ATLAS_HEIGHT * 4],
        )
        .unwrap();

        let frame_sequences = Self::parse_effect_tbl(archive);
        let palette_indices = Self::parse_palette_indices(archive);
        let palette_data = Self::load_palette(archive);

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
            ],
            label: Some("effect_bind_group_layout"),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("effect_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Effect Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/effect.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Effect Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, camera_bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Effect Pipeline"),
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
        });

        let vertices = make_quad(VERTEX_SIZE as u32, VERTEX_SIZE as u32).to_vec();
        let instances = SharedInstanceBatch::new(device, vertices, bind_group);

        Self {
            loaded_effects: HashMap::new(),
            frame_sequences,
            palette_data,
            palette_indices,
            instances,
            atlas: TextureAtlas::new(diffuse_texture.texture),
            pipeline,
        }
    }

    fn parse_effect_tbl(archive: &ArxArchive) -> Vec<EffectFrameSequence> {
        let Ok(data) = archive.get_file("roh/effect.tbl") else {
            tracing::error!("Failed to load effect.tbl");
            return Vec::new();
        };

        let text = String::from_utf8_lossy(&data);

        let mut lines = text.lines();

        // Skip first line (count)
        lines.next();

        lines
            .map(|line| {
                let frame_indices = line
                    .split_whitespace()
                    .filter_map(|e| str::parse::<i32>(e).ok())
                    .map(|v| v as usize)
                    .collect();

                EffectFrameSequence { frame_indices }
            })
            .collect()
    }

    fn parse_palette_indices(archive: &ArxArchive) -> rangemap::RangeMap<u16, u16> {
        let Ok(data) = archive.get_file("roh/eff.tbl.bin") else {
            tracing::error!("Failed to load eff.tbl.bin");
            return rangemap::RangeMap::new();
        };

        match bincode::serde::decode_from_slice::<rangemap::RangeMap<u16, u16>, _>(
            &data,
            bincode::config::standard(),
        ) {
            Ok((map, _)) => map,
            Err(e) => {
                tracing::error!("Failed to decode eff.tbl.bin: {:?}", e);
                rangemap::RangeMap::new()
            }
        }
    }

    fn load_palette(archive: &ArxArchive) -> Option<Vec<u8>> {
        let data = archive.get_file("roh/eff.ktx2").ok()?;
        let reader = ktx2::Reader::new(&data).ok()?;
        let level = reader.levels().next()?;
        Some(level.data.to_vec())
    }

    pub fn spawn_effect(
        &mut self,
        queue: &wgpu::Queue,
        archive: &ArxArchive,
        effect_id: u16,
        x: f32,
        y: f32,
        z_offset: f32,
    ) -> Option<EffectHandle> {
        if !self.loaded_effects.contains_key(&effect_id) {
            self.load_effect(queue, archive, effect_id)?;
        }

        let loaded = self.loaded_effects.get(&effect_id)?;

        let first_frame = *loaded.frame_sequence.first()?;
        let instance = self.create_instance(loaded, first_frame, x, y, z_offset)?;

        let instance_index = self.instances.add(queue, instance)?;

        Some(EffectHandle {
            instance_index,
            effect_id,
            frame_count: loaded.frame_sequence.len(),
            frame_interval_ms: loaded.frame_interval_ms,
        })
    }

    fn load_effect(
        &mut self,
        queue: &wgpu::Queue,
        archive: &ArxArchive,
        effect_id: u16,
    ) -> Option<()> {
        let sequence = self
            .frame_sequences
            .get((effect_id - 1) as usize)
            .map(|s| s.frame_indices.clone());

        let efa_path = format!("roh/efct{:03}.efa.bin", effect_id);

        if let Ok(data) = archive.get_file(&efa_path) {
            self.load_efa(queue, effect_id, &data, sequence)
        } else {
            self.load_epf(queue, archive, effect_id, sequence)
        }
    }

    fn load_efa(
        &mut self,
        queue: &wgpu::Queue,
        effect_id: u16,
        data: &[u8],
        sequence: Option<Vec<usize>>,
    ) -> Option<()> {
        let (efa, _) = bincode::decode_from_slice::<EfaFile, Configuration>(
            &data,
            bincode::config::standard(),
        )
        .ok()?;

        let mut allocations = Vec::with_capacity(efa.frames.len());
        let mut frame_widths = Vec::with_capacity(efa.frames.len());
        let mut frame_heights = Vec::with_capacity(efa.frames.len());
        let mut frame_offsets = Vec::with_capacity(efa.frames.len());

        for frame in &efa.frames {
            let w = frame.width as usize;
            let h = frame.height as usize;

            if w > 0 && h > 0 {
                let alloc = self.atlas.allocate(queue, w, h, &frame.data);
                allocations.push(alloc);
            } else {
                allocations.push(None);
            }
            frame_widths.push(frame.width);
            frame_heights.push(frame.height);
            frame_offsets.push((frame.left, frame.top));
        }

        let frame_sequence = match sequence {
            Some(seq) if !(seq.len() == 1 && seq[0] == 0) => seq,
            _ => (0..allocations.len()).collect(),
        };

        self.loaded_effects.insert(
            effect_id,
            LoadedEffect {
                allocations,
                frame_widths,
                frame_heights,
                frame_offsets,
                frame_interval_ms: efa.frame_interval_ms,
                frame_sequence,
                sheet_width: 0,
                sheet_height: 0,
            },
        );

        Some(())
    }

    fn load_epf(
        &mut self,
        queue: &wgpu::Queue,
        archive: &ArxArchive,
        effect_id: u16,
        sequence: Option<Vec<usize>>,
    ) -> Option<()> {
        let epf_path = format!("roh/efct{:03}.epf.bin", effect_id);
        let data = archive.get_file(&epf_path).ok()?;

        let (epf, _) =
            bincode::decode_from_slice::<EpfImage, _>(&data, bincode::config::standard()).ok()?;

        let palette_index = self.palette_indices.get(&effect_id).copied().unwrap_or(0) as u8;
        let palette = self.palette_data.as_ref()?;

        let mut allocations = Vec::with_capacity(epf.frames.len());
        let mut frame_widths = Vec::with_capacity(epf.frames.len());
        let mut frame_heights = Vec::with_capacity(epf.frames.len());
        let mut frame_offsets = Vec::with_capacity(epf.frames.len());

        for frame in &epf.frames {
            let w = frame.right.saturating_sub(frame.left);
            let h = frame.bottom.saturating_sub(frame.top);

            if w > 0 && h > 0 {
                let rgba_data = self.apply_palette(&frame.data, palette, palette_index);

                let alloc = self.atlas.allocate(queue, w, h, &rgba_data);
                allocations.push(alloc);
                frame_widths.push(w as u16);
                frame_heights.push(h as u16);
                frame_offsets.push((frame.left as i16, frame.top as i16));
            } else {
                allocations.push(None);
                frame_widths.push(0);
                frame_heights.push(0);
                frame_offsets.push((0, 0));
            }
        }

        let frame_sequence = match sequence {
            Some(seq) if !(seq.len() == 1 && seq[0] == 0) => seq,
            _ => (0..allocations.len()).collect(),
        };

        self.loaded_effects.insert(
            effect_id,
            LoadedEffect {
                allocations,
                frame_widths,
                frame_heights,
                frame_offsets,
                frame_interval_ms: 100,
                frame_sequence,
                sheet_width: epf.width as u16,
                sheet_height: epf.height as u16,
            },
        );

        Some(())
    }

    fn apply_palette(&self, indexed_data: &[u8], palette: &[u8], palette_row: u8) -> Vec<u8> {
        let row_offset = (palette_row as usize) * 256 * 4;
        let mut rgba = Vec::with_capacity(indexed_data.len() * 4);
        for &idx in indexed_data {
            if idx == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let offset = row_offset + (idx as usize) * 4;
                if offset + 3 < palette.len() {
                    rgba.extend_from_slice(&palette[offset..offset + 4]);
                } else {
                    rgba.extend_from_slice(&[255, 0, 255, 255]);
                }
            }
        }
        rgba
    }

    fn create_instance(
        &self,
        loaded: &LoadedEffect,
        frame_index: usize,
        x: f32,
        y: f32,
        z_offset: f32,
    ) -> Option<Instance> {
        let w = *loaded.frame_widths.get(frame_index)? as f32;
        let h = *loaded.frame_heights.get(frame_index)? as f32;
        let (offset_x, offset_y) = *loaded.frame_offsets.get(frame_index)?;

        let world_pos = get_isometric_coordinate(x, y);
        let z = calculate_tile_z(x, y, 1.0) + z_offset;

        let alloc = match loaded.allocations.get(frame_index).and_then(|a| a.as_ref()) {
            Some(alloc) => alloc,
            None => {
                return Some(Instance {
                    position: world_pos.extend(z),
                    ..Default::default()
                });
            }
        };

        let atlas_w = ATLAS_WIDTH as f32;
        let atlas_h = ATLAS_HEIGHT as f32;

        // EPF-based effects use sheet dimensions for centering (like items)
        // EFA-based effects (sheet_width == 0) use direct frame offsets
        // Both are shifted up by TILE_HEIGHT to position above the tile
        let effect_offset = if loaded.sheet_width > 0 {
            // EPF positioning: center on sheet, offset by frame position
            let sheet_w = loaded.sheet_width as f32;
            let sheet_h = loaded.sheet_height as f32;
            Vec2::new(
                -(sheet_w / 2.0).floor() + offset_x as f32,
                -(sheet_h / 2.0).floor() + offset_y as f32 - TILE_HEIGHT as f32,
            )
        } else {
            // EFA positioning: use frame offsets directly
            Vec2::new(
                -(offset_x as f32 + (w / 2.)),
                -(offset_y as f32 + (h / 2.) + TILE_HEIGHT as f32) - TILE_HEIGHT as f32,
            )
        };

        Some(Instance {
            position: (world_pos + effect_offset).extend(z),
            tex_min: Vec2::new(
                alloc.rectangle.min.x as f32 / atlas_w,
                alloc.rectangle.min.y as f32 / atlas_h,
            ),
            tex_max: Vec2::new(
                (alloc.rectangle.min.x as f32 + w) / atlas_w,
                (alloc.rectangle.min.y as f32 + h) / atlas_h,
            ),
            sprite_size: Vec2::new(w / VERTEX_SIZE as f32, h / VERTEX_SIZE as f32),
            palette_offset: 0.0,
            dye_v_offset: -1.0,
            flags: InstanceFlag::None,
            tint: glam::Vec3::ZERO,
        })
    }

    pub fn update_effect(
        &self,
        queue: &wgpu::Queue,
        handle: &EffectHandle,
        x: f32,
        y: f32,
        z_offset: f32,
        frame_in_sequence: usize,
    ) -> bool {
        let Some(loaded) = self.loaded_effects.get(&handle.effect_id) else {
            return false;
        };

        let frame_index = loaded
            .frame_sequence
            .get(frame_in_sequence % loaded.frame_sequence.len())
            .copied()
            .unwrap_or(0);

        if let Some(instance) = self.create_instance(loaded, frame_index, x, y, z_offset) {
            self.instances
                .update(queue, handle.instance_index, instance);
            true
        } else {
            false
        }
    }

    pub fn remove_effect(&mut self, queue: &wgpu::Queue, handle: &EffectHandle) {
        self.instances.remove(queue, handle.instance_index);
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        let instance_count = self.instances.len();
        if instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.instances.bind_group, &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instances.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instances.instance_buffer.slice(..));
        render_pass.draw(
            0..self.instances.vertices.len() as u32,
            0..instance_count as u32,
        );
    }
}
