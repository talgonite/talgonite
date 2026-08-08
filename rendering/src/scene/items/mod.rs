pub mod types;
pub use types::*;

use formats::sheets::ItemSheet;
use bevy_math::Vec2;
use std::collections::HashMap;
use tracing::error;

use crate::{
    instance::InstanceFlag,
    scene::{
        Instance, Z_ITEMS, get_isometric_coordinate,
        sprite::SpriteBatch,
        texture_atlas::{FrameRow, FrameUpload, TextureAtlas, merge_uploads},
        texture_bind::TextureBind,
        utils::{atlas_uv, calculate_tile_z},
    },
    texture,
};

pub const ITEM_ATLAS_WIDTH: usize = 1024;
pub const ITEM_ATLAS_HEIGHT: usize = 1024;
pub const ITEMS_PER_EPF_FILE: u32 = 266;

pub struct ItemAssetStore {
    pub(crate) atlas: TextureAtlas,
    pub(crate) loaded_sheets: HashMap<u32, LoadedItemSheet>,
    pub(crate) bind_group: wgpu::BindGroup,
    palette_table: rangemap::RangeMap<u16, u16>,
    /// Frames staged this frame but not yet uploaded; flushed once per frame
    /// so a burst of item spawns shares a single staging belt submit.
    pending_uploads: Vec<FrameUpload>,
}

pub struct ItemBatch {
    batch: SpriteBatch<u16>,
}

pub const ITEM_Z_RANGE: f32 = Z_ITEMS;
/// Item z-order is based on item.id % this value for deterministic ordering
const ITEM_COUNT_BUCKET_SIZE: u32 = 20;

impl ItemAssetStore {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        archive: &formats::game_files::SquashfsArchive,
    ) -> Self {
        let diffuse = texture::Texture::from_data(
            device,
            queue,
            "item_atlas",
            ITEM_ATLAS_WIDTH as u32,
            ITEM_ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::R8Unorm,
            &vec![0; ITEM_ATLAS_WIDTH * ITEM_ATLAS_HEIGHT],
        )
        .unwrap();

        let palette_data = archive.get_file_or_panic("Legend/item.ktx2");
        let palette =
            texture::Texture::from_ktx2_rgba8(device, queue, "item_palette", &palette_data)
                .unwrap();

        let palette_table_data = archive
            .get_file("Legend/item.tbl.bin")
            .expect("item palette table missing");
        let (palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            oxicode::serde::decode_from_slice(&palette_table_data, oxicode::config::standard())
                .unwrap();

        let bind_group = TextureBind::to_bind_group(
            device,
            &diffuse,
            &palette,
            &texture::Texture::empty_view(device, "item_empty"),
        );

        Self {
            atlas: TextureAtlas::new(device, diffuse.texture.clone()),
            loaded_sheets: HashMap::new(),
            bind_group,
            palette_table,
            pending_uploads: Vec::new(),
        }
    }

    pub(crate) fn ensure_sheet(
        &mut self,
        queue: &wgpu::Queue,
        archive: &formats::game_files::SquashfsArchive,
        sheet_index: u32,
    ) -> anyhow::Result<()> {
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count += 1;
            return Ok(());
        }
        let base = format!("Legend/item{:03}", sheet_index);
        let meta_bytes = archive.get_file(&format!("{base}.sheet.bin"))?;
        let (meta, _) = oxicode::decode_from_slice::<ItemSheet>(&meta_bytes)?;

        let image_paths: Vec<String> = (0..meta.chunks.len())
            .map(|chunk| format!("{base}.sheet{chunk}.ktx2"))
            .collect();
        let image_results = archive.get_files_parallel(&image_paths);
        let mut images = Vec::with_capacity(meta.chunks.len());
        for result in image_results {
            let bytes = result?;
            let (_, _, pixels) = texture::Texture::load_ktx2(&bytes)?;
            images.push(pixels);
        }

        // Allocate one atlas slot per chunk; evict unused sheets if full.
        let mut allocations: Vec<etagere::Allocation> = Vec::with_capacity(meta.chunks.len());
        for chunk in &meta.chunks {
            let mut allocation = self
                .atlas
                .allocate_slot(chunk.width as usize, chunk.height as usize);
            if allocation.is_none() {
                self.evict_unused_sheets(queue);
                allocation = self
                    .atlas
                    .allocate_slot(chunk.width as usize, chunk.height as usize);
            }
            let Some(allocation) = allocation else {
                error!("Item atlas full - cannot allocate sheet {}", sheet_index);
                for slot in &allocations {
                    self.atlas.atlas.deallocate(slot.id);
                }
                return Ok(()); // a later add can retry
            };
            allocations.push(allocation);
        }

        for (chunk_index, image) in images.into_iter().enumerate() {
            let chunk = meta.chunks[chunk_index];
            self.pending_uploads.push(FrameUpload {
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

        self.loaded_sheets.insert(
            sheet_index,
            LoadedItemSheet {
                meta,
                allocations,
                ref_count: 1,
            },
        );
        Ok(())
    }

    pub(crate) fn release_sheet(&mut self, sprite_id: u16) {
        let sheet_index = ((sprite_id - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count = sheet.ref_count.saturating_sub(1);
        }
    }

    /// Uploads every frame staged since the last flush as one batched submit.
    pub fn flush_pending_uploads(&mut self, queue: &wgpu::Queue) {
        if self.pending_uploads.is_empty() {
            return;
        }
        let uploads = merge_uploads(std::mem::take(&mut self.pending_uploads));
        self.atlas.upload_batch(queue, &uploads);
    }

    /// Frees every cached sheet with no live references, returning their atlas
    /// slots to etagere. Called when an allocation fails.
    fn evict_unused_sheets(&mut self, queue: &wgpu::Queue) {
        self.flush_pending_uploads(queue);
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
                    self.atlas.atlas.deallocate(allocation.id);
                }
            }
        }
        tracing::info!(
            evicted = to_evict.len(),
            remaining = self.loaded_sheets.len(),
            "Evicted unused item sheets to make room in the atlas"
        );
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

impl ItemBatch {
    pub fn new(device: &wgpu::Device, store: &ItemAssetStore) -> Self {
        let vertices = crate::make_quad(512, 512).to_vec();
        Self {
            batch: SpriteBatch::new(device, vertices, store.bind_group.clone()),
        }
    }

    /// Clear all item instances.
    pub fn clear(&self) {
        self.batch.clear();
    }

    pub fn clear_and_unload(&self, store: &mut ItemAssetStore) {
        self.batch
            .clear_and_unload(|sprite_id| store.release_sheet(*sprite_id));
    }

    pub fn add_item(
        &mut self,
        queue: &wgpu::Queue,
        store: &mut ItemAssetStore,
        archive: &formats::game_files::SquashfsArchive,
        item: Item,
    ) -> Option<ItemInstanceHandle> {
        let sheet_index = ((item.sprite - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        let frame_index = ((item.sprite - 1) as u32 % ITEMS_PER_EPF_FILE) as usize;
        if store.ensure_sheet(queue, archive, sheet_index).is_err() {
            return None;
        }

        // Bounds-check the frame index before touching `allocations`.
        let sheet = store.loaded_sheets.get_mut(&sheet_index)?;
        if frame_index >= sheet.meta.frames.len() {
            return None;
        }

        let instance = get_instance_for_frame(&store.palette_table, sheet, &item, frame_index)?;

        let idx = self.batch.add_instance(queue, instance)?;
        let handle = ItemInstanceHandle {
            index: idx,
            sprite_id: item.sprite,
        };
        self.batch.insert_handle(handle.index, handle.sprite_id);
        Some(handle)
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.batch.draw(render_pass);
    }

    pub fn update_item(
        &self,
        queue: &wgpu::Queue,
        store: &ItemAssetStore,
        handle: &ItemInstanceHandle,
        item: Item,
    ) {
        let sheet_index = ((item.sprite - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        let frame_index = ((item.sprite - 1) as u32 % ITEMS_PER_EPF_FILE) as usize;

        let Some(sheet) = store.loaded_sheets.get(&sheet_index) else {
            return;
        };

        let Some(instance) =
            get_instance_for_frame(&store.palette_table, sheet, &item, frame_index)
        else {
            return;
        };

        self.batch.update_instance(queue, handle.index, instance);
    }

    pub fn remove_item(
        &self,
        queue: &wgpu::Queue,
        store: &mut ItemAssetStore,
        handle: ItemInstanceHandle,
    ) {
        self.batch.remove_instance(queue, handle.index);
        store.release_sheet(handle.sprite_id);
    }
}

/// Build the GPU instance for an item at its current position/sprite.
///
/// Shared by `add_item` and `update_item` so the instance construction stays in
/// one place.
fn get_instance_for_frame(
    palette_table: &rangemap::RangeMap<u16, u16>,
    sheet: &LoadedItemSheet,
    item: &Item,
    frame_index: usize,
) -> Option<Instance> {
    let frame = sheet.meta.frames.get(frame_index)?.as_ref()?;
    let allocation = sheet.allocations.get(frame.chunk as usize)?;
    let frame_w = frame.width as f32;
    let frame_h = frame.height as f32;

    let atlas_w = ITEM_ATLAS_WIDTH as f32;
    let atlas_h = ITEM_ATLAS_HEIGHT as f32;
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
        (palette_table.get(&item.sprite).copied().unwrap_or_default() as f32 + 0.5) / 256.,
        -1.,
        false,
        false,
        InstanceFlag::None,
    ))
}
