use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::vertex::Vertex;
use bevy_math::{Vec2, Vec3};
use num_enum::IntoPrimitive;
use wgpu;
use wgpu::util::DeviceExt;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum InstanceFlag {
    #[default]
    None = 0,
    XRay = 1,
    Hover = 2,
    Translucent = 4,
    /// Local player's invisible self pops through occluders.
    Overlay = 8,
    /// Translucent + Overlay (4 | 8), as its own variant so the bit value is
    /// a valid discriminant.
    TranslucentOverlay = 12,
}

#[derive(Clone)]
pub struct Instance {
    pub position: Vec3,
    pub tex_min: Vec2,
    pub tex_max: Vec2,
    pub sprite_size: Vec2,
    pub palette_offset: f32,
    pub dye_v_offset: f32,
    pub flags: InstanceFlag,
    pub tint: Vec3,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            tex_min: Vec2::ZERO,
            tex_max: Vec2::ONE,
            sprite_size: Vec2::ZERO,
            palette_offset: -1.,
            dye_v_offset: -1.,
            flags: InstanceFlag::None,
            tint: Vec3::ZERO,
        }
    }
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            position: self.position.into(),
            tex_min: self.tex_min.into(),
            tex_max: self.tex_max.into(),
            sprite_size: self.sprite_size.into(),
            palette_offset: self.palette_offset,
            dye_v_offset: self.dye_v_offset,
            flags: self.flags.into(),
            tint: self.tint.into(),
        }
    }

    pub fn with_texture_region(
        position: Vec3,
        tex_min: Vec2,
        tex_max: Vec2,
        sprite_size: Vec2,
        palette_offset: f32,
    ) -> Self {
        Self {
            position,
            tex_min,
            tex_max,
            sprite_size,
            palette_offset,
            dye_v_offset: -1.,
            flags: InstanceFlag::None,
            tint: Vec3::ZERO,
        }
    }

    pub fn with_texture_atlas(
        position: Vec3,
        atlas_min: Vec2,
        atlas_max: Vec2,
        sprite_size: Vec2,
        palette_offset: f32,
        dye_v_offset: f32,
        flip_x: bool,
        flip_y: bool,
        flags: InstanceFlag,
    ) -> Self {
        let (tex_min, tex_max) = if flip_x || flip_y {
            let mut min = atlas_min;
            let mut max = atlas_max;

            if flip_x {
                std::mem::swap(&mut min.x, &mut max.x);
            }
            if flip_y {
                std::mem::swap(&mut min.y, &mut max.y);
            }

            (min, max)
        } else {
            (atlas_min, atlas_max)
        };

        Self {
            position,
            tex_min,
            tex_max,
            sprite_size,
            palette_offset,
            dye_v_offset,
            flags,
            tint: Vec3::ZERO,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    pub position: [f32; 3],
    pub tex_min: [f32; 2],
    pub tex_max: [f32; 2],
    pub sprite_size: [f32; 2],
    pub palette_offset: f32,
    pub dye_v_offset: f32,
    pub flags: u32,
    pub tint: [f32; 3],
}

impl Default for InstanceRaw {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            tex_min: [0.0; 2],
            tex_max: [1.0; 2],
            sprite_size: [0.0; 2],
            palette_offset: -1.0,
            dye_v_offset: -1.,
            flags: 0,
            tint: [0.0; 3],
        }
    }
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 7]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress
                        + mem::size_of::<u32>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct InstanceBatch {
    pub instances: Vec<Instance>,
    pub vertices: Vec<Vertex>,
    pub bind_group: wgpu::BindGroup,
    pub instance_buffer: wgpu::Buffer,
    pub vertex_buffer: wgpu::Buffer,
    pub buffer_capacity: usize,
}

const BATCH_SIZE: usize = 2048;

impl InstanceBatch {
    pub fn new(
        device: &wgpu::Device,
        instances: Vec<Instance>,
        vertices: Vec<Vertex>,
        bind_group: wgpu::BindGroup,
    ) -> Self {
        let buffer_capacity = instances.len().max(BATCH_SIZE);

        let mut buffer_data = Vec::with_capacity(buffer_capacity);

        for instance in &instances {
            buffer_data.push(instance.to_raw());
        }

        // Fill remaining slots with defaults
        buffer_data.resize(buffer_capacity, InstanceRaw::default());

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&buffer_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            instances,
            vertices,
            bind_group,
            instance_buffer: buffer,
            vertex_buffer,
            buffer_capacity,
        }
    }

    pub fn new_empty(
        device: &wgpu::Device,
        vertices: Vec<Vertex>,
        bind_group: wgpu::BindGroup,
    ) -> Self {
        Self::new(device, Vec::new(), vertices, bind_group)
    }

    /// Draw all instances with the batch's vertex buffer, instance buffer, and bind group.
    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.instances.is_empty() {
            return;
        }
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(
            0..self.vertices.len() as u32,
            0..self.instances.len() as u32,
        );
    }

    pub fn update_instance(&mut self, queue: &wgpu::Queue, index: usize, instance: Instance) {
        if index < self.instances.len() {
            let raw_instance = instance.to_raw();
            self.instances[index] = instance;
            queue.write_buffer(
                &self.instance_buffer,
                (index * std::mem::size_of::<InstanceRaw>()) as u64,
                bytemuck::cast_slice(&[raw_instance]),
            );
        }
    }

    /// Replaces every instance with a single upload, reusing the existing GPU
    /// buffer. The caller must ensure the new instance count fits within
    /// `buffer_capacity`; otherwise the batch should be recreated instead.
    pub fn replace_all(&mut self, queue: &wgpu::Queue, instances: Vec<Instance>) {
        debug_assert!(instances.len() <= self.buffer_capacity);

        let raw_instances = instances
            .iter()
            .map(Instance::to_raw)
            .collect::<Vec<InstanceRaw>>();
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&raw_instances),
        );

        self.instances = instances;
    }

    pub fn add_instance(&mut self, queue: &wgpu::Queue, instance: Instance) -> Option<usize> {
        let index = self.instances.len();

        if index >= self.buffer_capacity {
            return None;
        }

        let raw_instance = instance.to_raw();
        queue.write_buffer(
            &self.instance_buffer,
            (index * std::mem::size_of::<InstanceRaw>()) as u64,
            bytemuck::cast_slice(&[raw_instance]),
        );
        self.instances.push(instance);

        Some(index)
    }

    pub fn remove_instance(&mut self, queue: &wgpu::Queue, index: usize) {
        if index >= self.instances.len() {
            return;
        }

        let removed_index = self.instances.len() - 1;

        self.instances.swap_remove(index);

        queue.write_buffer(
            &self.instance_buffer,
            (index * std::mem::size_of::<InstanceRaw>()) as u64,
            bytemuck::cast_slice(&[self.instances[index].to_raw()]),
        );

        if index != removed_index {
            queue.write_buffer(
                &self.instance_buffer,
                (removed_index * std::mem::size_of::<InstanceRaw>()) as u64,
                bytemuck::cast_slice(&[InstanceRaw::default()]),
            );
        }
    }

    pub fn get_instance(&self, index: usize) -> Option<&Instance> {
        if index < self.instances.len() {
            Some(&self.instances[index])
        } else {
            None
        }
    }
}

pub struct SharedInstanceBatch {
    pub vertices: Vec<Vertex>,
    pub bind_group: wgpu::BindGroup,
    pub instance_buffer: wgpu::Buffer,
    pub vertex_buffer: wgpu::Buffer,
    next_index: AtomicUsize,
    free_indices: Arc<Mutex<Vec<usize>>>,
    written_instances: Arc<Mutex<Vec<InstanceRaw>>>,
    translucent_count: AtomicUsize,
}

impl SharedInstanceBatch {
    pub fn new(device: &wgpu::Device, vertices: Vec<Vertex>, bind_group: wgpu::BindGroup) -> Self {
        let mut buffer_data = Vec::with_capacity(BATCH_SIZE);

        buffer_data.resize(BATCH_SIZE, InstanceRaw::default());

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&buffer_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            vertices,
            bind_group,
            instance_buffer: buffer,
            vertex_buffer,
            next_index: AtomicUsize::new(0),
            free_indices: Arc::new(Mutex::new(Vec::with_capacity(BATCH_SIZE))),
            written_instances: Arc::new(Mutex::new(vec![InstanceRaw::default(); BATCH_SIZE])),
            translucent_count: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.next_index.load(Ordering::Relaxed)
    }

    /// Live instances the translucent/x-ray composite pass must draw.
    pub fn translucent_count(&self) -> usize {
        self.translucent_count.load(Ordering::Relaxed)
    }

    /// Draw all live instances with the batch's vertex buffer, instance buffer, and bind group.
    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        let instance_count = self.len();
        if instance_count == 0 {
            return;
        }
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..self.vertices.len() as u32, 0..instance_count as u32);
    }

    pub fn clear(&self) {
        self.next_index.store(0, Ordering::Relaxed);
        if let Ok(mut free_indices) = self.free_indices.lock() {
            free_indices.clear();
        }
        if let Ok(mut written_instances) = self.written_instances.lock() {
            written_instances.fill(InstanceRaw::default());
        }
        self.translucent_count.store(0, Ordering::Relaxed);
    }

    pub fn update(&self, queue: &wgpu::Queue, index: usize, instance: Instance) {
        let raw_instance = instance.to_raw();

        if let Ok(mut written_instances) = self.written_instances.lock() {
            let old_translucent = needs_translucent_pass(
                written_instances
                    .get(index)
                    .map_or(0, |cached| cached.flags),
            );
            let new_translucent = needs_translucent_pass(raw_instance.flags);
            if old_translucent != new_translucent {
                if new_translucent {
                    self.translucent_count.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.translucent_count.fetch_sub(1, Ordering::Relaxed);
                }
            }

            if written_instances.get(index) == Some(&raw_instance) {
                return;
            }

            if let Some(cached_instance) = written_instances.get_mut(index) {
                *cached_instance = raw_instance;
            }
        }

        queue.write_buffer(
            &self.instance_buffer,
            (index * std::mem::size_of::<InstanceRaw>()) as u64,
            bytemuck::cast_slice(&[raw_instance]),
        );
    }

    fn get_next_index(&self) -> Option<usize> {
        if let Ok(mut free_indices) = self.free_indices.lock() {
            if let Some(index) = free_indices.pop() {
                return Some(index);
            }
        }

        let index = self.next_index.fetch_add(1, Ordering::Relaxed);

        if index < BATCH_SIZE {
            Some(index)
        } else {
            None
        }
    }

    pub fn add(&self, queue: &wgpu::Queue, instance: Instance) -> Option<usize> {
        let index = self.get_next_index()?;

        self.update(queue, index, instance);

        Some(index)
    }

    pub fn remove(&self, queue: &wgpu::Queue, index: usize) {
        if let Ok(mut free_indices) = self.free_indices.lock() {
            free_indices.push(index);
        }

        self.update(queue, index, Instance::default());
    }
}

fn needs_translucent_pass(flags: u32) -> bool {
    flags & (u32::from(InstanceFlag::XRay) | u32::from(InstanceFlag::Translucent)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance.enumerate_adapters(wgpu::Backends::all()).await.into_iter().next()?;
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("Instance Batch Test Device"),
                    required_features: wgpu::Features::IMMEDIATES,
                    required_limits: wgpu::Limits {
                        max_immediate_size: 16,
                        ..Default::default()
                    },
                    memory_hints: Default::default(),
                    ..Default::default()
                })
                .await
                .ok()
        })
    }

    fn empty_bind_group(device: &wgpu::Device) -> wgpu::BindGroup {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Empty Test Layout"),
            entries: &[],
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Empty Test Bind Group"),
            layout: &layout,
            entries: &[],
        })
    }

    #[test]
    fn translucent_flag_matches_shader_discard() {
        assert!(!needs_translucent_pass(0));
        assert!(!needs_translucent_pass(2)); // Hover
        assert!(!needs_translucent_pass(8)); // Overlay alone
        assert!(needs_translucent_pass(1)); // XRay
        assert!(needs_translucent_pass(4)); // Translucent
        assert!(needs_translucent_pass(12)); // TranslucentOverlay
    }

    #[test]
    fn translucent_count_tracks_flag_changes() {
        let Some((device, queue)) = test_device() else {
            eprintln!("No GPU adapter available; skipping translucent count test");
            return;
        };

        let batch = SharedInstanceBatch::new(&device, Vec::new(), empty_bind_group(&device));
        assert_eq!(batch.translucent_count(), 0);

        let index = batch
            .add(&queue, Instance::with_texture_region(Vec3::ZERO, Vec2::ZERO, Vec2::ONE, Vec2::ZERO, 0.0))
            .unwrap();
        assert_eq!(batch.translucent_count(), 0);

        batch.update(
            &queue,
            index,
            Instance {
                flags: InstanceFlag::XRay,
                ..Default::default()
            },
        );
        assert_eq!(batch.translucent_count(), 1);

        batch.update(
            &queue,
            index,
            Instance {
                flags: InstanceFlag::Translucent,
                ..Default::default()
            },
        );
        assert_eq!(batch.translucent_count(), 1);

        batch.update(
            &queue,
            index,
            Instance {
                flags: InstanceFlag::None,
                ..Default::default()
            },
        );
        assert_eq!(batch.translucent_count(), 0);

        batch.remove(&queue, index);
        assert_eq!(batch.translucent_count(), 0);
    }
}
