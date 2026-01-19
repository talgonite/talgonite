pub mod types;
pub use types::*;

use bincode::config::Configuration;
use etagere::AtlasAllocator;
use formats::epf::EpfImage;
use glam::Vec2;
use std::collections::HashMap;
use tracing::error;

use crate::{
    SharedInstanceBatch,
    instance::InstanceFlag,
    scene::{Instance, get_isometric_coordinate, texture_bind::TextureBind},
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
    pub(crate) instances: SharedInstanceBatch,
    pub(crate) item_order_counter: u32,
}

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
            bincode::serde::decode_from_slice(&palette_table_data, bincode::config::standard())
                .unwrap();

        let tb = TextureBind::default();
        let bind_group = tb.to_bind_group(
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
        if self.loaded_sheets.contains_key(&sheet_index) {
            return Ok(());
        }
        let path = format!("Legend/item{:03}.epf.bin", sheet_index);
        let bytes = archive.get_file(&path)?;
        let (epf, _) = bincode::decode_from_slice::<EpfImage, Configuration>(
            &bytes,
            bincode::config::standard(),
        )?;
        let mut allocations: Vec<Option<etagere::Allocation>> =
            Vec::with_capacity(epf.frames.len());
        allocations.resize(epf.frames.len(), None);
        self.loaded_sheets
            .insert(sheet_index, LoadedItemSheet { epf, allocations });
        Ok(())
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

impl ItemBatch {
    pub fn new(device: &wgpu::Device, store: &ItemAssetStore) -> Self {
        let vertices = crate::make_quad(512, 512).to_vec();
        let batch = SharedInstanceBatch::new(device, vertices, store.bind_group.clone());
        Self {
            instances: batch,
            item_order_counter: 1,
        }
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
                let mut data: Vec<u8> = vec![0; w * h];
                data.copy_from_slice(&frame.data);

                let texture = &store.diffuse;
                for row in 0..h {
                    let start = row * w;
                    let end = start + w;
                    let row_data = &data[start..end];
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: allocation.rectangle.min.x as u32,
                                y: allocation.rectangle.min.y as u32 + row as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        row_data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w as u32),
                            rows_per_image: Some(1),
                        },
                        wgpu::Extent3d {
                            width: w as u32,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                sheet.allocations[frame_index] = Some(allocation);
            } else {
                error!("Item atlas full - cannot allocate sprite {}", item.sprite);
                return None;
            }
        }
        let allocation = sheet.allocations[frame_index].as_ref()?;
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

        let z = -0.5 + self.item_order_counter as f32 * 0.000001;

        let instance = Instance::with_texture_atlas(
            (world_pos + item_offset).extend(z),
            Vec2::new(
                allocation.rectangle.min.x as f32 / atlas_w,
                allocation.rectangle.min.y as f32 / atlas_h,
            ),
            Vec2::new(
                (allocation.rectangle.min.x as f32 + frame_w) / atlas_w,
                (allocation.rectangle.min.y as f32 + frame_h) / atlas_h,
            ),
            Vec2::new(frame_w / 512., frame_h / 512.),
            store
                .palette_table
                .get(&item.sprite)
                .copied()
                .unwrap_or_default() as f32
                / 256.,
            -1.,
            false,
            false,
            InstanceFlag::None,
        );

        self.item_order_counter = self.item_order_counter.wrapping_add(1);

        let idx = self.instances.add(queue, instance)?;
        Some(ItemInstanceHandle(idx))
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
        let Some(allocation) = sheet.allocations.get(frame_index).and_then(|a| a.as_ref()) else {
            return;
        };

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

        let z = 0.00001;

        let instance = Instance::with_texture_atlas(
            (world_pos + item_offset).extend(z),
            Vec2::new(
                allocation.rectangle.min.x as f32 / atlas_w,
                allocation.rectangle.min.y as f32 / atlas_h,
            ),
            Vec2::new(
                (allocation.rectangle.min.x as f32 + frame_w) / atlas_w,
                (allocation.rectangle.min.y as f32 + frame_h) / atlas_h,
            ),
            Vec2::new(frame_w / 512., frame_h / 512.),
            item.color as f32 / 256.,
            -1.,
            false,
            false,
            InstanceFlag::None,
        );

        self.instances.update(queue, handle.0, instance);
    }

    pub fn remove_item(&self, queue: &wgpu::Queue, handle: ItemInstanceHandle) {
        self.instances.remove(queue, handle.0);
    }
}
