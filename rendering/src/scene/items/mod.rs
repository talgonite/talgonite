pub mod types;
pub use types::*;

use etagere::AtlasAllocator;
use formats::epf::EpfImage;
use glam::Vec2;
use std::collections::HashMap;
use tracing::error;

use crate::{
    instance::InstanceFlag,
    scene::{
        Instance, Z_ITEMS, get_isometric_coordinate,
        sprite::SpriteBatch,
        texture_bind::TextureBind,
        utils::{atlas_uv, calculate_tile_z},
    },
    texture,
};

pub const ITEM_ATLAS_WIDTH: usize = 1024;
pub const ITEM_ATLAS_HEIGHT: usize = 1024;
pub const ITEMS_PER_EPF_FILE: u32 = 266;

pub struct ItemAssetStore {
    pub(crate) allocation_atlas: AtlasAllocator,
    pub(crate) diffuse: texture::Texture,
    pub(crate) loaded_sheets: HashMap<u32, LoadedItemSheet>,
    pub(crate) bind_group: wgpu::BindGroup,
    palette_table: rangemap::RangeMap<u16, u16>,
}

pub struct ItemBatch {
    batch: SpriteBatch<u16>,
}

pub const ITEM_Z_RANGE: f32 = Z_ITEMS;
/// Item z-order is based on item.id % this value for deterministic ordering
const ITEM_COUNT_BUCKET_SIZE: u32 = 20;

impl ItemAssetStore {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        archive: &formats::game_files::ArxArchive,
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
            allocation_atlas: AtlasAllocator::new(etagere::size2(
                ITEM_ATLAS_WIDTH as i32,
                ITEM_ATLAS_HEIGHT as i32,
            )),
            diffuse,
            loaded_sheets: HashMap::new(),
            bind_group,
            palette_table,
        }
    }

    pub(crate) fn ensure_sheet(
        &mut self,
        archive: &formats::game_files::ArxArchive,
        sheet_index: u32,
    ) -> anyhow::Result<()> {
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count += 1;
            return Ok(());
        }
        let path = format!("Legend/item{:03}.epf.bin", sheet_index);
        let bytes = archive.get_file(&path)?;
        let (epf, _) = oxicode::decode_from_slice::<EpfImage>(&bytes)?;
        let mut allocations: Vec<Option<etagere::Allocation>> =
            Vec::with_capacity(epf.frames.len());
        allocations.resize(epf.frames.len(), None);
        self.loaded_sheets.insert(
            sheet_index,
            LoadedItemSheet {
                epf,
                allocations,
                ref_count: 1,
            },
        );
        Ok(())
    }

    pub(crate) fn unload_sprite(&mut self, sprite_id: u16) {
        let sheet_index = ((sprite_id - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        if let Some(sheet) = self.loaded_sheets.get_mut(&sheet_index) {
            sheet.ref_count -= 1;
            if sheet.ref_count == 0 {
                for allocation in &sheet.allocations {
                    if let Some(allocation) = allocation {
                        self.allocation_atlas.deallocate(allocation.id);
                    }
                }
                self.loaded_sheets.remove(&sheet_index);
            }
        }
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
            .clear_and_unload(|sprite_id| store.unload_sprite(*sprite_id));
    }

    pub fn add_item(
        &mut self,
        queue: &wgpu::Queue,
        store: &mut ItemAssetStore,
        archive: &formats::game_files::ArxArchive,
        item: Item,
    ) -> Option<ItemInstanceHandle> {
        let sheet_index = ((item.sprite - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        let frame_index = ((item.sprite - 1) as u32 % ITEMS_PER_EPF_FILE) as usize;
        if store.ensure_sheet(archive, sheet_index).is_err() {
            return None;
        }
        let sheet = store.loaded_sheets.get_mut(&sheet_index)?;
        if frame_index >= sheet.epf.frames.len() {
            return None;
        }
        if sheet.allocations[frame_index].is_none() {
            let frame = &sheet.epf.frames[frame_index];
            let w = (frame.right - frame.left) as usize;
            let h = (frame.bottom - frame.top) as usize;
            if let Some(allocation) = store
                .allocation_atlas
                .allocate(etagere::size2(w as i32, h as i32))
            {
                let texture = &store.diffuse;
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: allocation.rectangle.min.x as u32,
                            y: allocation.rectangle.min.y as u32,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &frame.data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(w as u32),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width: w as u32,
                        height: h as u32,
                        depth_or_array_layers: 1,
                    },
                );
                sheet.allocations[frame_index] = Some(allocation);
            } else {
                error!("Item atlas full - cannot allocate sprite {}", item.sprite);
                return None;
            }
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
        store.unload_sprite(handle.sprite_id);
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
    let allocation = sheet.allocations.get(frame_index)?.as_ref()?;
    let frame = &sheet.epf.frames[frame_index];
    let frame_w = (frame.right - frame.left) as f32;
    let frame_h = (frame.bottom - frame.top) as f32;

    let atlas_w = ITEM_ATLAS_WIDTH as f32;
    let atlas_h = ITEM_ATLAS_HEIGHT as f32;
    let world_pos = get_isometric_coordinate(item.x as f32, item.y as f32);

    let epf_w = sheet.epf.width as f32;
    let epf_h = sheet.epf.height as f32;

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
            allocation.rectangle.min.x as f32,
            allocation.rectangle.min.y as f32,
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
