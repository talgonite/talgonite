use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug)]
pub struct UserId {
    pub id: u32,
    pub direction: u8,
    pub base_class: u8,
}

impl TryFromBytes for UserId {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let id = cursor.read_u32::<BigEndian>()?;
        let direction = cursor.read_u8()?;
        let _ = cursor.read_u8()?; //LI: what is this for?
        let base_class = cursor.read_u8()?;
        // let _ = cursor.read_u8()?; //LI: what is this for?
        // let _ = cursor.read_u8()?; //LI: what is this for?
        // let _ = cursor.read_u8()?; //LI: what is this for?;

        Ok(UserId {
            id,
            direction,
            base_class,
        })
    }
}
