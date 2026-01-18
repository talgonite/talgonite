use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct CreatureWalk {
    pub source_id: u32,
    pub old_point: (u16, u16),
    pub direction: u8,
}

impl TryFromBytes for CreatureWalk {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let source_id = cursor.read_u32::<BigEndian>()?;
        let old_point = {
            let x = cursor.read_u16::<BigEndian>()?;
            let y = cursor.read_u16::<BigEndian>()?;
            (x, y)
        };
        let direction = cursor.read_u8()?;

        Ok(CreatureWalk {
            source_id,
            old_point,
            direction,
        })
    }
}
