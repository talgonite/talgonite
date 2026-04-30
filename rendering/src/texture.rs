use anyhow::*;
use wgpu;

pub struct Texture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn load_ktx2<'a>(bytes: &'a [u8]) -> Result<(u32, u32, Vec<u8>)> {
        let reader = ktx2::Reader::new(bytes).unwrap();
        let info = reader.header();
        let buf = reader.levels().next().unwrap();
        Ok((info.pixel_width, info.pixel_height, buf.data.to_vec()))
    }

    pub fn from_ktx2_r8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        let reader = ktx2::Reader::new(bytes).unwrap();
        let info = reader.header();
        let buf = reader.levels().next().unwrap();

        Self::from_data(
            &device,
            &queue,
            label,
            info.pixel_width,
            info.pixel_height,
            wgpu::TextureFormat::R8Unorm,
            &buf.data,
        )
    }

    pub fn from_ktx2_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        let reader = ktx2::Reader::new(bytes).unwrap();
        let info = reader.header();
        let buf = reader.levels().next().unwrap();

        Self::from_data(
            &device,
            &queue,
            label,
            info.pixel_width,
            info.pixel_height,
            wgpu::TextureFormat::Rgba8Unorm,
            &buf.data,
        )
    }

    pub fn from_data(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> Result<Self> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(format.components() as u32 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{}_view", label)),
            ..Default::default()
        });

        // TODO: Not everything needs a sampler to be created here
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{}_view", label)),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::GreaterEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn create_render_texture(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{}_view", label)),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn empty_view(device: &wgpu::Device, label: &str) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{}_view", label)),
            ..Default::default()
        })
    }
}
