use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum SpellType {
    #[default]
    None = 0,
    Prompt = 1,
    Targeted = 2,
    Prompt4Nums = 3,
    Prompt3Nums = 4,
    NoTarget = 5,
    Prompt2Nums = 6,
    Prompt1Num = 7,
}

#[derive(Debug, Clone)]
pub struct AddSpellToPane {
    pub slot: u8,
    pub sprite: u16,
    pub spell_type: SpellType,
    pub panel_name: String,
    pub prompt: String,
    pub cast_lines: u8,
}

impl TryFromBytes for AddSpellToPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let slot = cursor.read_u8()?;
        let sprite = cursor.read_u16::<BigEndian>()?;
        let spell_type = cursor.read_u8()?.try_into().unwrap_or_default();
        let panel_name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode panel_name: {}", e))?
        };
        let prompt = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode prompt: {}", e))?
        };
        let cast_lines = cursor.read_u8()?;
        Ok(AddSpellToPane {
            slot,
            sprite,
            spell_type,
            panel_name,
            prompt,
            cast_lines,
        })
    }
}
