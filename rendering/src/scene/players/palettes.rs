use super::types::{Gender, PlayerPieceType, PlayerSpriteKey};
use crate::scene::sprite_atlas::PaletteRows;
use formats::game_files::SquashfsArchive;
use rangemap::RangeMap;
use rustc_hash::FxHashMap;

type Archive = SquashfsArchive;

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
            Gender::Male => self.male.as_ref().and_then(|m| m.get(&id).copied()),
            Gender::Female => self.female.as_ref().and_then(|f| f.get(&id).copied()),
            _ => None,
        };

        gendered_override.or_else(|| self.base.as_ref()?.get(&id).copied())
    }
}

pub struct PlayerPalettes {
    table: FxHashMap<char, PaletteLookup>,
}

impl PlayerPalettes {
    pub fn new(archive: &Archive) -> Self {
        let mut palette_table = FxHashMap::default();

        fn load(archive: &Archive, path: &str) -> anyhow::Result<RangeMap<u16, u16>> {
            let data = archive.get_file(path)?;
            let (base_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
                oxicode::serde::decode_from_slice(&data, oxicode::config::standard())?;
            Ok(base_palette_table)
        }

        for letter in crate::scene::sprite_atlas::PLAYER_PALETTE_CHARS.iter() {
            palette_table.insert(
                *letter,
                PaletteLookup::new(
                    load(archive, &format!("khanpal/pal{}.tbl.bin", letter)).ok(),
                    load(archive, &format!("khanpal/pal{}_m.tbl.bin", letter)).ok(),
                    load(archive, &format!("khanpal/pal{}_f.tbl.bin", letter)).ok(),
                ),
            );
        }

        Self {
            table: palette_table,
        }
    }

    pub fn get_palette_params(
        &self,
        key: &PlayerSpriteKey,
        dye_color: u8,
        rows: &PaletteRows,
    ) -> (f32, f32) {
        let palette_prefix = key.prefix_for_palette(key.sprite_id);
        let palette_y = rows.players.get(&palette_prefix).copied().unwrap_or(0);

        let palette_index = if matches!(
            key.slot,
            PlayerPieceType::Body | PlayerPieceType::Face | PlayerPieceType::Emote
        ) {
            dye_color as u16
        } else {
            match self.table.get(&key.slot.prefix(key.sprite_id)) {
                Some(lookup) => lookup.get_palette(key.gender, key.sprite_id).unwrap_or(0),
                _ => 0,
            }
        };

        let v_coord = rows.row(palette_y, palette_index as u32);
        let dye_param = if matches!(
            key.slot,
            PlayerPieceType::Body | PlayerPieceType::Face | PlayerPieceType::Emote
        ) {
            -1.
        } else {
            dye_color as f32 / 256.
        };

        (v_coord, dye_param)
    }
}
