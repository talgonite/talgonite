use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub enum CooldownType {
    Skill,
    Item,
}

#[derive(Debug, Clone)]
pub struct Cooldown {
    pub kind: CooldownType,
    pub slot: u8,
    pub cooldown_secs: u32,
}

impl TryFromBytes for Cooldown {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let is_skill = cursor.read_u8()? == 1;
        let slot = cursor.read_u8()?;
        let cooldown_secs = cursor.read_u32::<BigEndian>()?;

        Ok(Cooldown {
            kind: if is_skill {
                CooldownType::Skill
            } else {
                CooldownType::Item
            },
            slot,
            cooldown_secs,
        })
    }
}
