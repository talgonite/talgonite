mod palettes;
pub mod types;

pub use palettes::*;
pub use types::*;

use etagere::Allocation;
use formats::epf::{AnimationDirection, EpfAnimation, EpfAnimationType};
use formats::util::parallel_indexed;
use glam::{Vec2, Vec3};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::error;
use wgpu;

use crate::instance::InstanceFlag;
use crate::make_quad;
use crate::scene::utils::{atlas_uv, calculate_tile_z, direction_to_orientation};
use crate::{
    scene::{
        Instance, TILE_WIDTH_HALF, Z_PLAYERS_BASE, get_isometric_coordinate, sprite::SpriteBatch,
        texture_atlas::TextureAtlas, texture_bind::TextureBind,
    },
    texture,
};
use formats::game_files::SquashfsArchive;

type Archive = SquashfsArchive;

const ATLAS_WIDTH: usize = 4096;
const ATLAS_HEIGHT: usize = 8192;
const VERTEX_WIDTH: usize = 512;
const VERTEX_HEIGHT: usize = 512;
const PLAYER_Y_OFFSET: f32 = -70.0;

pub struct PlayerAssetStore {
    loaded_sprites: FxHashMap<PlayerSpriteKey, LoadedSprite>,
    missing_sprites: FxHashSet<PlayerSpriteKey>,
    atlas: TextureAtlas,
    palettes: PlayerPalettes,
    bind_group: wgpu::BindGroup,
}

/// Max players per tile for z-ordering (wraps after this)
const PLAYERS_PER_TILE: u8 = 3;
/// Z range within a tile allocated for player stacking.
/// Must be much smaller than z_priority layer differences (~0.01) to avoid
/// equipment parts from different players interleaving.
const PLAYER_STACK_Z_RANGE: f32 = 0.003;

pub struct PlayerBatch {
    batch: SpriteBatch<PlayerSpriteKey>,
}

struct DecodedPlayerSprite {
    epf_image: Vec<EpfAnimation>,
    animations: FxHashMap<(EpfAnimationType, AnimationDirection), AnimationData>,
}

impl PlayerAssetStore {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, archive: &Archive) -> Self {
        let diffuse_texture = texture::Texture::from_data(
            device,
            queue,
            "player_atlas",
            ATLAS_WIDTH as u32,
            ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::R8Unorm,
            &vec![0; ATLAS_WIDTH * ATLAS_HEIGHT],
        )
        .unwrap();

        let (palettes, palette_texture, dye_texture) = PlayerPalettes::new(device, queue, archive);

        let bind_group = TextureBind::to_bind_group(
            device,
            &diffuse_texture,
            &palette_texture,
            &dye_texture.view,
        );

        let atlas = TextureAtlas::new(diffuse_texture.texture);

        Self {
            loaded_sprites: FxHashMap::default(),
            missing_sprites: FxHashSet::default(),
            atlas,
            palettes,
            bind_group,
        }
    }

    fn get_instance_for_frame(
        palettes: &PlayerPalettes,
        loaded_sprite: &LoadedSprite,
        sprite: &PlayerSpriteKey,
        animation_type: EpfAnimationType,
        frame_index: usize,
        position: Vec2,
        is_towards: bool,
        flip: bool,
        dye_color: u8,
        flags: InstanceFlag,
        tint: Vec3,
        stack_order: u8,
    ) -> anyhow::Result<Instance> {
        let (palette_v, palette_dye) = palettes.get_palette_params(sprite, dye_color);
        let direction = if is_towards {
            AnimationDirection::Towards
        } else {
            AnimationDirection::Away
        };

        let anim_data = loaded_sprite
            .animations
            .get(&(animation_type, direction))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Animation {:?} for direction {:?} not found",
                    animation_type,
                    direction
                )
            })?;

        let frame_detail = loaded_sprite.epf_image[anim_data.epf_index]
            .image
            .frames
            .get(frame_index)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Frame index {} out of bounds for animation {:?} (count: {})",
                    frame_index,
                    animation_type,
                    anim_data.frame_count
                )
            })?;

        let frame_w = (frame_detail.right - frame_detail.left) as f32;
        let frame_h = (frame_detail.bottom - frame_detail.top) as f32;

        if frame_w == 0.0 || frame_h == 0.0 {
            return Ok(Instance::default());
        }

        let allocation = loaded_sprite.allocations[anim_data.start_frame_index + frame_index]
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No allocation for sprite: {:?} at frame {}",
                    sprite,
                    frame_index
                )
            })?;

        let mut frame_offset = Vec2::new(frame_detail.left as f32, frame_detail.top as f32);
        let mut piece_offset = sprite.slot.offset();
        let mut iso_coord_offset = Vec2::ZERO;

        if flip {
            frame_offset.x = -(frame_offset.x + frame_w);
            piece_offset.x = -piece_offset.x;
            iso_coord_offset = Vec2::new(1., -1.);
        }

        let z = calculate_tile_z(
            position.x,
            position.y,
            // Player Z range is 0.1 to 0.2, with stack_order adding a small offset
            // to separate multiple players on the same tile
            Z_PLAYERS_BASE
                + (sprite.slot.z_priority(is_towards) * 0.1)
                + (stack_order as f32 / PLAYERS_PER_TILE as f32) * PLAYER_STACK_Z_RANGE,
        );

        let (tex_min, tex_max) = atlas_uv(
            Vec2::new(
                allocation.rectangle.min.x as f32,
                allocation.rectangle.min.y as f32,
            ),
            frame_w,
            frame_h,
            ATLAS_WIDTH as f32,
            ATLAS_HEIGHT as f32,
        );

        let mut instance = Instance::with_texture_atlas(
            (get_isometric_coordinate(
                position.x + iso_coord_offset.x,
                position.y + iso_coord_offset.y,
            ) + frame_offset
                + piece_offset
                + Vec2::new(-(TILE_WIDTH_HALF as f32), PLAYER_Y_OFFSET))
            .extend(z),
            tex_min,
            tex_max,
            Vec2::new(
                frame_w / VERTEX_WIDTH as f32,
                frame_h / VERTEX_HEIGHT as f32,
            ),
            palette_v,
            palette_dye,
            flip,
            false,
            flags,
        );
        instance.tint = tint;
        Ok(instance)
    }

    fn unload_sprite(&mut self, key: PlayerSpriteKey) {
        if let Some(sprite) = self.loaded_sprites.get_mut(&key) {
            sprite.ref_count -= 1;
            if sprite.ref_count == 0 {
                for allocation in &sprite.allocations {
                    if let Some(allocation) = allocation {
                        self.atlas.atlas.deallocate(allocation.id);
                    }
                }
                self.loaded_sprites.remove(&key);
            }
        }
    }

    pub fn preload_player_sprites(
        &mut self,
        queue: &wgpu::Queue,
        archive: &Archive,
        sprites: &[PlayerSpriteKey],
    ) -> anyhow::Result<()> {
        let mut queued = FxHashSet::default();
        let mut sprites_to_load = Vec::new();

        for sprite in sprites.iter().copied() {
            if self.loaded_sprites.contains_key(&sprite)
                || self.missing_sprites.contains(&sprite)
                || !queued.insert(sprite)
            {
                continue;
            }

            sprites_to_load.push((sprite, Self::player_sprite_path(&sprite)));
        }

        if sprites_to_load.is_empty() {
            return Ok(());
        }

        let paths: Vec<String> = sprites_to_load
            .iter()
            .map(|(_, path)| path.clone())
            .collect();
        let file_results = archive.get_files_parallel(&paths);

        let mut bytes_by_sprite = Vec::with_capacity(file_results.len());

        for ((key, _path), file_result) in sprites_to_load.into_iter().zip(file_results) {
            match file_result {
                Ok(bytes) => bytes_by_sprite.push((key, bytes)),
                Err(formats::game_files::SquashfsError::FileNotFound(_)) => {
                    self.missing_sprites.insert(key);
                }
                Err(error) => return Err(error.into()),
            }
        }

        if bytes_by_sprite.is_empty() {
            return Ok(());
        }

        let decode_workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(bytes_by_sprite.len())
            .max(1);

        let decoded_sprites = if bytes_by_sprite.len() <= 1 {
            bytes_by_sprite
                .iter()
                .map(|(key, bytes)| {
                    Self::decode_player_sprite(bytes).map(|decoded| (*key, decoded))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        } else {
            let mut decoded = (0..bytes_by_sprite.len())
                .map(|_| None)
                .collect::<Vec<Option<anyhow::Result<(PlayerSpriteKey, DecodedPlayerSprite)>>>>();

            for (index, result) in
                parallel_indexed(bytes_by_sprite.len(), decode_workers, |index| {
                    let (key, bytes) = &bytes_by_sprite[index];
                    Self::decode_player_sprite(bytes).map(|decoded_sprite| (*key, decoded_sprite))
                })
            {
                decoded[index] = Some(result);
            }

            decoded
                .into_iter()
                .map(|result| {
                    result.expect("parallel player sprite decode worker did not fill result")
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        };

        for (key, decoded) in decoded_sprites {
            let loaded_sprite =
                Self::finalize_player_sprite(&mut self.atlas, &key, queue, decoded, 0);
            self.loaded_sprites.insert(key, loaded_sprite);
        }

        Ok(())
    }

    fn player_sprite_path(key: &PlayerSpriteKey) -> String {
        if key.slot == PlayerPieceType::Emote {
            format!("khan/em/{:03}.epfanim", key.sprite_id % 1000)
        } else {
            format!(
                "khan/{}{}/{:03}.epfanim",
                key.gender.char(),
                key.slot.prefix(key.sprite_id),
                key.sprite_id % 1000
            )
        }
    }

    fn decode_player_sprite(epf_bytes: &[u8]) -> anyhow::Result<DecodedPlayerSprite> {
        let (epf_image, _) = oxicode::decode_from_slice::<Vec<EpfAnimation>>(epf_bytes)?;

        let mut animations = FxHashMap::default();
        let mut current_offset = 0;

        for (i, anim) in epf_image.iter().enumerate() {
            animations.insert(
                (anim.animation_type, anim.direction),
                AnimationData {
                    frame_count: anim.image.frames.len(),
                    start_frame_index: current_offset,
                    epf_index: i,
                },
            );
            current_offset += anim.image.frames.len();
        }

        Ok(DecodedPlayerSprite {
            epf_image,
            animations,
        })
    }

    fn finalize_player_sprite(
        atlas: &mut TextureAtlas,
        key: &PlayerSpriteKey,
        queue: &wgpu::Queue,
        decoded: DecodedPlayerSprite,
        ref_count: usize,
    ) -> LoadedSprite {
        let mut allocations: Vec<Option<Allocation>> = Vec::new();
        allocations.reserve(decoded.epf_image.iter().map(|a| a.image.frames.len()).sum());

        for anim in &decoded.epf_image {
            for frame in &anim.image.frames {
                let w = frame.right - frame.left;
                let h = frame.bottom - frame.top;
                if w > 0 && h > 0 {
                    let alloc = atlas.allocate(queue, w as usize, h as usize, &frame.data);
                    if alloc.is_none() {
                        error!(
                            "Player atlas full - cannot allocate sprite {:?} ({}x{})",
                            key, w, h
                        );
                    }
                    allocations.push(alloc);
                } else {
                    allocations.push(None);
                }
            }
        }

        LoadedSprite {
            epf_image: decoded.epf_image,
            allocations,
            animations: decoded.animations,
            ref_count,
        }
    }

    fn try_load_player_sprite(
        atlas: &mut TextureAtlas,
        key: &PlayerSpriteKey,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> anyhow::Result<LoadedSprite> {
        let path = Self::player_sprite_path(key);
        let epf_bytes = archive.get_file(&path)?;
        let decoded = Self::decode_player_sprite(&epf_bytes)?;

        Ok(Self::finalize_player_sprite(atlas, key, queue, decoded, 1))
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Returns the piece-local frame count for the given animation type and direction,
    /// or None if the sprite or animation variant is not loaded.
    pub fn animation_frame_count(
        &self,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
        is_towards: bool,
    ) -> Option<usize> {
        let loaded = self.loaded_sprites.get(&handle.key)?;
        let dir = if is_towards {
            AnimationDirection::Towards
        } else {
            AnimationDirection::Away
        };
        loaded
            .animations
            .get(&(animation_type, dir))
            .map(|a| a.frame_count)
    }
}

impl PlayerBatch {
    pub fn new(device: &wgpu::Device, store: &PlayerAssetStore) -> Self {
        let vertices = make_quad(VERTEX_WIDTH as u32, VERTEX_HEIGHT as u32).to_vec();
        Self {
            batch: SpriteBatch::new(device, vertices, store.bind_group.clone()),
        }
    }

    pub fn preview_instance_count(&self) -> usize {
        self.batch.len()
    }

    pub fn clear(&self) {
        self.batch.clear();
    }

    pub fn clear_and_unload(&self, store: &mut PlayerAssetStore) {
        self.batch.clear_and_unload(|key| store.unload_sprite(*key));
    }

    pub fn add_player_sprite(
        &self,
        queue: &wgpu::Queue,
        store: &mut PlayerAssetStore,
        archive: &Archive,
        sprite: PlayerSpriteKey,
        color: u8,
        direction: u8,
        x: f32,
        y: f32,
        entity_id: u32,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<PlayerSpriteHandle> {
        if store.missing_sprites.contains(&sprite) {
            return Err(anyhow::anyhow!("Sprite marked missing: {:?}", sprite));
        }

        let loaded_sprite = match store.loaded_sprites.entry(sprite) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let s = entry.into_mut();
                s.ref_count += 1;
                s
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let loaded_sprite = match PlayerAssetStore::try_load_player_sprite(
                    &mut store.atlas,
                    &sprite,
                    queue,
                    archive,
                ) {
                    Ok(loaded_sprite) => loaded_sprite,
                    Err(error) => {
                        if let Some(formats::game_files::SquashfsError::FileNotFound(_)) =
                            error.downcast_ref::<formats::game_files::SquashfsError>()
                        {
                            store.missing_sprites.insert(sprite);
                        }
                        return Err(error);
                    }
                };
                entry.insert(loaded_sprite)
            }
        };

        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;

        // Use entity_id as tiebreaker for players on the same tile
        let stack_order = (entity_id % PLAYERS_PER_TILE as u32) as u8;

        let instance = match PlayerAssetStore::get_instance_for_frame(
            &store.palettes,
            loaded_sprite,
            &sprite,
            EpfAnimationType::Idle,
            0,
            Vec2::new(x, y),
            is_towards,
            flip,
            color,
            flags,
            tint,
            stack_order,
        ) {
            Ok(inst) => inst,
            Err(_) => {
                // If Idle is missing (e.g. for purely emote pieces), just use an empty instance
                Instance::default()
            }
        };

        let instance_index = self
            .batch
            .add_instance(queue, instance)
            .expect("Failed to add instance to batch");

        let handle = PlayerSpriteHandle {
            key: sprite,
            index: PlayerSpriteIndex(instance_index),
            stack_order,
        };

        self.batch.insert_handle(handle.index.0, handle.key);

        Ok(handle)
    }

    pub fn update_player_sprite(
        &self,
        queue: &wgpu::Queue,
        store: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        direction: u8,
        x: f32,
        y: f32,
        color: u8,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<()> {
        let loaded_sprite = store
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;

        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;

        let instance = PlayerAssetStore::get_instance_for_frame(
            &store.palettes,
            loaded_sprite,
            &handle.key,
            EpfAnimationType::Idle,
            0,
            Vec2::new(x, y),
            is_towards,
            flip,
            color,
            flags,
            tint,
            handle.stack_order,
        )?;
        self.batch.update_instance(queue, handle.index.0, instance);

        Ok(())
    }

    pub fn supports_animation(
        &self,
        store: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
    ) -> bool {
        let Some(loaded_sprite) = store.loaded_sprites.get(&handle.key) else {
            return false;
        };

        loaded_sprite
            .animations
            .keys()
            .any(|(anim_type, _)| *anim_type == animation_type)
    }

    /// Returns the piece-local frame count for the given animation and direction.
    pub fn animation_frame_count(
        &self,
        store: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
        is_towards: bool,
    ) -> Option<usize> {
        store.animation_frame_count(handle, animation_type, is_towards)
    }

    pub fn update_player_sprite_with_animation(
        &self,
        queue: &wgpu::Queue,
        store: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        direction: u8,
        x: f32,
        y: f32,
        color: u8,
        animation_type: EpfAnimationType,
        frame_index: usize,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<()> {
        let loaded_sprite = store
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;

        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;

        let instance = PlayerAssetStore::get_instance_for_frame(
            &store.palettes,
            loaded_sprite,
            &handle.key,
            animation_type,
            frame_index,
            Vec2::new(x, y),
            is_towards,
            flip,
            color,
            flags,
            tint,
            handle.stack_order,
        )
        .unwrap_or_default();

        self.batch.update_instance(queue, handle.index.0, instance);
        Ok(())
    }

    pub fn hide_player_sprite(
        &self,
        queue: &wgpu::Queue,
        handle: &PlayerSpriteHandle,
    ) -> anyhow::Result<()> {
        self.batch
            .update_instance(queue, handle.index.0, Instance::default());
        Ok(())
    }

    pub fn remove_player_sprite(
        &self,
        queue: &wgpu::Queue,
        store: &mut PlayerAssetStore,
        handle: PlayerSpriteHandle,
    ) {
        self.batch.remove_instance(queue, handle.index.0);
        store.unload_sprite(handle.key);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.batch.draw(render_pass);
    }
}
