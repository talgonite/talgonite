use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug)]
pub struct Effect {
    pub icon: u16,
    pub color: u8,
}

impl TryFromBytes for Effect {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let icon = cursor.read_u16::<BigEndian>()?;
        let color = cursor.read_u8()?;

        Ok(Effect { icon, color })
    }
}
