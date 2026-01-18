use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::Read;

#[derive(Debug, Clone)]
pub struct WorldMapNodeInfo {
    pub screen_position: (i16, i16),
    pub text: String,
    pub check_sum: u16,
    pub map_id: u16,
    pub destination_point: (i16, i16),
}

#[derive(Debug, Clone)]
pub struct WorldMap {
    pub field_name: String,
    pub nodes: Vec<WorldMapNodeInfo>,
    pub field_index: u8,
}

impl TryFromBytes for WorldMap {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let field_name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode field_name: {}", e))?
        };
        let node_count = cursor.read_u8()?;
        let field_index = cursor.read_u8()?;
        let mut nodes = Vec::with_capacity(node_count as usize);
        for _ in 0..node_count {
            let screen_position = (
                cursor.read_i16::<BigEndian>()?,
                cursor.read_i16::<BigEndian>()?,
            );
            let text = {
                let mut buf = vec![0; cursor.read_u8()? as usize];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode text: {}", e))?
            };
            let check_sum = cursor.read_u16::<BigEndian>()?;
            let map_id = cursor.read_u16::<BigEndian>()?;
            let destination_point = (
                cursor.read_i16::<BigEndian>()?,
                cursor.read_i16::<BigEndian>()?,
            );
            nodes.push(WorldMapNodeInfo {
                screen_position,
                text,
                check_sum,
                map_id,
                destination_point,
            });
        }
        Ok(WorldMap {
            field_name,
            nodes,
            field_index,
        })
    }
}
