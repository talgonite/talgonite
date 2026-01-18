use crate::TryFromBytes;
use crate::types::EquipmentSlot;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

pub const ITEM_SPRITE_OFFSET: u16 = 0x8000; // matches server NETWORKING_CONSTANTS.ITEM_SPRITE_OFFSET

#[derive(Debug, Clone)]
pub struct Equipment {
    pub slot: EquipmentSlot,
    pub sprite: u16,
    pub color: u8,
    pub name: String,
    pub max_durability: u32,
    pub current_durability: u32,
}

impl TryFromBytes for Equipment {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let raw_slot = cursor.read_u8()?;
        let raw_sprite = cursor.read_u16::<BigEndian>()?;
        let color = cursor.read_u8()?;
        // String8: length byte then bytes
        let name_len = cursor.read_u8()? as usize;
        let mut name_buf = vec![0u8; name_len];
        cursor.read_exact(&mut name_buf)?;
        let name = WINDOWS_949
            .decode(&name_buf, DecoderTrap::Replace)
            .map_err(|e| anyhow!("Failed to decode name: {}", e))?;
        // Unknown padding byte after name per server converter
        let _unknown = cursor.read_u8()?;
        let max_durability = cursor.read_u32::<BigEndian>()?;
        let current_durability = cursor.read_u32::<BigEndian>()?;

        Ok(Equipment {
            slot: raw_slot.try_into()?,
            sprite: raw_sprite
                .checked_sub(ITEM_SPRITE_OFFSET)
                .ok_or_else(|| anyhow!("Invalid sprite offset: {}", raw_sprite))?,
            color,
            name,
            max_durability,
            current_durability,
        })
    }
}
