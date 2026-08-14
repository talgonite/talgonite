mod palettes;
pub mod types;

pub use palettes::*;
pub use types::*;

use bevy_math::{Vec2, Vec3};
use etagere::Allocation;
use formats::epf::{AnimationDirection, EpfAnimationType};
use formats::sheets::PlayerSheet;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::error;
use wgpu;

use crate::instance::InstanceFlag;
use crate::make_quad;
use crate::scene::sprite_store::{SpriteStore, SpriteStoreLifecycle, allocate_chunks};
use crate::scene::unified_batch::{SpriteScene, build_player_instance};
use crate::scene::utils::{atlas_uv, calculate_tile_z, direction_to_orientation};
use crate::scene::{
    Instance, TILE_WIDTH_HALF, Z_PLAYERS_BASE, get_isometric_coordinate,
    sprite::SpriteBatch,
    sprite_atlas::{PaletteRows, SPRITE_ATLAS_HEIGHT, SPRITE_ATLAS_WIDTH, SpriteAtlas},
    texture_atlas::{FrameRow, FrameUpload, merge_uploads},
};
use formats::game_files::SquashfsArchive;

type Archive = SquashfsArchive;

const VERTEX_WIDTH: usize = 512;
const VERTEX_HEIGHT: usize = 512;
const PLAYER_Y_OFFSET: f32 = -70.0;

pub struct PlayerAssetStore {
    pub(crate) loaded_sprites: FxHashMap<PlayerSpriteKey, LoadedSprite>,
    pub(crate) missing_sprites: FxHashSet<PlayerSpriteKey>,
    pub(crate) palettes: PlayerPalettes,
}

impl SpriteStoreLifecycle for PlayerAssetStore {
    fn label(&self) -> &'static str {
        "players"
    }

    fn cached_count(&self) -> usize {
        self.loaded_sprites.len()
    }

    fn evict_unused(&mut self, atlas: &mut SpriteAtlas, _queue: &wgpu::Queue) {
        self.evict_unused(atlas);
    }
}

impl SpriteStore for PlayerAssetStore {
    type Key = PlayerSpriteKey;
    type Sheet = PlayerSheet;

    fn ensure_loaded(
        &mut self,
        key: &Self::Key,
        atlas: &mut SpriteAtlas,
        queue: &wgpu::Queue,
        archive: &Archive,
        others: &mut [&mut dyn crate::scene::sprite_store::SpriteStoreLifecycle],
    ) -> anyhow::Result<()> {
        if self.missing_sprites.contains(key) {
            return Err(anyhow::anyhow!("Sprite marked missing: {:?}", key));
        }

        if !self.loaded_sprites.contains_key(key) {
            let base = Self::player_sprite_path(key);
            let sheet_bytes = archive
                .get_file(&format!("{base}.sheet.bin"))
                .map_err(|error| {
                    if matches!(&error, formats::game_files::SquashfsError::FileNotFound(_)) {
                        self.missing_sprites.insert(*key);
                    }
                    anyhow::Error::from(error)
                })?;
            let (sheet, consumed): (PlayerSheet, usize) = oxicode::decode_from_slice(&sheet_bytes)?;
            let chunk_slices =
                formats::sheets::chunk_pixel_slices(&sheet_bytes, consumed, &sheet.chunks, 1)?;
            let allocations = allocate_chunks(atlas, queue, self, others, &sheet.chunks);
            let (loaded_sprite, uploads) =
                self.stage_player_sheet(key, sheet, &chunk_slices, 1, allocations);
            let uploads = merge_uploads(uploads);
            atlas.upload_batch(queue, &uploads);
            self.loaded_sprites.insert(*key, loaded_sprite);
        }

        self.loaded_sprites
            .get_mut(key)
            .expect("sprite loaded above")
            .ref_count += 1;
        Ok(())
    }
}

/// Max players per tile for z-ordering (wraps after this)
pub(crate) const PLAYERS_PER_TILE: u8 = 3;
/// Z range within a tile allocated for player stacking.
/// Must be much smaller than z_priority layer differences (~0.01) to avoid
/// equipment parts from different players interleaving.
const PLAYER_STACK_Z_RANGE: f32 = 0.003;

pub struct PlayerBatch {
    batch: SpriteBatch<PlayerSpriteKey>,
}

impl PlayerAssetStore {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new(archive: &Archive) -> Self {
        Self {
            loaded_sprites: FxHashMap::default(),
            missing_sprites: FxHashSet::default(),
            palettes: PlayerPalettes::new(archive),
        }
    }

    pub(crate) fn get_instance_for_frame(
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
        rows: &PaletteRows,
    ) -> anyhow::Result<Instance> {
        let (palette_v, palette_dye) = palettes.get_palette_params(sprite, dye_color, rows);
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
            SPRITE_ATLAS_WIDTH as f32,
            SPRITE_ATLAS_HEIGHT as f32,
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
    pub(crate) fn release_sprite(&mut self, key: PlayerSpriteKey) {
        if let Some(sprite) = self.loaded_sprites.get_mut(&key) {
            sprite.ref_count = sprite.ref_count.saturating_sub(1);
        }
    }

    pub(crate) fn supports_animation(
        &self,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
    ) -> bool {
        let Some(loaded_sprite) = self.loaded_sprites.get(&handle.key) else {
            return false;
        };
        loaded_sprite
            .animations
            .keys()
            .any(|(anim_type, _)| *anim_type == animation_type)
    }

    /// Frees every cached sprite with no live references, returning their
    /// atlas slots to etagere. Called when an allocation fails so a busy map
    /// can reclaim space from sprites of previous maps/players.
    pub(crate) fn evict_unused(&mut self, atlas: &mut SpriteAtlas) {
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
                    atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted = to_evict.len(),
            remaining = self.loaded_sprites.len(),
            "Evicted unused player sprites to make room in the atlas"
        );
    }

    pub(crate) fn player_sprite_path(key: &PlayerSpriteKey) -> String {
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

    /// Stages each chunk image for a batched upload into pre-allocated atlas
    /// slots. `None` allocations (atlas was full) yield a cached sprite with
    /// no slots, so a later load does not repeat decode work.
    #[tracing::instrument(level = "info", skip_all, fields(sprite = ?key))]
    pub(crate) fn stage_player_sheet<'a>(
        &mut self,
        key: &PlayerSpriteKey,
        sheet: PlayerSheet,
        chunk_slices: &[&'a [u8]],
        ref_count: usize,
        allocations: Option<Vec<Allocation>>,
    ) -> (LoadedSprite, Vec<FrameUpload<'a>>) {
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

        let Some(allocations) = allocations else {
            error!("Sprite atlas full - cannot allocate sprite {:?}", key);
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

        let mut uploads = Vec::with_capacity(chunk_slices.len());
        for (chunk_index, &image) in chunk_slices.iter().enumerate() {
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

    /// Decodes and stages a batch of player sprites in one parallel pass,
    /// uploading all of them in a single submit.
    pub fn preload_player_sprites(
        &mut self,
        queue: &wgpu::Queue,
        archive: &Archive,
        atlas: &mut SpriteAtlas,
        others: &mut [&mut dyn crate::scene::sprite_store::SpriteStoreLifecycle],
        sprites: &[PlayerSpriteKey],
    ) -> anyhow::Result<()> {
        let mut queued = std::collections::HashSet::new();
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

        let sheet_paths: Vec<String> = sprites_to_load
            .iter()
            .map(|(_, base)| format!("{base}.sheet.bin"))
            .collect();
        let sheet_results = archive.get_files_parallel(&sheet_paths);

        let mut decoded: Vec<(PlayerSpriteKey, PlayerSheet, usize)> =
            Vec::with_capacity(sheet_results.len());
        let mut sheet_files: Vec<Vec<u8>> = Vec::with_capacity(sheet_results.len());
        for ((key, _base), result) in sprites_to_load.into_iter().zip(sheet_results) {
            match result {
                Ok(bytes) => {
                    let (sheet, consumed): (PlayerSheet, usize) =
                        oxicode::decode_from_slice(&bytes)?;
                    sheet_files.push(bytes);
                    decoded.push((key, sheet, consumed));
                }
                Err(formats::game_files::SquashfsError::FileNotFound(_)) => {
                    self.missing_sprites.insert(key);
                    sheet_files.push(Vec::new());
                }
                Err(error) => return Err(error.into()),
            }
        }
        if decoded.is_empty() {
            return Ok(());
        }

        let mut images_by_sprite: Vec<Vec<&[u8]>> = Vec::with_capacity(decoded.len());
        for (sprite_index, (_, sheet, consumed)) in decoded.iter().enumerate() {
            let slices = formats::sheets::chunk_pixel_slices(
                &sheet_files[sprite_index],
                *consumed,
                &sheet.chunks,
                1,
            )?;
            images_by_sprite.push(slices);
        }

        let mut uploads = Vec::new();
        let mut staged: Vec<(PlayerSpriteKey, LoadedSprite)> = Vec::with_capacity(decoded.len());
        for (sprite_index, (key, sheet, _)) in decoded.into_iter().enumerate() {
            let allocations = allocate_chunks(atlas, queue, self, others, &sheet.chunks);
            let (loaded_sprite, sprite_uploads) = self.stage_player_sheet(
                &key,
                sheet,
                &images_by_sprite[sprite_index],
                0,
                allocations,
            );
            uploads.extend(sprite_uploads);
            staged.push((key, loaded_sprite));
        }
        let uploads = merge_uploads(uploads);
        atlas.upload_batch(queue, &uploads);

        for (key, loaded_sprite) in staged {
            self.loaded_sprites.insert(key, loaded_sprite);
        }
        Ok(())
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
    pub fn new(device: &wgpu::Device, scene: &SpriteScene) -> Self {
        let vertices = make_quad(VERTEX_WIDTH as u32, VERTEX_HEIGHT as u32).to_vec();
        Self {
            batch: SpriteBatch::new(device, vertices, scene.atlas.bind_group().clone()),
        }
    }

    pub fn flush_pending(&self, encoder: &mut wgpu::CommandEncoder) {
        self.batch.flush_pending(encoder);
    }

    pub fn finish_uploads(&self) {
        self.batch.finish_uploads();
    }

    pub fn recall_uploads(&self) {
        self.batch.recall_uploads();
    }

    pub fn preview_instance_count(&self) -> usize {
        self.batch.len()
    }

    pub fn clear(&self) {
        self.batch.clear();
    }

    pub fn clear_and_unload(&self, scene: &mut SpriteScene) {
        self.batch
            .clear_and_unload(|key| scene.players.release_sprite(*key));
    }

    pub fn add_player_sprite(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
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
        scene.ensure_player(&sprite, queue, archive)?;
        let (instance, stack_order) = build_player_instance(
            scene, &sprite, color, direction, x, y, entity_id, flags, tint,
        );

        let instance_index = self
            .batch
            .add_instance(instance)
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
        scene: &SpriteScene,
        handle: &PlayerSpriteHandle,
        direction: u8,
        x: f32,
        y: f32,
        color: u8,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<()> {
        let loaded_sprite = scene
            .players
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;

        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;

        let instance = PlayerAssetStore::get_instance_for_frame(
            &scene.players.palettes,
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
            scene.atlas.palette_rows(),
        )?;
        self.batch.update_instance(handle.index.0, instance);

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
        scene: &SpriteScene,
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
        let loaded_sprite = scene
            .players
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;

        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;

        let instance = PlayerAssetStore::get_instance_for_frame(
            &scene.players.palettes,
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
            scene.atlas.palette_rows(),
        )
        .unwrap_or_default();

        self.batch.update_instance(handle.index.0, instance);
        Ok(())
    }

    pub fn hide_player_sprite(
        &self,
        handle: &PlayerSpriteHandle,
    ) -> anyhow::Result<()> {
        self.batch
            .update_instance(handle.index.0, Instance::default());
        Ok(())
    }

    pub fn remove_player_sprite(
        &self,
        scene: &mut SpriteScene,
        handle: PlayerSpriteHandle,
    ) {
        self.batch.remove_instance(handle.index.0);
        scene.players.release_sprite(handle.key);
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.batch.draw(render_pass);
    }
}
