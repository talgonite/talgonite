use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ServerMessageType {
    /// Text appears blue, and appears in the top left
    Whisper = 0,
    /// Text is in the action bar and shows up in Shift+F
    OrangeBar1 = 1,
    /// Text is in the action bar and shows up in Shift+F
    OrangeBar2 = 2,
    /// Text appears in the action bar and Shift+F
    ActiveMessage = 3,
    /// Text is in the action bar and shows up in Shift+F
    OrangeBar3 = 4,
    /// Text is in the action bar and shows up in Shift+F. In official this was used for admin world messages
    AdminMessage = 5,
    /// Text is only in the action bar, and will not show up in Shift+F
    OrangeBar5 = 6,
    /// UserOptions are sent via this text channel
    UserOptions = 7,
    /// Pops open a window with a scroll bar. In official this was used for Sense
    ScrollWindow = 8,
    /// Pops open a window with no scroll bar. In official this was used for perish lore
    NonScrollWindow = 9,
    /// Pops open a window with a wooden border. In official this was used for signposts and wooden boards
    WoodenBoard = 10,
    /// Text appears in a puke-green color. In official this was used for group chat
    GroupChat = 11,
    /// Text appears in an olive-green color. In official this was used for guild chat
    GuildChat = 12,
    /// Closes opened pop-up windows. ScrollWindow, NonScrollWindow, WoodenBoard
    ClosePopup = 17,
    /// Text appears white, and persists indefinitely until cleared in the top right corner
    PersistentMessage = 18,
}

#[derive(Debug, Clone)]
pub struct ServerMessage {
    pub message_type: ServerMessageType,
    pub message: String,
}

impl TryFromBytes for ServerMessage {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let message_type = cursor.read_u8()?;
        let message = {
            let mut buf = vec![0; cursor.read_u16::<BigEndian>()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode message: {}", e))?
        };
        Ok(ServerMessage {
            message_type: message_type.try_into()?,
            message,
        })
    }
}
