mod palettes;
pub mod types;

pub use palettes::*;
pub use types::*;

use etagere::Allocation;
use formats::epf::{AnimationDirection, EpfAnimationType};
use formats::sheets::PlayerSheet;
use bevy_math::{Vec2, Vec3};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::error;
use wgpu;

use crate::instance::InstanceFlag;
use crate::make_quad;
use crate::scene::utils::{atlas_uv, calculate_tile_z, direction_to_orientation};
use crate::{
    scene::{
        Instance, TILE_WIDTH_HALF, Z_PLAYERS_BASE, get_isometric_coordinate,
        sprite::SpriteBatch,
        texture_atlas::{FrameRow, FrameUpload, TextureAtlas, merge_uploads},
        texture_bind::TextureBind,
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

impl PlayerAssetStore {
    #[tracing::instrument(level = "info", skip_all)]
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

        let atlas = TextureAtlas::new(device, diffuse_texture.texture);

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

        let Some(frame) = loaded_sprite
            .frames
            .get(anim_data.start_frame_index + frame_index)
            .copied()
            .flatten()
        else {
            return Ok(Instance::default());
        };

        let frame_w = frame.width as f32;
        let frame_h = frame.height as f32;
        let allocation = loaded_sprite
            .allocations
            .get(frame.chunk as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No allocation for sprite: {:?} at frame {}",
                    sprite,
                    frame_index
                )
            })?;

        let mut frame_offset = Vec2::new(frame.left as f32, frame.top as f32);
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
                (allocation.rectangle.min.x + frame.x as i32) as f32,
                (allocation.rectangle.min.y + frame.y as i32) as f32,
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

    /// Drops one reference. The sprite stays cached (atlas slot included) so
    /// it can be reused without re-decoding or re-uploading; it is only
    /// evicted when the atlas actually needs the space (see `evict_unused`).
    fn release_sprite(&mut self, key: PlayerSpriteKey) {
        if let Some(sprite) = self.loaded_sprites.get_mut(&key) {
            sprite.ref_count = sprite.ref_count.saturating_sub(1);
        }
    }

    /// Frees every cached sprite with no live references, returning their
    /// atlas slots to etagere. Called when an allocation fails so a busy map
    /// can reclaim space from sprites of previous maps/players.
    fn evict_unused(&mut self) {
        let mut to_evict = Vec::new();
        for (key, sprite) in &self.loaded_sprites {
            if sprite.ref_count == 0 {
                to_evict.push(*key);
            }
        }
        if to_evict.is_empty() {
            return;
        }
        for key in &to_evict {
            if let Some(sprite) = self.loaded_sprites.remove(key) {
                for allocation in &sprite.allocations {
                    self.atlas.atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted = to_evict.len(),
            remaining = self.loaded_sprites.len(),
            "Evicted unused player sprites to make room in the atlas"
        );
    }

    #[tracing::instrument(level = "info", skip_all, fields(sprite_count = sprites.len()))]
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

        // Fetch every sprite's sheet metadata in one parallel pass.
        let meta_paths: Vec<String> = sprites_to_load
            .iter()
            .map(|(_, base)| format!("{base}.sheet.bin"))
            .collect();
        let meta_results = archive.get_files_parallel(&meta_paths);

        let mut decoded: Vec<(PlayerSpriteKey, PlayerSheet)> =
            Vec::with_capacity(meta_results.len());
        for ((key, _base), result) in sprites_to_load.into_iter().zip(meta_results) {
            match result {
                Ok(bytes) => {
                    let (sheet, _) = oxicode::decode_from_slice::<PlayerSheet>(&bytes)?;
                    decoded.push((key, sheet));
                }
                Err(formats::game_files::SquashfsError::FileNotFound(_)) => {
                    self.missing_sprites.insert(key);
                }
                Err(error) => return Err(error.into()),
            }
        }

        if decoded.is_empty() {
            return Ok(());
        }

        // Fetch every sheet image in one parallel pass. `image_plan` maps each
        // result back to its sprite and chunk index.
        let mut image_plan: Vec<(usize, usize)> = Vec::new();
        let mut image_paths: Vec<String> = Vec::new();
        for (sprite_index, (key, sheet)) in decoded.iter().enumerate() {
            let base = Self::player_sprite_path(key);
            for chunk_index in 0..sheet.chunks.len() {
                image_paths.push(format!("{base}.sheet{chunk_index}.ktx2"));
                image_plan.push((sprite_index, chunk_index));
            }
        }
        let image_results = archive.get_files_parallel(&image_paths);
        let mut images_by_sprite: Vec<Vec<Vec<u8>>> = decoded.iter().map(|_| Vec::new()).collect();
        for ((sprite_index, chunk_index), result) in image_plan.into_iter().zip(image_results) {
            let bytes = result?;
            let (_, _, pixels) = texture::Texture::load_ktx2(&bytes)?;
            while images_by_sprite[sprite_index].len() <= chunk_index {
                images_by_sprite[sprite_index].push(Vec::new());
            }
            images_by_sprite[sprite_index][chunk_index] = pixels;
        }

        let _finalize_span = tracing::info_span!(
            "player_sprites.finalize_batch",
            sprite_count = decoded.len()
        )
        .entered();

        // Stage every sprite (slot allocation + staging data) first, then do
        // one batched GPU upload for the whole set.
        let mut uploads: Vec<FrameUpload> = Vec::new();
        let mut staged: Vec<(PlayerSpriteKey, LoadedSprite)> = Vec::with_capacity(decoded.len());
        for (sprite_index, (key, sheet)) in decoded.into_iter().enumerate() {
            let images = std::mem::take(&mut images_by_sprite[sprite_index]);
            let (loaded_sprite, sprite_uploads) = self.stage_player_sheet(&key, sheet, images, 0);
            uploads.extend(sprite_uploads);
            staged.push((key, loaded_sprite));
        }

        let uploads = merge_uploads(uploads);
        self.atlas.upload_batch(queue, &uploads);

        for (key, loaded_sprite) in staged {
            self.loaded_sprites.insert(key, loaded_sprite);
        }

        drop(_finalize_span);
        Ok(())
    }

    fn player_sprite_path(key: &PlayerSpriteKey) -> String {
        if key.slot == PlayerPieceType::Emote {
            format!("khan/em/{:03}", key.sprite_id % 1000)
        } else {
            format!(
                "khan/{}{}/{:03}",
                key.gender.char(),
                key.slot.prefix(key.sprite_id),
                key.sprite_id % 1000
            )
        }
    }

    /// Reads the pre-packed sheet images for a sprite (one per chunk).
    fn load_sheet_images(
        archive: &Archive,
        base: &str,
        chunk_count: usize,
    ) -> anyhow::Result<Vec<Vec<u8>>> {
        let paths: Vec<String> = (0..chunk_count)
            .map(|chunk| format!("{base}.sheet{chunk}.ktx2"))
            .collect();
        let results = archive.get_files_parallel(&paths);
        let mut images = Vec::with_capacity(chunk_count);
        for result in results {
            let bytes = result?;
            let (_, _, pixels) = texture::Texture::load_ktx2(&bytes)?;
            images.push(pixels);
        }
        Ok(images)
    }

    /// Allocates one atlas slot per sheet chunk and stages each chunk image
    /// for a batched upload. If the atlas cannot make room, returns a sprite
    /// with no slots (kept cached so a later load does not repeat decode work).
    #[tracing::instrument(level = "info", skip_all, fields(sprite = ?key))]
    fn stage_player_sheet(
        &mut self,
        key: &PlayerSpriteKey,
        sheet: PlayerSheet,
        images: Vec<Vec<u8>>,
        ref_count: usize,
    ) -> (LoadedSprite, Vec<FrameUpload>) {
        let animations = sheet
            .animations
            .iter()
            .map(|a| {
                (
                    (a.animation_type, a.direction),
                    AnimationData {
                        frame_count: a.frame_count as usize,
                        start_frame_index: a.start_frame as usize,
                    },
                )
            })
            .collect();

        let mut allocations: Vec<Allocation> = Vec::with_capacity(sheet.chunks.len());
        for chunk in &sheet.chunks {
            let mut allocation = self
                .atlas
                .allocate_slot(chunk.width as usize, chunk.height as usize);
            if allocation.is_none() {
                // Atlas is full: evict unused cached sprites and retry once.
                self.evict_unused();
                allocation = self
                    .atlas
                    .allocate_slot(chunk.width as usize, chunk.height as usize);
            }
            let Some(allocation) = allocation else {
                error!(
                    "Player atlas full - cannot allocate sprite {:?} ({}x{})",
                    key, chunk.width, chunk.height
                );
                for slot in &allocations {
                    self.atlas.atlas.deallocate(slot.id);
                }
                return (
                    LoadedSprite {
                        frames: sheet.frames,
                        allocations: Vec::new(),
                        animations,
                        ref_count,
                    },
                    Vec::new(),
                );
            };
            allocations.push(allocation);
        }

        let mut uploads = Vec::with_capacity(images.len());
        for (chunk_index, image) in images.into_iter().enumerate() {
            let chunk = sheet.chunks[chunk_index];
            uploads.push(FrameUpload {
                rect: allocations[chunk_index].rectangle,
                width: chunk.width as usize,
                height: chunk.height as usize,
                frames: vec![FrameRow {
                    x: 0,
                    y: 0,
                    width: chunk.width as usize,
                    height: chunk.height as usize,
                    data: image,
                }],
            });
        }

        (
            LoadedSprite {
                frames: sheet.frames,
                allocations,
                animations,
                ref_count,
            },
            uploads,
        )
    }

    #[tracing::instrument(level = "info", skip_all, fields(sprite = ?key))]
    fn try_load_player_sprite(
        &mut self,
        key: &PlayerSpriteKey,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> anyhow::Result<LoadedSprite> {
        let base = Self::player_sprite_path(key);
        let meta_bytes = archive.get_file(&format!("{base}.sheet.bin"))?;
        let (sheet, _) = oxicode::decode_from_slice::<PlayerSheet>(&meta_bytes)?;
        let images = Self::load_sheet_images(archive, &base, sheet.chunks.len())?;

        let (loaded_sprite, uploads) = self.stage_player_sheet(key, sheet, images, 1);
        self.atlas.upload_batch(queue, &uploads);
        Ok(loaded_sprite)
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
        self.batch
            .clear_and_unload(|key| store.release_sprite(*key));
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

        // Ensure the sprite is loaded (without holding a map borrow across the
        // load, since loading can evict unused sprites), then take a reference.
        if !store.loaded_sprites.contains_key(&sprite) {
            let loaded_sprite = match store.try_load_player_sprite(&sprite, queue, archive) {
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
            store.loaded_sprites.insert(sprite, loaded_sprite);
        }
        let loaded_sprite = store
            .loaded_sprites
            .get_mut(&sprite)
            .expect("sprite loaded above");
        loaded_sprite.ref_count += 1;

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
        store.release_sprite(handle.key);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.batch.draw(render_pass);
    }
}
