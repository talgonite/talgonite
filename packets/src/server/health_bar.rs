use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct HealthBar {
    pub source_id: u32,
    pub health_percent: u8,
    pub sound: Option<u8>,
}

impl TryFromBytes for HealthBar {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let source_id = cursor.read_u32::<BigEndian>()?;
        let _ = cursor.read_u8()?; // Unused/padding byte
        let health_percent = cursor.read_u8()?;
        let sound = {
            let sound = cursor.read_u8()?;
            if sound == u8::MAX { None } else { Some(sound) }
        };

        Ok(HealthBar {
            source_id,
            health_percent,
            sound,
        })
    }
}
