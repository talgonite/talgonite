use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct Location {
    pub x: u16,
    pub y: u16,
}

impl TryFromBytes for Location {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let x = cursor.read_u16::<BigEndian>()?;
        let y = cursor.read_u16::<BigEndian>()?;
        Ok(Location { x, y })
    }
}
