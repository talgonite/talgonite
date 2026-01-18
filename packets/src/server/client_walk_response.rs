use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug)]
pub struct ClientWalkResponse {
    pub direction: u8,
    pub from: (u16, u16),
}

impl TryFromBytes for ClientWalkResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let direction = cursor.read_u8()?;
        let from = {
            let x = cursor.read_u16::<BigEndian>()?;
            let y = cursor.read_u16::<BigEndian>()?;
            (x, y)
        };

        Ok(ClientWalkResponse { direction, from })
    }
}
