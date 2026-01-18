use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum NotepadType {
    Brown = 0,
    GlitchedBlue1 = 1,
    GlitchedBlue2 = 2,
    Orange = 3,
    White = 4,
}

#[derive(Debug)]
pub struct Notepad {
    pub slot: u8,
    pub notepad_type: NotepadType,
    pub height: u8,
    pub width: u8,
    pub message: String,
}

impl TryFromBytes for Notepad {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let slot = cursor.read_u8()?;
        let notepad_type = cursor.read_u8()?;
        let height = cursor.read_u8()?;
        let width = cursor.read_u8()?;
        let message = {
            let mut buf = vec![0; cursor.read_u16::<BigEndian>()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode message: {}", e))?
        };
        Ok(Notepad {
            slot,
            notepad_type: notepad_type.try_into()?,
            height,
            width,
            message,
        })
    }
}
