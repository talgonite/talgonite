use etagere::{AllocId, Allocation};

pub struct TextureAtlas {
    pub atlas: etagere::AtlasAllocator,
    texture: wgpu::Texture,
    bytes_per_pixel: u32,
}

pub struct FrameUpload<'a> {
    pub width: usize,
    pub height: usize,
    pub data: &'a [u8],
}

impl TextureAtlas {
    pub fn new(texture: wgpu::Texture) -> Self {
        Self {
            atlas: etagere::AtlasAllocator::new(etagere::size2(
                texture.width() as i32,
                texture.height() as i32,
            )),
            bytes_per_pixel: texture.format().block_copy_size(None).unwrap_or_default(),
            texture,
        }
    }

    pub fn allocate(
        &mut self,
        queue: &wgpu::Queue,
        width: usize,
        height: usize,
        data: &[u8],
    ) -> Option<Allocation> {
        if let Some(allocation) = self
            .atlas
            .allocate(etagere::size2(width as i32, height as i32))
        {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: allocation.rectangle.min.x as u32,
                        y: allocation.rectangle.min.y as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_pixel * width as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: width as u32,
                    height: height as u32,
                    depth_or_array_layers: 1,
                },
            );

            Some(allocation)
        } else {
            None
        }
    }

    pub fn deallocate(&mut self, id: AllocId) {
        self.atlas.deallocate(id);
    }
}
