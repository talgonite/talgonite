use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct CreatureTurn {
    pub source_id: u32,
    pub direction: u8,
}

impl TryFromBytes for CreatureTurn {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let source_id = cursor.read_u32::<BigEndian>()?;
        let direction = cursor.read_u8()?;

        Ok(CreatureTurn {
            source_id,
            direction,
        })
    }
}
