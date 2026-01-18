use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct MapData {
    pub row: u16,
    pub data: Vec<u8>,
}

impl TryFromBytes for MapData {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let current_y_index = cursor.read_u16::<BigEndian>()?;
        let mut map_data = vec![];
        cursor.read_to_end(&mut map_data)?;

        for chunk in map_data.chunks_exact_mut(2) {
            chunk.swap(0, 1);
        }

        Ok(MapData {
            row: current_y_index,
            data: map_data,
        })
    }
}
