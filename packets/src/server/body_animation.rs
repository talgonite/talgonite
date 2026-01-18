use crate::TryFromBytes;
use crate::types::BodyAnimationKind;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct BodyAnimation {
    pub source_id: u32,
    pub kind: BodyAnimationKind,
    pub animation_speed: u16,
    pub sound: Option<u8>,
}

impl TryFromBytes for BodyAnimation {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let source_id = cursor.read_u32::<BigEndian>()?;
        let kind =
            BodyAnimationKind::try_from(cursor.read_u8()?).unwrap_or(BodyAnimationKind::None);
        let animation_speed = cursor.read_u16::<BigEndian>()?;
        let sound_byte = cursor.read_u8()?;
        let sound = if sound_byte == u8::MAX {
            None
        } else {
            Some(sound_byte)
        };
        Ok(BodyAnimation {
            source_id,
            kind,
            animation_speed,
            sound,
        })
    }
}
