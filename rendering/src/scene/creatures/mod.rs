pub mod types;
pub use types::*;

use formats::mpf::{MpfAnimation, MpfAnimationType, MpfFile};
use glam::{Vec2, Vec3};
use rustc_hash::FxHashMap;
use wgpu;

use crate::{
    instance::InstanceFlag,
    make_quad,
    scene::utils::{atlas_uv, calculate_tile_z, direction_to_orientation},
};
use crate::{
    scene::{
        Instance, Z_CREATURES, get_isometric_coordinate, sprite::SpriteBatch,
        texture_atlas::TextureAtlas, texture_bind::TextureBind,
    },
    texture,
};

use formats::game_files::SquashfsArchive;

type Archive = SquashfsArchive;

const ATLAS_WIDTH: usize = 2048;
const ATLAS_HEIGHT: usize = 4096;
const VERTEX_WIDTH: usize = 512;
const VERTEX_HEIGHT: usize = 512;

pub struct CreatureAssetStore {
    pub(crate) loaded_sprites: FxHashMap<u16, LoadedSprite>,
    pub(crate) atlas: TextureAtlas,
    pub(crate) bind_group: wgpu::BindGroup,
}

pub struct CreatureBatch {
    batch: SpriteBatch<u16>,
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

        let palette_data = archive.get_file_or_panic("hades/mns.ktx2");

        let palette_texture =
            texture::Texture::from_ktx2_rgba8(device, queue, "creature_palette", &palette_data)
                .unwrap();

        let bind_group = TextureBind::to_bind_group(
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
        Self {
            batch: SpriteBatch::new(device, vertices, store.bind_group.clone()),
        }
    }

    pub fn clear(&self) {
        self.batch.clear();
    }

    pub fn clear_and_unload(&self, store: &mut CreatureAssetStore) {
        self.batch
            .clear_and_unload(|sprite_id| store.unload_sprite(*sprite_id));
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

                let (mpf_file, _) = oxicode::decode_from_slice::<MpfFile>(&mpf_bytes)?;

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
            .batch
            .add_instance(queue, instance)
            .ok_or_else(|| anyhow::anyhow!("Failed to add creature instance"))?;

        let handle = CreateInstanceHandle {
            index: instance_index,
            sprite_id,
        };
        self.batch.insert_handle(handle.index, handle.sprite_id);

        Ok(AddCreatureResult {
            handle,
            animations: loaded_sprite.mpf_file.animations.clone(),
        })
    }

    pub fn remove_creature(
        &mut self,
        queue: &wgpu::Queue,
        store: &mut CreatureAssetStore,
        handle: CreateInstanceHandle,
    ) {
        self.batch.remove_instance(queue, handle.index);
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
                self.batch.update_instance(queue, handle.index, instance);
                return true;
            }
        }
        false
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.batch.draw(render_pass);
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

    let (tex_min, tex_max) = atlas_uv(
        Vec2::new(
            first_frame.rectangle.min.x as f32,
            first_frame.rectangle.min.y as f32,
        ),
        frame_w as f32,
        frame_h as f32,
        ATLAS_WIDTH as f32,
        ATLAS_HEIGHT as f32,
    );

    Ok(Instance::with_texture_atlas(
        (get_isometric_coordinate(position.x, position.y)
            - Vec2::new(offset_x, (frame_detail.center_y - frame_detail.top) as f32))
        .extend(calculate_tile_z(position.x, position.y, Z_CREATURES)),
        tex_min,
        tex_max,
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
