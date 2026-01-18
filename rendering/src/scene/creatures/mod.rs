pub mod types;
pub use types::*;

use bincode::config::Configuration;
use formats::{
    epf::AnimationDirection,
    mpf::{MpfAnimation, MpfAnimationType, MpfFile},
};
use glam::{Vec2, Vec3};
use rustc_hash::FxHashMap;
use wgpu;

use crate::{
    instance::{InstanceFlag, SharedInstanceBatch},
    make_quad,
};
use crate::{
    scene::{
        Instance, get_isometric_coordinate, texture_atlas::TextureAtlas, texture_bind::TextureBind,
    },
    texture,
};

use formats::game_files::ArxArchive;

type Archive = ArxArchive;

const ATLAS_WIDTH: usize = 2048;
const ATLAS_HEIGHT: usize = 2048;
const VERTEX_WIDTH: usize = 512;
const VERTEX_HEIGHT: usize = 512;

pub struct CreatureAssetStore {
    pub(crate) loaded_sprites: FxHashMap<u16, LoadedSprite>,
    pub(crate) atlas: TextureAtlas,
    pub(crate) bind_group: wgpu::BindGroup,
}

pub struct CreatureBatch {
    pub(crate) instances: SharedInstanceBatch,
}

impl CreatureAssetStore {
    pub async fn new(device: &wgpu::Device, queue: &wgpu::Queue, archive: &Archive) -> Self {
        let diffuse_texture = texture::Texture::from_data(
            device,
            queue,
            "creature_atlas",
            ATLAS_WIDTH as u32,
            ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::R8Unorm,
            &vec![0; ATLAS_WIDTH * ATLAS_HEIGHT],
        )
        .unwrap();

        let palette_data = archive.get_file_or_panic_async("hades/mns.ktx2").await;

        let palette_texture =
            texture::Texture::from_ktx2_rgba8(device, queue, "creature_palette", &palette_data)
                .unwrap();

        let tb = TextureBind::default();
        let bind_group = tb.to_bind_group(
            device,
            &diffuse_texture,
            &palette_texture,
            &texture::Texture::empty_view(device, "creature_empty"),
        );

        let atlas = TextureAtlas::new(diffuse_texture.texture);

        Self {
            loaded_sprites: FxHashMap::default(),
            atlas,
            bind_group,
        }
    }

    pub(crate) fn unload_sprite(&mut self, sprite_id: u16) {
        if let Some(sprite) = self.loaded_sprites.get_mut(&sprite_id) {
            sprite.ref_count -= 1;
            if sprite.ref_count == 0 {
                for allocation in &sprite.allocations {
                    self.atlas.atlas.deallocate(allocation.id);
                }
                self.loaded_sprites.remove(&sprite_id);
            }
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

impl CreatureBatch {
    pub fn new(device: &wgpu::Device, store: &CreatureAssetStore) -> Self {
        let vertices = make_quad(VERTEX_WIDTH as u32, VERTEX_HEIGHT as u32).to_vec();
        let creature_batch = SharedInstanceBatch::new(device, vertices, store.bind_group.clone());

        Self {
            instances: creature_batch,
        }
    }

    pub fn add_creature(
        &mut self,
        queue: &wgpu::Queue,
        store: &mut CreatureAssetStore,
        archive: &Archive,
        sprite_id: u16,
        direction: u8,
        x: f32,
        y: f32,
    ) -> anyhow::Result<AddCreatureResult> {
        let loaded_sprite = match store.loaded_sprites.entry(sprite_id) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let s = entry.into_mut();
                s.ref_count += 1;
                s
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let mpf_bytes = archive
                    .get_file(&format!("hades/mns{:03}.mpf.bin", sprite_id))
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to load MPF for sprite {}: {}", sprite_id, e)
                    })?;

                let (mpf_file, _) = bincode::decode_from_slice::<MpfFile, Configuration>(
                    &mpf_bytes,
                    bincode::config::standard(),
                )?;

                let mut allocations: Vec<etagere::Allocation> =
                    Vec::with_capacity(mpf_file.frames.len());
                for frame in &mpf_file.frames {
                    let w = frame.right - frame.left;
                    let h = frame.bottom - frame.top;
                    let alloc = store
                        .atlas
                        .allocate(queue, w as usize, h as usize, &frame.data)
                        .ok_or_else(|| anyhow::anyhow!("Atlas full for creature {}", sprite_id))?;
                    allocations.push(alloc);
                }
                entry.insert(LoadedSprite {
                    mpf_file,
                    allocations,
                    ref_count: 1,
                })
            }
        };

        let (anim_dir, flip) = direction_to_orientation(direction);

        let anim = loaded_sprite
            .mpf_file
            .animations
            .iter()
            .find(|a| a.animation_type == MpfAnimationType::Standing)
            .ok_or_else(|| {
                anyhow::anyhow!("No standing animation found for sprite {}", sprite_id)
            })?;

        let frame_index = anim.frame_index_for_direction(anim_dir);

        let instance =
            get_instance_for_frame(loaded_sprite, frame_index as usize, Vec2::new(x, y), flip)?;

        let instance_index = self
            .instances
            .add(queue, instance)
            .ok_or_else(|| anyhow::anyhow!("Failed to add creature instance"))?;

        Ok(AddCreatureResult {
            handle: CreateInstanceHandle {
                index: instance_index,
                sprite_id,
            },
            animations: loaded_sprite.mpf_file.animations.clone(),
        })
    }

    pub fn remove_creature(
        &mut self,
        queue: &wgpu::Queue,
        store: &mut CreatureAssetStore,
        handle: CreateInstanceHandle,
    ) {
        self.instances.remove(queue, handle.index);
        store.unload_sprite(handle.sprite_id);
    }

    pub fn update_creature(
        &self,
        queue: &wgpu::Queue,
        store: &CreatureAssetStore,
        handle: &CreateInstanceHandle,
        x: f32,
        y: f32,
        anim: &MpfAnimation,
        anim_frame: usize,
        direction: u8,
        tint: Vec3,
    ) -> bool {
        if let Some(loaded_sprite) = store.loaded_sprites.get(&handle.sprite_id) {
            let (anim_dir, flip) = direction_to_orientation(direction);

            let frame_index = anim.frame_index_for_direction(anim_dir) as usize + anim_frame;
            if let Ok(mut instance) =
                get_instance_for_frame(loaded_sprite, frame_index, Vec2::new(x, y), flip)
            {
                instance.tint = tint;
                self.instances.update(queue, handle.index, instance);
                return true;
            }
        }
        false
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass) {
        let batch = &self.instances;
        let instance_count = batch.len();
        if instance_count > 0 {
            render_pass.set_bind_group(0, &batch.bind_group, &[]);
            render_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
            render_pass.draw(0..batch.vertices.len() as u32, 0..instance_count as u32);
        }
    }
}

fn direction_to_orientation(dir: u8) -> (AnimationDirection, bool) {
    match dir {
        0 => (AnimationDirection::Away, false),    // Up
        1 => (AnimationDirection::Towards, false), // Right
        2 => (AnimationDirection::Towards, true),  // Down
        3 => (AnimationDirection::Away, true),     // Left
        _ => (AnimationDirection::Towards, false),
    }
}

fn get_instance_for_frame(
    loaded_sprite: &LoadedSprite,
    frame_index: usize,
    position: Vec2,
    flip: bool,
) -> anyhow::Result<Instance> {
    let first_frame = loaded_sprite.allocations.get(frame_index).ok_or_else(|| {
        anyhow::anyhow!(
            "Frame index out of bounds for sprite {} ({})",
            loaded_sprite.mpf_file.palette_number,
            frame_index
        )
    })?;

    let frame_detail = &loaded_sprite.mpf_file.frames[frame_index];

    let frame_w = frame_detail.right - frame_detail.left;
    let frame_h = frame_detail.bottom - frame_detail.top;

    let offset_x = if flip {
        (frame_detail.right - frame_detail.center_x) as f32
    } else {
        (frame_detail.center_x - frame_detail.left) as f32
    };

    Ok(Instance::with_texture_atlas(
        (get_isometric_coordinate(position.x, position.y)
            - Vec2::new(offset_x, (frame_detail.center_y - frame_detail.top) as f32))
        .extend((position.x + position.y) / 2048.0 - 0.000001),
        Vec2::new(
            first_frame.rectangle.min.x as f32 / ATLAS_WIDTH as f32,
            first_frame.rectangle.min.y as f32 / ATLAS_HEIGHT as f32,
        ),
        Vec2::new(
            (first_frame.rectangle.min.x + (frame_w as i32)) as f32 / ATLAS_WIDTH as f32,
            (first_frame.rectangle.min.y + (frame_h as i32)) as f32 / ATLAS_HEIGHT as f32,
        ),
        Vec2::new(
            frame_w as f32 / VERTEX_WIDTH as f32,
            frame_h as f32 / VERTEX_HEIGHT as f32,
        ),
        loaded_sprite.mpf_file.palette_number as f32 / 256.0,
        -1.,
        flip,
        false,
        InstanceFlag::None,
    ))
}
