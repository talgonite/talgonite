use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug, Clone)]
pub struct MapInfo {
    pub map_id: u16,
    pub width: u8,
    pub height: u8,
    pub flags: u8,
    pub check_sum: u16,
    pub name: String,
}

impl Default for MapInfo {
    fn default() -> Self {
        MapInfo {
            map_id: 0,
            width: 0,
            height: 0,
            flags: 0,
            check_sum: 0,
            name: String::new(),
        }
    }
}

impl MapInfo {
    pub fn get_stride(&self) -> usize {
        self.width as usize * 6
    }
}

impl TryFromBytes for MapInfo {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let map_id = cursor.read_u16::<BigEndian>()?;
        let width = cursor.read_u8()?;
        let height = cursor.read_u8()?;
        let flags = cursor.read_u8()?;
        let _ = cursor.read_u16::<BigEndian>()?; //LI: what is this for?
        let check_sum = cursor.read_u16::<BigEndian>()?;
        let name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode name: {}", e))?
        };
        Ok(MapInfo {
            map_id,
            width,
            height,
            flags,
            check_sum,
            name,
        })
    }
}
