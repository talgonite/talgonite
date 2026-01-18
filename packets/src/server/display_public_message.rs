use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum PublicMessageType {
    /// Normal message above a player's head that shows in the chat window as "PlayerName: Message"
    Normal = 0,
    /// Yellow message above a player's head that shows in the chat window as "PlayerName! Message"
    Shout = 1,
    /// Chant shows in light blue text above a player's head when they are invoking spells
    Chant = 2,
}

#[derive(Debug, Clone)]
pub struct DisplayPublicMessage {
    pub message_type: PublicMessageType,
    pub source_id: u32,
    pub message: String,
}

impl TryFromBytes for DisplayPublicMessage {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let message_type = cursor.read_u8()?;
        let source_id = cursor.read_u32::<BigEndian>()?;
        let message = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode message: {}", e))?
        };
        Ok(DisplayPublicMessage {
            message_type: PublicMessageType::try_from(message_type)?,
            source_id,
            message,
        })
    }
}
