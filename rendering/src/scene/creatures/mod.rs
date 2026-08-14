pub mod types;
pub use types::*;

use bevy_math::Vec2;
use formats::game_files::SquashfsArchive;
use formats::sheets::CreatureSheet;
use rustc_hash::FxHashMap;

use crate::scene::{
    Instance, Z_CREATURES, get_isometric_coordinate,
    sprite_atlas::{PaletteRows, SPRITE_ATLAS_HEIGHT, SPRITE_ATLAS_WIDTH, SpriteAtlas},
    sprite_store::{SpriteStore, SpriteStoreLifecycle, allocate_chunks},
    texture_atlas::{FrameRow, FrameUpload, merge_uploads},
};
use crate::{
    instance::InstanceFlag,
    scene::utils::{atlas_uv, calculate_tile_z},
};

const VERTEX_WIDTH: usize = 512;
const VERTEX_HEIGHT: usize = 512;

pub struct CreatureAssetStore {
    pub(crate) loaded_sprites: FxHashMap<u16, LoadedSprite>,
}

impl SpriteStoreLifecycle for CreatureAssetStore {
    fn label(&self) -> &'static str {
        "creatures"
    }

    fn cached_count(&self) -> usize {
        self.loaded_sprites.len()
    }

    fn evict_unused(&mut self, atlas: &mut SpriteAtlas, _queue: &wgpu::Queue) {
        self.evict_unused(atlas);
    }
}

impl SpriteStore for CreatureAssetStore {
    type Key = u16;
    type Sheet = CreatureSheet;

    fn ensure_loaded(
        &mut self,
        key: &Self::Key,
        atlas: &mut SpriteAtlas,
        queue: &wgpu::Queue,
        archive: &SquashfsArchive,
        others: &mut [&mut dyn crate::scene::sprite_store::SpriteStoreLifecycle],
    ) -> anyhow::Result<()> {
        let sprite_id = *key;
        if let Some(sprite) = self.loaded_sprites.get_mut(&sprite_id) {
            sprite.ref_count += 1;
            return Ok(());
        }

        let base = format!("hades/mns{:03}", sprite_id);
        let sheet_bytes = archive
            .get_file(&format!("{base}.sheet.bin"))
            .map_err(|e| {
                anyhow::anyhow!("Failed to load sheet for creature {}: {}", sprite_id, e)
            })?;
        let (meta, consumed): (CreatureSheet, usize) = oxicode::decode_from_slice(&sheet_bytes)?;
        let chunk_slices =
            formats::sheets::chunk_pixel_slices(&sheet_bytes, consumed, &meta.chunks, 1)?;
        let allocations = allocate_chunks(atlas, queue, self, others, &meta.chunks)
            .ok_or_else(|| anyhow::anyhow!("Atlas full for creature {}", sprite_id))?;
        let (allocations, uploads) = self
            .stage_sheet(&meta, &chunk_slices, Some(allocations))
            .expect("allocations checked above");
        let uploads = merge_uploads(uploads);
        atlas.upload_batch(queue, &uploads);
        self.loaded_sprites.insert(
            sprite_id,
            LoadedSprite {
                meta,
                allocations,
                ref_count: 1,
            },
        );
        Ok(())
    }
}

impl CreatureAssetStore {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new() -> Self {
        Self {
            loaded_sprites: FxHashMap::default(),
        }
    }
}

impl Default for CreatureAssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CreatureAssetStore {
    /// Drops one reference. The sprite stays cached so it can be reused
    /// without re-decoding or re-uploading; it is only evicted when the atlas
    /// needs the space (see `evict_unused`).
    pub(crate) fn release_sprite(&mut self, sprite_id: u16) {
        if let Some(sprite) = self.loaded_sprites.get_mut(&sprite_id) {
            sprite.ref_count = sprite.ref_count.saturating_sub(1);
        }
    }

    /// Frees every cached sprite with no live references, returning their
    /// atlas slots to etagere. Called when an allocation fails.
    pub(crate) fn evict_unused(&mut self, atlas: &mut SpriteAtlas) {
        let mut to_evict = Vec::new();
        for (id, sprite) in &self.loaded_sprites {
            if sprite.ref_count == 0 {
                to_evict.push(*id);
            }
        }
        if to_evict.is_empty() {
            return;
        }
        for id in &to_evict {
            if let Some(sprite) = self.loaded_sprites.remove(id) {
                for allocation in &sprite.allocations {
                    atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted = to_evict.len(),
            remaining = self.loaded_sprites.len(),
            "Evicted unused creature sprites to make room in the atlas"
        );
    }

    /// Stages each chunk image into pre-allocated atlas slots for a batched
    /// upload. `None` means the atlas was full and the sprite could not load.
    pub(crate) fn stage_sheet<'a>(
        &mut self,
        meta: &CreatureSheet,
        chunk_slices: &[&'a [u8]],
        allocations: Option<Vec<etagere::Allocation>>,
    ) -> Option<(Vec<etagere::Allocation>, Vec<FrameUpload<'a>>)> {
        let allocations = allocations?;

        let mut uploads = Vec::with_capacity(chunk_slices.len());
        for (chunk_index, &image) in chunk_slices.iter().enumerate() {
            let chunk = meta.chunks[chunk_index];
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

        Some((allocations, uploads))
    }
}

pub(crate) fn get_instance_for_frame(
    loaded_sprite: &LoadedSprite,
    frame_index: usize,
    position: Vec2,
    flip: bool,
    rows: &PaletteRows,
    flags: InstanceFlag,
) -> anyhow::Result<Instance> {
    let Some(frame) = loaded_sprite
        .meta
        .frames
        .get(frame_index)
        .copied()
        .flatten()
    else {
        return Ok(Instance::default());
    };
    let allocation = loaded_sprite
        .allocations
        .get(frame.chunk as usize)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No allocation for sprite {} frame {}",
                loaded_sprite.meta.palette_number,
                frame_index
            )
        })?;

    let frame_w = frame.width as i32;
    let frame_h = frame.height as i32;

    let offset_x = if flip {
        (frame.right - frame.center_x) as f32
    } else {
        (frame.center_x - frame.left) as f32
    };

    let (tex_min, tex_max) = atlas_uv(
        Vec2::new(
            (allocation.rectangle.min.x + frame.x as i32) as f32,
            (allocation.rectangle.min.y + frame.y as i32) as f32,
        ),
        frame_w as f32,
        frame_h as f32,
        SPRITE_ATLAS_WIDTH as f32,
        SPRITE_ATLAS_HEIGHT as f32,
    );

    Ok(Instance::with_texture_atlas(
        (get_isometric_coordinate(position.x, position.y)
            - Vec2::new(offset_x, (frame.center_y - frame.top) as f32))
        .extend(calculate_tile_z(position.x, position.y, Z_CREATURES)),
        tex_min,
        tex_max,
        Vec2::new(
            frame_w as f32 / VERTEX_WIDTH as f32,
            frame_h as f32 / VERTEX_HEIGHT as f32,
        ),
        rows.row(rows.creatures, loaded_sprite.meta.palette_number as u32),
        -1.,
        flip,
        false,
        flags,
    ))
}
