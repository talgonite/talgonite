use crate::{TryFromBytes, types::Direction};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct EntityTurn {
    pub source_id: u32,
    pub direction: Direction,
}

impl TryFromBytes for EntityTurn {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let source_id = cursor.read_u32::<BigEndian>()?;
        let direction = Direction::try_from(cursor.read_u8()?)?;

        Ok(EntityTurn {
            source_id,
            direction,
        })
    }
}
