use crate::TryFromBytes;
use byteorder::ReadBytesExt;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub enum Sound {
    Music(u8),
    Sound(u8),
}

impl TryFromBytes for Sound {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let indicator_or_index = cursor.read_u8()?;

        if indicator_or_index == u8::MAX {
            let music_index = cursor.read_u8()?;
            Ok(Sound::Music(music_index))
        } else {
            Ok(Sound::Sound(indicator_or_index))
        }
    }
}
