//! One shared R8 sprite atlas + palette texture for all indexed sprite
//! classes (players, creatures, items).
//!
//! The three classes already share a shader, instance format, and bind-group
//! layout, so they can also share a single diffuse atlas (palette indices)
//! and a single palette texture. Each class rebases its palette row into the
//! stacked palette texture; the dye table stays separate (it is sampled from
//! its own binding).

use etagere::{AllocId, Allocation};
use formats::game_files::SquashfsArchive;
use rustc_hash::FxHashMap;

use crate::scene::texture_atlas::{FrameUpload, TextureAtlas};
use crate::scene::texture_bind::TextureBind;
use crate::texture;

// 8192x8192 R8 (64 MiB) gives the shared atlas roughly 4x the room of the old
// per-class split (players 32 MiB + creatures 8 MiB + items 4 MiB). The device
// inherits the adapter's texture-size limit (Slint applies
// `using_resolution(adapter.limits())`), so 8192 is available on any adapter
// that already supported the previous 4096x8192 player atlas.
pub const SPRITE_ATLAS_WIDTH: usize = 8192;
pub const SPRITE_ATLAS_HEIGHT: usize = 8192;

pub(crate) const PLAYER_PALETTE_CHARS: [char; 9] = ['b', 'c', 'e', 'f', 'h', 'l', 'm', 'u', 'w'];

/// Base rows into the shared palette texture (one 256-color row per palette).
/// Instance `palette_offset` is `(base + index) / height` (a v coordinate into
/// the 256-wide texture).
pub struct PaletteRows {
    /// Player palette base rows, keyed by khanpal letter.
    pub players: FxHashMap<char, u32>,
    /// Base row for creature palettes (`hades/mns.ktx2`).
    pub creatures: u32,
    /// Base row for item palettes (`Legend/item.ktx2`).
    pub items: u32,
    /// Total number of palette rows in the shared texture.
    pub height: u32,
}

impl PaletteRows {
    /// The v coordinate for `base + index`, centered on the texel. Centering
    /// matters: the stacked texture's height is not a power of two, so
    /// sampling exactly on a row boundary can round into the neighbouring
    /// palette (off-by-one colors, or invisible when the wrong row is
    /// transparent).
    pub fn row(&self, base: u32, index: u32) -> f32 {
        (base + index) as f32 / self.height as f32 + 0.5 / self.height as f32
    }
}

pub struct SpriteAtlas {
    atlas: TextureAtlas,
    #[allow(unused)]
    palette_texture: texture::Texture,
    #[allow(unused)]
    dye_texture: texture::Texture,
    bind_group: wgpu::BindGroup,
    palette_rows: PaletteRows,
}

impl SpriteAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, archive: &SquashfsArchive) -> Self {
        // Stack every class's palette files into one 256-wide texture: player
        // khanpal palettes first, then creature mns rows, then item rows.
        let mut palette_data = Vec::new();
        let mut players = FxHashMap::default();
        let mut total_rows = 0u32;
        for letter in PLAYER_PALETTE_CHARS {
            let path = format!("khanpal/pal{}.ktx2", letter);
            let bytes = archive.get_file_or_panic(&path);
            let (w, h, data) = texture::Texture::load_ktx2(&bytes).unwrap();
            debug_assert_eq!(w, 256);
            players.insert(letter, total_rows);
            total_rows += h;
            palette_data.extend_from_slice(&data);
        }

        let creatures = total_rows;
        let creature_bytes = archive.get_file_or_panic("hades/mns.ktx2");
        let (w, h, data) = texture::Texture::load_ktx2(&creature_bytes).unwrap();
        debug_assert_eq!((w, h), (256, 256));
        total_rows += h;
        palette_data.extend_from_slice(&data);

        let items = total_rows;
        let item_bytes = archive.get_file_or_panic("Legend/item.ktx2");
        let (w, h, data) = texture::Texture::load_ktx2(&item_bytes).unwrap();
        debug_assert_eq!((w, h), (256, 256));
        total_rows += h;
        palette_data.extend_from_slice(&data);

        let palette_texture = texture::Texture::from_data(
            device,
            queue,
            "sprite_palette",
            256,
            total_rows,
            wgpu::TextureFormat::Rgba8Unorm,
            &palette_data,
        )
        .unwrap();

        let dye_bytes = archive.get_file_or_panic("Legend/color0.ktx2");
        let dye_texture =
            texture::Texture::from_ktx2_rgba8(device, queue, "sprite_dye", &dye_bytes).unwrap();

        let diffuse = texture::Texture::from_data(
            device,
            queue,
            "sprite_atlas",
            SPRITE_ATLAS_WIDTH as u32,
            SPRITE_ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::R8Unorm,
            &vec![0; SPRITE_ATLAS_WIDTH * SPRITE_ATLAS_HEIGHT],
        )
        .unwrap();

        let bind_group =
            TextureBind::to_bind_group(device, &diffuse, &palette_texture, &dye_texture.view);
        let atlas = TextureAtlas::new(device, diffuse.texture);

        Self {
            atlas,
            palette_texture,
            dye_texture,
            bind_group,
            palette_rows: PaletteRows {
                players,
                creatures,
                items,
                height: total_rows,
            },
        }
    }

    pub fn allocate_slot(&mut self, width: usize, height: usize) -> Option<Allocation> {
        self.atlas.allocate_slot(width, height)
    }

    pub fn deallocate(&mut self, id: AllocId) {
        self.atlas.atlas.deallocate(id);
    }

    pub fn upload_batch(&mut self, queue: &wgpu::Queue, uploads: &[FrameUpload]) {
        self.atlas.upload_batch(queue, uploads);
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn palette_rows(&self) -> &PaletteRows {
        &self.palette_rows
    }
}
