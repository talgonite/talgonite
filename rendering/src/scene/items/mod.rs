pub mod types;
pub use types::*;

use bevy_math::Vec2;
use formats::sheets::{ItemSheet, SheetChunk};
use std::collections::HashMap;
use tracing::error;

use crate::{
    instance::InstanceFlag,
    scene::{
        Instance, Z_ITEMS, get_isometric_coordinate,
        sprite_atlas::{PaletteRows, SPRITE_ATLAS_HEIGHT, SPRITE_ATLAS_WIDTH, SpriteAtlas},
        sprite_store::{SpriteStore, SpriteStoreLifecycle, allocate_chunks},
        texture_atlas::{FrameRow, FrameUpload, merge_uploads},
        utils::{atlas_uv, calculate_tile_z},
    },
};

pub const ITEMS_PER_EPF_FILE: u32 = 266;

pub struct ItemAssetStore {
    pub(crate) loaded_sheets: HashMap<u32, LoadedItemSheet>,
    pub(crate) palette_table: rangemap::RangeMap<u16, u16>,
    /// Sheets staged this frame but not yet uploaded; flushed once per frame
    /// so a burst of item spawns shares a single staging belt submit. The raw
    /// file bytes are kept so the flush can borrow pixel slices straight from
    /// them (no per-chunk copy at stage time).
    pending_sheets: Vec<PendingSheetUpload>,
}

impl SpriteStoreLifecycle for ItemAssetStore {
    fn label(&self) -> &'static str {
        "items"
    }

    fn cached_count(&self) -> usize {
        self.loaded_sheets.len()
    }

    fn evict_unused(&mut self, atlas: &mut SpriteAtlas, queue: &wgpu::Queue) {
        self.evict_unused_sheets(queue, atlas);
    }
}

impl SpriteStore for ItemAssetStore {
    type Key = u32;
    type Sheet = ItemSheet;

    fn ensure_loaded(
        &mut self,
        key: &Self::Key,
        atlas: &mut SpriteAtlas,
        queue: &wgpu::Queue,
        archive: &formats::game_files::SquashfsArchive,
        others: &mut [&mut dyn crate::scene::sprite_store::SpriteStoreLifecycle],
    ) -> anyhow::Result<()> {
        let sheet_index = *key;
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count += 1;
            return Ok(());
        }

        let base = format!("Legend/item{:03}", sheet_index);
        let sheet_bytes = archive.get_file(&format!("{base}.sheet.bin"))?;
        let (meta, consumed): (ItemSheet, usize) = oxicode::decode_from_slice(&sheet_bytes)?;
        // Validate the pixel blob now so the flush can slice it without erroring.
        formats::sheets::chunk_pixel_slices(&sheet_bytes, consumed, &meta.chunks, 1)?;
        let allocations = allocate_chunks(atlas, queue, self, others, &meta.chunks);
        self.stage_sheet(sheet_index, meta, consumed, sheet_bytes, allocations);
        Ok(())
    }
}

/// One staged item sheet, waiting for the next flush. `bytes` is the sheet
/// file (oxicode metadata + raw chunk pixels), `consumed` is where the pixel
/// blob starts, and `slots` maps each chunk to its allocated atlas rect.
struct PendingSheetUpload {
    bytes: Vec<u8>,
    consumed: usize,
    chunks: Vec<SheetChunk>,
    slots: Vec<(etagere::Rectangle, usize)>,
}

pub const ITEM_Z_RANGE: f32 = Z_ITEMS;
/// Item z-order is based on item.id % this value for deterministic ordering
const ITEM_COUNT_BUCKET_SIZE: u32 = 20;

impl ItemAssetStore {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new(archive: &formats::game_files::SquashfsArchive) -> Self {
        let palette_table_data = archive
            .get_file("Legend/item.tbl.bin")
            .expect("item palette table missing");
        let (palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            oxicode::serde::decode_from_slice(&palette_table_data, oxicode::config::standard())
                .unwrap();

        Self {
            loaded_sheets: HashMap::new(),
            palette_table,
            pending_sheets: Vec::new(),
        }
    }

    /// Stages a decoded sheet into pre-allocated atlas slots: the pixel data
    /// is queued for the next flush and the sheet is cached with ref count 1.
    /// `None` allocations (atlas was full) skip staging so a later add can
    /// retry.
    pub(crate) fn stage_sheet(
        &mut self,
        sheet_index: u32,
        meta: ItemSheet,
        consumed: usize,
        sheet_bytes: Vec<u8>,
        allocations: Option<Vec<etagere::Allocation>>,
    ) {
        let Some(allocations) = allocations else {
            error!(
                "Sprite atlas full - cannot allocate item sheet {}",
                sheet_index
            );
            return; // a later add can retry
        };

        self.pending_sheets.push(PendingSheetUpload {
            slots: allocations
                .iter()
                .enumerate()
                .map(|(chunk_index, allocation)| (allocation.rectangle, chunk_index))
                .collect(),
            bytes: sheet_bytes,
            consumed,
            chunks: meta.chunks.clone(),
        });

        self.loaded_sheets.insert(
            sheet_index,
            LoadedItemSheet {
                meta,
                allocations,
                ref_count: 1,
            },
        );
    }

    pub(crate) fn release_sheet(&mut self, sprite_id: u16) {
        let sheet_index = ((sprite_id - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count = sheet.ref_count.saturating_sub(1);
        }
    }

    /// Uploads every sheet staged since the last flush as one batched submit.
    pub fn flush_pending_uploads(&mut self, queue: &wgpu::Queue, atlas: &mut SpriteAtlas) {
        if self.pending_sheets.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_sheets);
        let mut uploads = Vec::new();
        for sheet in &pending {
            let slices =
                formats::sheets::chunk_pixel_slices(&sheet.bytes, sheet.consumed, &sheet.chunks, 1)
                    .expect("pixel blob validated when the sheet was staged");
            for (rect, chunk_index) in &sheet.slots {
                let chunk = sheet.chunks[*chunk_index];
                uploads.push(FrameUpload {
                    rect: *rect,
                    width: chunk.width as usize,
                    height: chunk.height as usize,
                    frames: vec![FrameRow {
                        x: 0,
                        y: 0,
                        width: chunk.width as usize,
                        height: chunk.height as usize,
                        data: slices[*chunk_index],
                    }],
                });
            }
        }
        let uploads = merge_uploads(uploads);
        atlas.upload_batch(queue, &uploads);
    }

    /// Frees every cached sheet with no live references, returning their atlas
    /// slots to etagere. Called when an allocation fails.
    pub(crate) fn evict_unused_sheets(&mut self, queue: &wgpu::Queue, atlas: &mut SpriteAtlas) {
        self.flush_pending_uploads(queue, atlas);
        let mut to_evict = Vec::new();
        for (index, sheet) in &self.loaded_sheets {
            if sheet.ref_count == 0 {
                to_evict.push(*index);
            }
        }
        if to_evict.is_empty() {
            return;
        }
        for index in &to_evict {
            if let Some(sheet) = self.loaded_sheets.remove(index) {
                for allocation in &sheet.allocations {
                    atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted = to_evict.len(),
            remaining = self.loaded_sheets.len(),
            "Evicted unused item sheets to make room in the atlas"
        );
    }
}

/// Build the GPU instance for an item at its current position/sprite.
///
/// Shared by `add_item` and `update_item` so the instance construction stays in
/// one place.
pub(crate) fn get_instance_for_frame(
    palette_table: &rangemap::RangeMap<u16, u16>,
    sheet: &LoadedItemSheet,
    item: &Item,
    frame_index: usize,
    rows: &PaletteRows,
) -> Option<Instance> {
    let frame = sheet.meta.frames.get(frame_index)?.as_ref()?;
    let allocation = sheet.allocations.get(frame.chunk as usize)?;
    let frame_w = frame.width as f32;
    let frame_h = frame.height as f32;

    let atlas_w = SPRITE_ATLAS_WIDTH as f32;
    let atlas_h = SPRITE_ATLAS_HEIGHT as f32;
    let world_pos = get_isometric_coordinate(item.x as f32, item.y as f32);

    let epf_w = sheet.meta.width as f32;
    let epf_h = sheet.meta.height as f32;

    let offset_x = -(epf_w / 2.0).floor() + frame.left as f32;
    let offset_y = -(epf_h / 2.0).floor() + frame.top as f32 - 2.0;

    let item_offset = Vec2::new(offset_x, offset_y);

    // Use spawn_order for z-ordering (set by network receive order).
    // Modulo ensures we stay within ITEM_Z_RANGE even if spawn_order exceeds bucket size.
    let item_order = item.spawn_order.min(ITEM_COUNT_BUCKET_SIZE as u8 - 1);
    let z_within_tile = (item_order as f32 / ITEM_COUNT_BUCKET_SIZE as f32) * ITEM_Z_RANGE;
    let z = calculate_tile_z(item.x as f32, item.y as f32, z_within_tile);

    let (tex_min, tex_max) = atlas_uv(
        Vec2::new(
            (allocation.rectangle.min.x + frame.x as i32) as f32,
            (allocation.rectangle.min.y + frame.y as i32) as f32,
        ),
        frame_w,
        frame_h,
        atlas_w,
        atlas_h,
    );

    Some(Instance::with_texture_atlas(
        (world_pos + item_offset).extend(z),
        tex_min,
        tex_max,
        Vec2::new(frame_w / 512., frame_h / 512.),
        {
            let palette_index = palette_table.get(&item.sprite).copied().unwrap_or_default();
            rows.row(rows.items, palette_index as u32)
        },
        -1.,
        false,
        false,
        InstanceFlag::None,
    ))
}
