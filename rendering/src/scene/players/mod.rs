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
        texture_atlas::{FrameRow, FrameUpload, TextureAtlas, merge_uploads},
        texture_bind::TextureBind,
    },
    texture,
};
use formats::game_files::SquashfsArchive;

type Archive = SquashfsArchive;

const ATLAS_WIDTH: usize = 4096;
const ATLAS_HEIGHT: usize = 8192;
/// Player parts are stacked into one contiguous atlas slot; parts taller than
/// this are chunked into multiple slots so they stay packable in the atlas.
const PLAYER_SLOT_MAX_HEIGHT: usize = 4096;
/// Frames are shelf-packed into rows up to this width so slots stay short and
/// wide enough for the atlas packer to place efficiently.
const PLAYER_SHELF_TARGET_WIDTH: usize = 512;
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

        let (slot_index, frame_x, frame_y) = loaded_sprite.frame_rows
            [anim_data.start_frame_index + frame_index]
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No allocation for sprite: {:?} at frame {}",
                    sprite,
                    frame_index
                )
            })?;
        let allocation = loaded_sprite.allocations.get(slot_index).ok_or_else(|| {
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
                (allocation.rectangle.min.x + frame_x as i32) as f32,
                (allocation.rectangle.min.y + frame_y as i32) as f32,
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

        let _decode_span = tracing::info_span!(
            "player_sprites.decode_batch",
            sprite_count = bytes_by_sprite.len()
        )
        .entered();

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

        drop(_decode_span);
        let _finalize_span = tracing::info_span!(
            "player_sprites.finalize_batch",
            sprite_count = decoded_sprites.len()
        )
        .entered();

        // Stage every sprite (slot allocation + staging data) first, then do
        // one batched GPU upload for the whole set.
        let mut uploads: Vec<FrameUpload> = Vec::new();
        let mut staged: Vec<(PlayerSpriteKey, LoadedSprite)> =
            Vec::with_capacity(decoded_sprites.len());
        for (key, decoded) in decoded_sprites {
            let (loaded_sprite, sprite_uploads) = self.stage_player_sprite(&key, decoded, 0);
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

    #[tracing::instrument(level = "info", skip_all, fields(bytes = epf_bytes.len()))]
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

    #[tracing::instrument(level = "info", skip_all, fields(sprite = ?key))]
    /// Allocates atlas slots and stages frame data for a sprite, returning the
    /// loaded sprite plus everything that still needs uploading. The caller
    /// batches the returned uploads (see `TextureAtlas::upload_batch`).
    fn stage_player_sprite(
        &mut self,
        key: &PlayerSpriteKey,
        mut decoded: DecodedPlayerSprite,
        ref_count: usize,
    ) -> (LoadedSprite, Vec<FrameUpload>) {
        let frame_count: usize = decoded
            .epf_image
            .iter()
            .map(|a| a.image.frames.len())
            .sum();

        // Pass 1: lay the part's frames out in shelves (rows) within a single
        // slot. Tall stacks of narrow frames don't pack into the atlas, so
        // frames are wrapped at a target row width to keep slots short and
        // wide. Unusually large parts double the target width until the slot
        // height is bounded.
        let mut frame_rows = vec![None; frame_count];
        let non_empty: Vec<(usize, usize, usize)> = decoded
            .epf_image
            .iter()
            .flat_map(|a| a.image.frames.iter())
            .enumerate()
            .filter_map(|(frame_index, frame)| {
                let w = (frame.right - frame.left) as usize;
                let h = (frame.bottom - frame.top) as usize;
                (w > 0 && h > 0).then_some((frame_index, w, h))
            })
            .collect();

        let pack_shelves = |non_empty: &[(usize, usize, usize)],
                            target_width: usize|
         -> (Vec<(usize, usize, usize)>, usize, usize) {
            let mut placed = Vec::with_capacity(non_empty.len());
            let mut slot_width = 0usize;
            let mut shelf_y = 0usize;
            let mut shelf_h = 0usize;
            let mut row_x = 0usize;
            for &(frame_index, w, h) in non_empty {
                if row_x > 0 && row_x + w > target_width {
                    shelf_y += shelf_h;
                    row_x = 0;
                    shelf_h = 0;
                }
                placed.push((frame_index, row_x, shelf_y));
                row_x += w;
                slot_width = slot_width.max(row_x);
                shelf_h = shelf_h.max(h);
            }
            (placed, slot_width, shelf_y + shelf_h)
        };

        // (frame indices, slot width, slot height) - one slot per part.
        let mut chunks: Vec<(Vec<usize>, usize, usize)> = Vec::new();
        if !non_empty.is_empty() {
            let mut target_width = PLAYER_SHELF_TARGET_WIDTH;
            let (mut placed, mut slot_width, mut slot_height) =
                pack_shelves(&non_empty, target_width);
            while slot_height > PLAYER_SLOT_MAX_HEIGHT && target_width < ATLAS_WIDTH {
                target_width *= 2;
                (placed, slot_width, slot_height) = pack_shelves(&non_empty, target_width);
            }
            for &(frame_index, x, y) in &placed {
                frame_rows[frame_index] = Some((0, x as u32, y as u32));
            }
            chunks.push((
                placed.into_iter().map(|(frame_index, _, _)| frame_index).collect(),
                slot_width,
                slot_height,
            ));
        }

        // Pass 2: allocate one atlas slot per chunk.
        let mut allocations: Vec<Allocation> = Vec::with_capacity(chunks.len());
        for &(_, slot_width, slot_height) in &chunks {
            let mut allocation = self.atlas.allocate_slot(slot_width, slot_height);
            if allocation.is_none() {
                // Atlas is full: evict unused cached sprites and retry once.
                self.evict_unused();
                allocation = self.atlas.allocate_slot(slot_width, slot_height);
            }
            let Some(allocation) = allocation else {
                error!(
                    "Player atlas full - cannot allocate sprite {:?} ({}x{})",
                    key, slot_width, slot_height
                );
                // Return any slots allocated by earlier chunks of this part.
                for slot in &allocations {
                    self.atlas.atlas.deallocate(slot.id);
                }
                return (
                    LoadedSprite {
                        epf_image: decoded.epf_image,
                        allocations: Vec::new(),
                        frame_rows: vec![None; frame_count],
                        animations: decoded.animations,
                        ref_count,
                    },
                    Vec::new(),
                );
            };
            allocations.push(allocation);
        }

        // Pass 3: hand each frame's pixels to upload_batch as tight rows so the
        // belt's mapped staging memory is the only copy destination. The loaded
        // sprite keeps only frame geometry (right/left/top/bottom).
        let mut frames_per_slot: Vec<Vec<FrameRow>> = Vec::with_capacity(chunks.len());
        for _ in 0..chunks.len() {
            frames_per_slot.push(Vec::new());
        }
        let mut frame_index = 0usize;
        for anim in &mut decoded.epf_image {
            for frame in &mut anim.image.frames {
                let w = (frame.right - frame.left) as usize;
                let h = (frame.bottom - frame.top) as usize;
                if w > 0 && h > 0 {
                    let (slot_index, x, y) = frame_rows[frame_index]
                        .expect("non-empty frame must have a row assigned");
                    let data = std::mem::take(&mut frame.data);
                    frames_per_slot[slot_index].push(FrameRow {
                        x,
                        y,
                        width: w,
                        height: h,
                        data,
                    });
                }
                frame_index += 1;
            }
        }

        let mut uploads = Vec::with_capacity(chunks.len());
        for (slot_index, &(_, slot_width, slot_height)) in chunks.iter().enumerate() {
            uploads.push(FrameUpload {
                rect: allocations[slot_index].rectangle,
                width: slot_width,
                height: slot_height,
                frames: std::mem::take(&mut frames_per_slot[slot_index]),
            });
        }

        (
            LoadedSprite {
                epf_image: decoded.epf_image,
                allocations,
                frame_rows,
                animations: decoded.animations,
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
        let path = Self::player_sprite_path(key);
        let epf_bytes = archive.get_file(&path)?;
        let decoded = Self::decode_player_sprite(&epf_bytes)?;

        let (loaded_sprite, uploads) = self.stage_player_sprite(key, decoded, 1);
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
        self.batch.clear_and_unload(|key| store.release_sprite(*key));
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
