use super::types::{Gender, PlayerPieceType, PlayerSpriteKey};
use crate::texture;
use formats::game_files::ArxArchive;
use rangemap::RangeMap;
use rustc_hash::FxHashMap;

type Archive = ArxArchive;

const PALETTE_CHARS: [char; 11] = ['b', 'c', 'e', 'f', 'h', 'i', 'l', 'm', 'p', 'u', 'w'];

#[derive(Debug, Clone)]
pub struct PaletteLookup {
    base: Option<RangeMap<u16, u16>>,
    male: Option<RangeMap<u16, u16>>,
    female: Option<RangeMap<u16, u16>>,
}

impl PaletteLookup {
    pub fn new(
        base: Option<RangeMap<u16, u16>>,
        male: Option<RangeMap<u16, u16>>,
        female: Option<RangeMap<u16, u16>>,
    ) -> Self {
        Self { base, male, female }
    }

    pub fn get_palette(&self, gender: Gender, id: u16) -> Option<u16> {
        let gendered_override = match gender {
            Gender::Male => self.male.as_ref()?.get(&id),
            Gender::Female => self.female.as_ref()?.get(&id),
            _ => None,
        };

        match gendered_override {
            Some(p) => Some(*p),
            None => self.base.as_ref()?.get(&id).copied(),
        }
    }
}

pub struct PlayerPalettes {
    info: FxHashMap<char, u32>,
    table: FxHashMap<char, PaletteLookup>,
    count: u32,
}

impl PlayerPalettes {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> (Self, texture::Texture, texture::Texture) {
        let (palette_texture, dye_texture, info) = Self::load_palettes(archive, device, queue);
        let table = Self::load_palette_table(archive);

        (
            Self {
                info,
                table,
                count: palette_texture.texture.height(),
            },
            palette_texture,
            dye_texture,
        )
    }

    fn load_palette_table(archive: &Archive) -> FxHashMap<char, PaletteLookup> {
        let mut palette_table = FxHashMap::default();

        fn load(archive: &Archive, path: &str) -> anyhow::Result<RangeMap<u16, u16>> {
            let data = archive.get_file(path)?;
            let (base_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
                bincode::serde::decode_from_slice(&data, bincode::config::standard())?;
            Ok(base_palette_table)
        }

        for letter in PALETTE_CHARS.iter() {
            palette_table.insert(
                *letter,
                PaletteLookup::new(
                    load(archive, &format!("khanpal/pal{}.tbl.bin", letter)).ok(),
                    load(archive, &format!("khanpal/pal{}_m.tbl.bin", letter)).ok(),
                    load(archive, &format!("khanpal/pal{}_f.tbl.bin", letter)).ok(),
                ),
            );
        }

        palette_table
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_palettes(
        archive: &Archive,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (texture::Texture, texture::Texture, FxHashMap<char, u32>) {
        let mut data = Vec::new();
        let mut range_map = FxHashMap::default();
        let mut total_h = 0;

        let dye_data = archive.get_file_or_panic("Legend/color0.ktx2");

        let dye_palette =
            texture::Texture::from_ktx2_rgba8(device, queue, "dye_data", &dye_data).unwrap();

        debug_assert_eq!(dye_palette.texture.width(), 256);

        for letter in PALETTE_CHARS.iter() {
            let path = format!("khanpal/pal{}.ktx2", letter);
            let bytes = archive.get_file_or_panic(&path);

            let (w, h, palette_data) = texture::Texture::load_ktx2(&bytes).unwrap();

            range_map.insert(*letter, total_h);

            debug_assert_eq!(w, 256);
            total_h += h;

            data.extend_from_slice(&palette_data);
        }

        (
            texture::Texture::from_data(
                device,
                queue,
                "dye_palette",
                256,
                total_h,
                wgpu::TextureFormat::Rgba8Unorm,
                &data,
            )
            .unwrap(),
            dye_palette,
            range_map,
        )
    }

    pub fn get_palette_params(&self, key: &PlayerSpriteKey, dye_color: u8) -> (f32, f32) {
        let palette_prefix = key.prefix_for_palette(key.sprite_id);
        let palette_y = *self.info.get(&palette_prefix).unwrap_or(&0);

        let palette_index = if matches!(key.slot, PlayerPieceType::Body | PlayerPieceType::Face | PlayerPieceType::Emote) {
            dye_color as u16
        } else {
            match self.table.get(&key.slot.prefix(key.sprite_id)) {
                Some(lookup) => lookup.get_palette(key.gender, key.sprite_id).unwrap_or(0),
                _ => 0,
            }
        };

        let v_coord = ((palette_y + palette_index as u32) as f32 + 0.5) / self.count as f32;
        let dye_param = if matches!(key.slot, PlayerPieceType::Body | PlayerPieceType::Face | PlayerPieceType::Emote) {
            -1.
        } else {
            dye_color as f32 / 256.
        };

        (v_coord, dye_param)
    }
}
