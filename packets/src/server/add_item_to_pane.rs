use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

const ITEM_SPRITE_OFFSET: u16 = 0x8000;

#[derive(Debug, Clone)]
pub struct AddItemToPane {
    pub slot: u8,
    pub sprite: u16,
    pub color: u8,
    pub name: String,
    pub count: u32,
    pub stackable: bool,
    pub max_durability: u32,
    pub current_durability: u32,
}

impl TryFromBytes for AddItemToPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let slot = cursor.read_u8()?;
        let sprite = cursor.read_u16::<BigEndian>()? - ITEM_SPRITE_OFFSET;
        let color = cursor.read_u8()?;
        let name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow::anyhow!("Failed to decode name: {}", e))?
        };
        let count = cursor.read_u32::<BigEndian>()?;
        let stackable = cursor.read_u8()? == 1;
        let max_durability = cursor.read_u32::<BigEndian>()?;
        let current_durability = cursor.read_u32::<BigEndian>()?;
        Ok(AddItemToPane {
            slot,
            sprite,
            color,
            name,
            count,
            stackable,
            max_durability,
            current_durability,
        })
    }
}
