use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::Read;

#[derive(Debug, Clone)]
pub struct AddSkillToPane {
    pub slot: u8,
    pub sprite: u16,
    pub name: String,
}

impl TryFromBytes for AddSkillToPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let slot = cursor.read_u8()?;
        let sprite = cursor.read_u16::<byteorder::BigEndian>()?;
        let name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode name: {}", e))?
        };
        Ok(AddSkillToPane { slot, sprite, name })
    }
}
