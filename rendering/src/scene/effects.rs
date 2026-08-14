use std::collections::HashMap;

use bevy_math::Vec2;
use formats::game_files::SquashfsArchive;
use formats::sheets::EffectSheet;
use tracing::error;

use crate::instance::InstanceFlag;
use crate::scene::texture_atlas::{FrameRow, FrameUpload, TextureAtlas, merge_uploads};
use crate::scene::utils::calculate_tile_z;
use crate::scene::{TILE_HEIGHT, Z_EFFECTS, get_isometric_coordinate};
use crate::{Instance, InstanceRaw, SharedInstanceBatch, Vertex, make_quad, texture};

const ATLAS_WIDTH: usize = 2048;
const ATLAS_HEIGHT: usize = 2048;
const VERTEX_SIZE: usize = 512;
/// Manual per-effect vertical offsets (world px, added to the effect's y
/// placement), tuned from visual reports. The game data does not encode an
/// effect's intended height, so effects whose official placement differs from
/// the default formula are listed here.
const EFFECT_VERTICAL_OFFSETS: &[(u16, f32)] = &[
    // Tall EPF canvas: content sits ~1.5-2 tiles too low with the default.
    (89, -54.0),
];

pub struct EffectFrameSequence {
    pub frame_indices: Vec<usize>,
}

struct LoadedEffect {
    allocations: Vec<etagere::Allocation>,
    frame_rows: Vec<Option<(usize, u32, u32)>>,
    frame_widths: Vec<u16>,
    frame_heights: Vec<u16>,
    frame_offsets: Vec<(i16, i16)>,
    /// EFA frame anchors (`center_x`, `center_y`); `(0, 0)` for EPF effects.
    frame_anchors: Vec<(i16, i16)>,
    frame_interval_ms: usize,
    frame_sequence: Vec<usize>,
    /// Sheet dimensions for EPF-based positioning (0,0 for EFA which uses direct offsets)
    sheet_width: u16,
    sheet_height: u16,
    /// Extra vertical offset from `EFFECT_VERTICAL_OFFSETS`.
    vertical_offset: f32,
    /// Live `EffectHandle`s referencing this effect. Only zero-refcount
    /// effects are evicted when the atlas needs room.
    ref_count: usize,
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
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        archive: &SquashfsArchive,
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
            bind_group_layouts: &[Some(&bind_group_layout), Some(camera_bind_group_layout)],
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
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
            atlas: TextureAtlas::new(device, diffuse_texture.texture),
            pipeline,
        }
    }

    fn parse_effect_tbl(archive: &SquashfsArchive) -> Vec<EffectFrameSequence> {
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

    fn parse_palette_indices(archive: &SquashfsArchive) -> rangemap::RangeMap<u16, u16> {
        let Ok(data) = archive.get_file("roh/eff.tbl.bin") else {
            tracing::error!("Failed to load eff.tbl.bin");
            return rangemap::RangeMap::new();
        };

        match oxicode::serde::decode_from_slice::<rangemap::RangeMap<u16, u16>, _>(
            &data,
            oxicode::config::standard(),
        ) {
            Ok((map, _)) => map,
            Err(e) => {
                tracing::error!("Failed to decode eff.tbl.bin: {:?}", e);
                rangemap::RangeMap::new()
            }
        }
    }

    fn load_palette(archive: &SquashfsArchive) -> Option<Vec<u8>> {
        let data = archive.get_file("roh/eff.ktx2").ok()?;
        let reader = ktx2::Reader::new(&data).ok()?;
        let level = reader.levels().next()?;
        Some(level.data.to_vec())
    }

    pub fn spawn_effect(
        &mut self,
        queue: &wgpu::Queue,
        archive: &SquashfsArchive,
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
        let frame_count = loaded.frame_sequence.len();
        let frame_interval_ms = loaded.frame_interval_ms;
        let instance = self.create_instance(loaded, first_frame, x, y, z_offset)?;

        let instance_index = self.instances.add(instance)?;
        if let Some(loaded) = self.loaded_effects.get_mut(&effect_id) {
            loaded.ref_count += 1;
        }

        Some(EffectHandle {
            instance_index,
            effect_id,
            frame_count,
            frame_interval_ms,
        })
    }

    fn load_effect(
        &mut self,
        queue: &wgpu::Queue,
        archive: &SquashfsArchive,
        effect_id: u16,
    ) -> Option<()> {
        let sequence = self
            .frame_sequences
            .get((effect_id - 1) as usize)
            .map(|s| s.frame_indices.clone());

        let base = format!("roh/efct{:03}", effect_id);
        let sheet_bytes = archive.get_file(&format!("{base}.sheet.bin")).ok()?;
        let (meta, consumed) = oxicode::decode_from_slice::<EffectSheet>(&sheet_bytes).ok()?;
        let bytes_per_pixel = if meta.indexed { 1 } else { 4 };
        let chunk_slices = formats::sheets::chunk_pixel_slices(
            &sheet_bytes,
            consumed,
            &meta.chunks,
            bytes_per_pixel,
        )
        .ok()?;
        self.load_sheet(queue, effect_id, meta, &chunk_slices, sequence)
    }

    /// Frees every cached effect with no live instances, returning its atlas
    /// slots to etagere. Called when an allocation fails so effects from
    /// earlier casts can make room for new ones.
    fn evict_unused_effects(&mut self) {
        let to_evict: Vec<u16> = self
            .loaded_effects
            .iter()
            .filter(|(_, effect)| effect.ref_count == 0)
            .map(|(effect_id, _)| *effect_id)
            .collect();
        if to_evict.is_empty() {
            return;
        }
        let evicted = to_evict.len();
        for effect_id in &to_evict {
            if let Some(effect) = self.loaded_effects.remove(effect_id) {
                for allocation in &effect.allocations {
                    self.atlas.atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted,
            remaining = self.loaded_effects.len(),
            "Evicted unused effects to make room in the atlas"
        );
    }

    /// Loads a pre-packed effect sheet: allocates one atlas slot per chunk,
    /// uploads the sheet pixels (baking palette-indexed EPF frames to RGBA
    /// first), and caches the effect.
    fn load_sheet<'a>(
        &mut self,
        queue: &wgpu::Queue,
        effect_id: u16,
        meta: EffectSheet,
        chunk_slices: &[&'a [u8]],
        sequence: Option<Vec<usize>>,
    ) -> Option<()> {
        let mut allocations: Vec<etagere::Allocation> = Vec::with_capacity(meta.chunks.len());
        for chunk in &meta.chunks {
            let mut allocation = self
                .atlas
                .allocate_slot(chunk.width as usize, chunk.height as usize);
            if allocation.is_none() {
                // Atlas is full: evict unused effects and retry once.
                self.evict_unused_effects();
                allocation = self
                    .atlas
                    .allocate_slot(chunk.width as usize, chunk.height as usize);
            }
            let Some(allocation) = allocation else {
                error!(
                    "Effect atlas full - cannot allocate effect {} ({}x{})",
                    effect_id, chunk.width, chunk.height
                );
                for slot in &allocations {
                    self.atlas.atlas.deallocate(slot.id);
                }
                return None;
            };
            allocations.push(allocation);
        }

        // Bake palette-indexed chunks up front so their buffers outlive the
        // uploads that borrow them; RGBA chunks upload directly from the file
        // slices without a copy.
        let mut baked = Vec::with_capacity(meta.chunks.len());
        for (_, image) in chunk_slices.iter().enumerate() {
            if meta.indexed {
                let palette_index =
                    self.palette_indices.get(&effect_id).copied().unwrap_or(0) as u8;
                let Some(palette) = self.palette_data.as_ref() else {
                    for slot in &allocations {
                        self.atlas.atlas.deallocate(slot.id);
                    }
                    return None;
                };
                baked.push(self.apply_palette(image, palette, palette_index));
            } else {
                baked.push(Vec::new());
            }
        }

        // Upload one whole chunk image per slot. A sheet belongs to a single
        // effect, so one palette row applies to every pixel; baking the whole
        // chunk keeps all frames intact (per-frame uploads of the same slot
        // would overwrite each other) and matches the old uploader exactly.
        let mut uploads = Vec::with_capacity(meta.chunks.len());
        for (chunk_index, &image) in chunk_slices.iter().enumerate() {
            let chunk = meta.chunks[chunk_index];
            let data: &[u8] = if meta.indexed {
                &baked[chunk_index]
            } else {
                image
            };
            uploads.push(FrameUpload {
                rect: allocations[chunk_index].rectangle,
                width: chunk.width as usize,
                height: chunk.height as usize,
                frames: vec![FrameRow {
                    x: 0,
                    y: 0,
                    width: chunk.width as usize,
                    height: chunk.height as usize,
                    data,
                }],
            });
        }

        let uploads = merge_uploads(uploads);
        self.atlas.upload_batch(queue, &uploads);

        let mut frame_rows = vec![None; meta.frames.len()];
        let mut frame_widths = vec![0u16; meta.frames.len()];
        let mut frame_heights = vec![0u16; meta.frames.len()];
        let mut frame_offsets = vec![(0i16, 0i16); meta.frames.len()];
        let mut frame_anchors = vec![(0i16, 0i16); meta.frames.len()];
        for (frame_index, frame) in meta.frames.iter().enumerate() {
            if let Some(frame) = frame {
                frame_rows[frame_index] = Some((frame.chunk as usize, frame.x, frame.y));
                frame_widths[frame_index] = frame.width as u16;
                frame_heights[frame_index] = frame.height as u16;
                frame_offsets[frame_index] = (frame.left, frame.top);
                frame_anchors[frame_index] = (frame.center_x, frame.center_y);
            }
        }

        let frame_sequence = match sequence {
            Some(seq) if !(seq.len() == 1 && seq[0] == 0) => seq,
            // A lone `0` (or an empty line) in effect.tbl marks "no specific
            // sequence" - play every frame in the sheet, in order.
            _ => (0..meta.frames.len()).collect(),
        };

        let vertical_offset = EFFECT_VERTICAL_OFFSETS
            .iter()
            .find(|&&(id, _)| id == effect_id)
            .map(|&(_, offset)| offset)
            .unwrap_or(0.0);

        tracing::info!(
            effect_id,
            indexed = meta.indexed,
            frame_count = meta.frames.len(),
            played_frames = frame_sequence.len(),
            non_empty_frames = meta.frames.iter().filter(|f| f.is_some()).count(),
            chunk_count = meta.chunks.len(),
            sheet_width = meta.sheet_width,
            sheet_height = meta.sheet_height,
            center_x = frame_anchors
                .iter()
                .find(|&&(x, _)| x != 0)
                .map(|&(x, _)| x)
                .unwrap_or(0),
            center_y = frame_anchors
                .iter()
                .find(|&&(_, y)| y != 0)
                .map(|&(_, y)| y)
                .unwrap_or(0),
            "Loaded effect sheet"
        );

        self.loaded_effects.insert(
            effect_id,
            LoadedEffect {
                allocations,
                frame_rows,
                frame_widths,
                frame_heights,
                frame_offsets,
                frame_anchors,
                frame_interval_ms: meta.frame_interval_ms,
                frame_sequence,
                sheet_width: meta.sheet_width,
                sheet_height: meta.sheet_height,
                vertical_offset,
                ref_count: 0,
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
        let (center_x, center_y) = *loaded.frame_anchors.get(frame_index).unwrap_or(&(0, 0));

        let world_pos = get_isometric_coordinate(x, y);
        let z = calculate_tile_z(x, y, Z_EFFECTS) + z_offset;

        let (slot_index, frame_x, frame_y) = match loaded.frame_rows.get(frame_index) {
            Some(Some(row)) => *row,
            _ => {
                return Some(Instance {
                    position: world_pos.extend(z),
                    ..Default::default()
                });
            }
        };
        let Some(alloc) = loaded.allocations.get(slot_index) else {
            return Some(Instance {
                position: world_pos.extend(z),
                ..Default::default()
            });
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
            // EFA positioning: the frame's anchor point (center_x, center_y)
            // in the image lines up with the draw point, exactly like creature
            // sprites. `offset_x`/`offset_y` are the (trimmed) content's
            // origin within the image.
            Vec2::new(
                offset_x as f32 - center_x as f32,
                offset_y as f32 - center_y as f32,
            )
        };
        let effect_offset = effect_offset + Vec2::new(0.0, loaded.vertical_offset);

        Some(Instance {
            position: (world_pos + effect_offset).extend(z),
            tex_min: Vec2::new(
                (alloc.rectangle.min.x as f32 + frame_x as f32) / atlas_w,
                (alloc.rectangle.min.y as f32 + frame_y as f32) / atlas_h,
            ),
            tex_max: Vec2::new(
                (alloc.rectangle.min.x as f32 + frame_x as f32 + w) / atlas_w,
                (alloc.rectangle.min.y as f32 + frame_y as f32 + h) / atlas_h,
            ),
            sprite_size: Vec2::new(w / VERTEX_SIZE as f32, h / VERTEX_SIZE as f32),
            palette_offset: 0.0,
            dye_v_offset: -1.0,
            flags: InstanceFlag::None,
            tint: bevy_math::Vec3::ZERO,
        })
    }

    pub fn update_effect(
        &self,
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
            self.instances.update(handle.instance_index, instance);
            true
        } else {
            false
        }
    }

    pub fn remove_effect(&mut self, handle: &EffectHandle) {
        self.instances.remove(handle.instance_index);
        if let Some(loaded) = self.loaded_effects.get_mut(&handle.effect_id) {
            loaded.ref_count = loaded.ref_count.saturating_sub(1);
        }
    }

    pub fn instance_count(&self) -> usize {
        self.instances.live_len()
    }

    pub fn stats(&self) -> crate::instance::InstanceBatchStatsSnapshot {
        self.instances.stats()
    }

    pub fn flush_pending(&self, encoder: &mut wgpu::CommandEncoder) {
        self.instances.flush_pending(encoder);
    }

    pub fn finish_uploads(&self) {
        self.instances.finish_uploads();
    }

    pub fn recall_uploads(&self) {
        self.instances.recall_uploads();
    }

    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.instances.len() == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        self.instances.draw(render_pass);
    }
}
